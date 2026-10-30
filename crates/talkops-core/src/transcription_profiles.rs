//! Which engine transcribes what (ADR 0024): one profile per purpose, each
//! a local Whisper model or a model of a cloud connection
//! ([crate::transcription_api]). Also records the audio sent to the cloud,
//! for an estimate of the cost.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::settings::TranscriptionQuality;
use crate::tenant::TenantId;
use crate::transcription_api::valid_model;

/// What a transcription is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Preview during an active call.
    Live,
    /// First pass of a call recording, shown right away.
    Recording,
    /// Optional second, more accurate pass of a call recording.
    RecordingRefine,
    /// First pass of a voicemail; the e-mail waits for it.
    Voicemail,
    /// Optional second pass of a voicemail.
    VoicemailRefine,
}

impl Purpose {
    pub const ALL: [Self; 5] = [
        Self::Live,
        Self::Recording,
        Self::RecordingRefine,
        Self::Voicemail,
        Self::VoicemailRefine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Recording => "recording",
            Self::RecordingRefine => "recording_refine",
            Self::Voicemail => "voicemail",
            Self::VoicemailRefine => "voicemail_refine",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == s)
    }

    /// Second passes can be switched off.
    pub fn is_refine(self) -> bool {
        matches!(self, Self::RecordingRefine | Self::VoicemailRefine)
    }

    /// The pass of a recording or voicemail (1 or 2).
    pub fn for_pass(voicemail: bool, pass: u8) -> Self {
        match (voicemail, pass >= 2) {
            (false, false) => Self::Recording,
            (false, true) => Self::RecordingRefine,
            (true, false) => Self::Voicemail,
            (true, true) => Self::VoicemailRefine,
        }
    }

    fn default_profile(self) -> Profile {
        let (engine, local_model) = match self {
            Self::Live => ("local", "base"),
            Self::Recording | Self::Voicemail => ("local", "fast"),
            Self::RecordingRefine | Self::VoicemailRefine => ("off", "fast"),
        };
        Profile {
            purpose: self.as_str().into(),
            engine: engine.into(),
            local_model: local_model.into(),
            provider_id: None,
            model: String::new(),
            realtime: false,
            coral_stable_preview: false,
            price_per_minute: None,
            list_price: None,
            provider_kind: None,
        }
    }
}

/// List prices of OpenAI transcription models in USD per audio minute.
/// Source: <https://developers.openai.com/api/docs/pricing>, checked
/// 2026-10-13. A profile can set its own price.
pub const OPENAI_PRICES: &[(&str, f64)] = &[
    ("gpt-transcribe", 0.0045),
    ("gpt-4o-mini-transcribe", 0.003),
    ("gpt-4o-transcribe", 0.006),
    ("gpt-4o-transcribe-diarize", 0.006),
    ("gpt-live-transcribe", 0.017),
    ("gpt-realtime-whisper", 0.017),
    ("whisper-1", 0.006),
];

/// The list price of a model of a connection of the given kind.
pub fn list_price(kind: &str, model: &str) -> Option<f64> {
    (kind == "openai")
        .then(|| OPENAI_PRICES.iter().find(|(m, _)| *m == model))
        .flatten()
        .map(|(_, p)| *p)
}

/// A profile as shown and edited in the UI.
#[derive(Debug, Clone, PartialEq, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Profile {
    /// `live`, `recording`, `recording_refine`, `voicemail` or `voicemail_refine`.
    pub purpose: String,
    /// `off` (second passes only), `local` or `cloud`.
    pub engine: String,
    /// Local Whisper model: `base`, `fast`, `accurate`, `best` or `german`
    /// (live: `base` or `fast`).
    pub local_model: String,
    /// Cloud connection.
    pub provider_id: Option<Uuid>,
    /// Cloud model, e.g. `gpt-transcribe`.
    pub model: String,
    /// Live only: stream over the Realtime API.
    #[serde(default)]
    pub realtime: bool,
    /// Coral realtime only: hold open words until the model closes them.
    #[serde(default)]
    pub coral_stable_preview: bool,
    /// Own price per audio minute; `null` = list price.
    #[serde(default)]
    pub price_per_minute: Option<f64>,
    /// The known list price of the cloud model (read-only).
    #[serde(default)]
    pub list_price: Option<f64>,
    /// The kind of the connection: `openai`, `compatible` or `coral`
    /// (read-only). For `coral`, `local_model` is the local fallback.
    #[serde(default)]
    #[sqlx(skip)]
    pub provider_kind: Option<String>,
}

/// The engine that runs a transcription.
#[derive(Debug, Clone, PartialEq)]
pub enum Engine {
    Local(TranscriptionQuality),
    Cloud {
        provider_id: Uuid,
        model: String,
        realtime: bool,
        coral_stable_preview: bool,
        /// Price per audio minute, if known.
        price: Option<f64>,
        /// A speech service on this server, coral-stt (ADR 0046) or cuda-stt
        /// (ADR 0054): no usage, and `fallback` runs when it is not available.
        coral: bool,
        /// The local model that replaces an unavailable Coral connection.
        /// The live preview always falls back to `base`.
        fallback: Option<TranscriptionQuality>,
    },
}

#[derive(FromRow)]
struct Row {
    #[sqlx(flatten)]
    profile: Profile,
    kind: Option<String>,
}

const SELECT: &str = "SELECT p.purpose, p.engine, p.local_model, p.provider_id, p.model, \
                      p.realtime, p.coral_stable_preview, p.price_per_minute, NULL::float8 AS list_price, tp.kind \
                      FROM transcription_profiles p \
                      LEFT JOIN transcription_providers tp \
                        ON tp.tenant_id = p.tenant_id AND tp.id = p.provider_id \
                      WHERE p.tenant_id = $1";

fn finish(row: Row) -> Profile {
    let mut profile = row.profile;
    profile.list_price = match (&row.kind, profile.engine.as_str()) {
        (Some(kind), "cloud") => list_price(kind, &profile.model),
        _ => None,
    };
    profile.provider_kind = row.kind;
    profile
}

/// All profiles, in the order of [Purpose::ALL]; missing ones are defaults.
pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Profile>> {
    let rows: Vec<Row> = sqlx::query_as(SELECT).bind(tenant).fetch_all(pool).await?;
    let mut stored: Vec<Profile> = rows.into_iter().map(finish).collect();
    Ok(Purpose::ALL
        .into_iter()
        .map(
            |purpose| match stored.iter().position(|p| p.purpose == purpose.as_str()) {
                Some(i) => stored.swap_remove(i),
                None => purpose.default_profile(),
            },
        )
        .collect())
}

fn check(profile: &Profile) -> CoreResult<Purpose> {
    let invalid = |msg: &str| Err(CoreError::Validation(format!("{}: {msg}", profile.purpose)));
    let Some(purpose) = Purpose::parse(&profile.purpose) else {
        return Err(CoreError::Validation(format!(
            "unknown purpose {}",
            profile.purpose
        )));
    };
    match profile.engine.as_str() {
        "off" if !purpose.is_refine() => return invalid("only a second pass can be off"),
        "off" | "local" | "cloud" => {}
        _ => return invalid("engine must be off, local or cloud"),
    }
    let Some(local) = TranscriptionQuality::parse(&profile.local_model) else {
        return invalid("unknown local model");
    };
    if profile.engine == "local"
        && purpose == Purpose::Live
        && !matches!(
            local,
            TranscriptionQuality::Base | TranscriptionQuality::Fast
        )
    {
        return invalid("live previews use the base or fast model");
    }
    if profile.engine == "cloud" {
        if profile.provider_id.is_none() {
            return invalid("select a cloud connection");
        }
        if !valid_model(profile.model.trim()) {
            return invalid("invalid model name");
        }
    }
    if profile.realtime && (purpose != Purpose::Live || profile.engine != "cloud") {
        return invalid("only a cloud live preview can stream");
    }
    if profile.coral_stable_preview
        && (purpose != Purpose::Live || profile.engine != "cloud" || !profile.realtime)
    {
        return invalid("complete-word preview requires a Coral realtime live profile");
    }
    if let Some(price) = profile.price_per_minute
        && !(price.is_finite() && (0.0..=10.0).contains(&price))
    {
        return invalid("price per minute must be between 0 and 10");
    }
    Ok(purpose)
}

/// Replaces the given profiles; others stay as they are.
pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    profiles: &[Profile],
) -> CoreResult<Vec<Profile>> {
    let mut seen = Vec::new();
    for profile in profiles {
        let purpose = check(profile)?;
        if seen.contains(&purpose) {
            return Err(CoreError::Validation(format!(
                "{} is given twice",
                profile.purpose
            )));
        }
        seen.push(purpose);
    }
    let mut tx = pool.begin().await?;
    for p in profiles {
        let cloud = p.engine == "cloud";
        if p.coral_stable_preview {
            let kind: Option<String> = sqlx::query_scalar(
                "SELECT kind FROM transcription_providers WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant)
            .bind(p.provider_id)
            .fetch_optional(&mut *tx)
            .await?;
            if !kind
                .as_deref()
                .is_some_and(crate::transcription_api::is_local_kind)
            {
                return Err(CoreError::Validation(
                    "complete-word preview requires a Coral or CUDA connection".into(),
                ));
            }
        }
        sqlx::query(
            "INSERT INTO transcription_profiles
                 (tenant_id, purpose, engine, local_model, provider_id, model, realtime, price_per_minute, coral_stable_preview)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (tenant_id, purpose) DO UPDATE SET engine = $3, local_model = $4,
                 provider_id = $5, model = $6, realtime = $7, price_per_minute = $8,
                 coral_stable_preview = $9, updated_at = now()",
        )
        .bind(tenant)
        .bind(&p.purpose)
        .bind(&p.engine)
        .bind(&p.local_model)
        .bind(if cloud { p.provider_id } else { None })
        .bind(if cloud { p.model.trim() } else { "" })
        .bind(p.realtime && cloud)
        .bind(if cloud { p.price_per_minute } else { None })
        .bind(p.coral_stable_preview)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list(pool, tenant).await
}

/// The engine for a purpose; `None` if it is switched off.
pub async fn engine(
    pool: &PgPool,
    tenant: TenantId,
    purpose: Purpose,
) -> CoreResult<Option<Engine>> {
    let sql = format!("{SELECT} AND p.purpose = $2");
    let row: Option<Row> = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(purpose.as_str())
        .fetch_optional(pool)
        .await?;
    let profile = row.map(finish).unwrap_or_else(|| purpose.default_profile());
    Ok(to_engine(&profile))
}

fn to_engine(p: &Profile) -> Option<Engine> {
    match (p.engine.as_str(), p.provider_id) {
        ("cloud", Some(provider_id)) => {
            let coral = p
                .provider_kind
                .as_deref()
                .is_some_and(crate::transcription_api::is_local_kind);
            let fallback = if p.purpose == Purpose::Live.as_str() {
                TranscriptionQuality::Base
            } else {
                TranscriptionQuality::parse(&p.local_model).unwrap_or(TranscriptionQuality::Fast)
            };
            Some(Engine::Cloud {
                provider_id,
                model: p.model.clone(),
                realtime: p.realtime,
                coral_stable_preview: coral && p.coral_stable_preview,
                price: if coral {
                    None
                } else {
                    p.price_per_minute.or(p.list_price)
                },
                coral,
                fallback: coral.then_some(fallback),
            })
        }
        ("local", _) => Some(Engine::Local(
            TranscriptionQuality::parse(&p.local_model).unwrap_or(TranscriptionQuality::Fast),
        )),
        _ => None,
    }
}

/// Adds audio sent to a cloud provider to today's usage.
pub async fn record_usage(
    pool: &PgPool,
    tenant: TenantId,
    purpose: Purpose,
    engine: &str,
    seconds: f64,
    price_per_minute: Option<f64>,
) -> CoreResult<()> {
    if !(seconds.is_finite() && seconds > 0.0) {
        return Ok(());
    }
    let (cost, unpriced) = match price_per_minute {
        Some(price) => (seconds / 60.0 * price, 0.0),
        None => (0.0, seconds),
    };
    sqlx::query(
        "INSERT INTO transcription_usage (tenant_id, day, purpose, engine, seconds, cost, unpriced_seconds)
         VALUES ($1, current_date, $2, $3, $4, $5, $6)
         ON CONFLICT (tenant_id, day, purpose, engine) DO UPDATE SET
             seconds = transcription_usage.seconds + $4,
             cost = transcription_usage.cost + $5,
             unpriced_seconds = transcription_usage.unpriced_seconds + $6",
    )
    .bind(tenant)
    .bind(purpose.as_str())
    .bind(engine)
    .bind(seconds)
    .bind(cost)
    .bind(unpriced)
    .execute(pool)
    .await?;
    Ok(())
}

/// Cloud usage of one month, purpose and engine.
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Usage {
    /// First day of the month, `YYYY-MM-DD`.
    pub month: String,
    pub purpose: String,
    pub engine: String,
    pub seconds: f64,
    /// Estimated cost (USD for OpenAI list prices).
    pub cost: f64,
    /// Seconds without a known price.
    pub unpriced_seconds: f64,
}

/// Usage of the current and the previous month, newest first.
pub async fn usage(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<Usage>> {
    Ok(sqlx::query_as(
        "SELECT to_char(date_trunc('month', day), 'YYYY-MM-DD') AS month, purpose, engine,
                sum(seconds) AS seconds, sum(cost) AS cost,
                sum(unpriced_seconds) AS unpriced_seconds
         FROM transcription_usage
         WHERE tenant_id = $1 AND day >= date_trunc('month', current_date) - interval '1 month'
         GROUP BY 1, 2, 3 ORDER BY 1 DESC, 2, 3",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await?)
}

/// Removes usage older than about two years (hourly retention).
pub async fn cleanup(pool: &PgPool) -> CoreResult<u64> {
    Ok(
        sqlx::query("DELETE FROM transcription_usage WHERE day < current_date - 731")
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(purpose: Purpose) -> Profile {
        purpose.default_profile()
    }

    #[test]
    fn purposes() {
        for p in Purpose::ALL {
            assert_eq!(Purpose::parse(p.as_str()), Some(p));
        }
        assert_eq!(Purpose::for_pass(false, 1), Purpose::Recording);
        assert_eq!(Purpose::for_pass(false, 2), Purpose::RecordingRefine);
        assert_eq!(Purpose::for_pass(true, 1), Purpose::Voicemail);
        assert_eq!(Purpose::for_pass(true, 2), Purpose::VoicemailRefine);
    }

    #[test]
    fn prices() {
        assert_eq!(list_price("openai", "gpt-transcribe"), Some(0.0045));
        assert_eq!(list_price("openai", "own-model"), None);
        assert_eq!(list_price("compatible", "whisper-1"), None);
    }

    #[test]
    fn validates() {
        for p in Purpose::ALL {
            assert!(check(&profile(p)).is_ok(), "default of {p:?}");
        }
        let mut p = profile(Purpose::Recording);
        p.engine = "off".into();
        assert!(check(&p).is_err(), "first pass cannot be off");
        let mut p = profile(Purpose::Live);
        p.local_model = "best".into();
        assert!(check(&p).is_err(), "live is limited to small models");
        p.engine = "cloud".into();
        assert!(check(&p).is_err(), "cloud needs a connection");
        p.provider_id = Some(Uuid::nil());
        p.model = "gpt-live-transcribe".into();
        p.realtime = true;
        assert!(check(&p).is_ok());
        p.price_per_minute = Some(-1.0);
        assert!(check(&p).is_err());
        let mut p = profile(Purpose::Recording);
        p.realtime = true;
        assert!(check(&p).is_err(), "only live streams");
    }

    #[test]
    fn engines() {
        assert_eq!(
            to_engine(&profile(Purpose::Live)),
            Some(Engine::Local(TranscriptionQuality::Base))
        );
        assert_eq!(to_engine(&profile(Purpose::RecordingRefine)), None);
        let mut p = profile(Purpose::Recording);
        p.engine = "cloud".into();
        p.provider_id = Some(Uuid::nil());
        p.model = "gpt-transcribe".into();
        p.list_price = Some(0.0045);
        assert_eq!(
            to_engine(&p),
            Some(Engine::Cloud {
                provider_id: Uuid::nil(),
                model: "gpt-transcribe".into(),
                realtime: false,
                coral_stable_preview: false,
                price: Some(0.0045),
                coral: false,
                fallback: None,
            })
        );
        p.price_per_minute = Some(0.0);
        assert!(matches!(
            to_engine(&p),
            Some(Engine::Cloud {
                price: Some(0.0),
                ..
            })
        ));
        // A Coral connection: no price, the local model is the fallback.
        p.provider_kind = Some("coral".into());
        p.model = "coral-auto".into();
        p.local_model = "accurate".into();
        assert_eq!(
            to_engine(&p),
            Some(Engine::Cloud {
                provider_id: Uuid::nil(),
                model: "coral-auto".into(),
                realtime: false,
                coral_stable_preview: false,
                price: None,
                coral: true,
                fallback: Some(TranscriptionQuality::Accurate),
            })
        );
        let mut live = profile(Purpose::Live);
        live.engine = "cloud".into();
        live.provider_id = Some(Uuid::nil());
        live.model = "coral-auto".into();
        live.realtime = true;
        live.local_model = "fast".into();
        live.provider_kind = Some("coral".into());
        assert!(matches!(
            to_engine(&live),
            Some(Engine::Cloud {
                coral: true,
                fallback: Some(TranscriptionQuality::Base),
                ..
            })
        ));
    }
}
