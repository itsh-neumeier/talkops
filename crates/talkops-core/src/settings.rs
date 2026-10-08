//! Per-tenant telephony settings.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::dialing::DialPlanSettings;
use crate::error::CoreResult;
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct TenantSettings {
    pub country_code: String,
    pub area_code: String,
    pub national_prefix: String,
    pub international_prefix: String,
    pub emergency_numbers: Vec<String>,
    pub external_ip: String,
    pub default_language: String,
    pub default_number_id: Option<Uuid>,
    /// IANA time zone, used for phone provisioning.
    pub timezone: String,
    /// Record calls by direction (extensions may override).
    #[serde(default)]
    pub record_inbound: bool,
    #[serde(default)]
    pub record_outbound: bool,
    #[serde(default)]
    pub record_internal: bool,
    /// Announce recordings to both parties.
    #[serde(default = "yes")]
    pub recording_announcement: bool,
    /// Days to keep recordings; 0 = forever.
    #[serde(default = "default_retention")]
    pub recording_retention_days: i32,
    /// Transcribe recordings and voicemails (media worker, whisper.cpp).
    #[serde(default)]
    pub transcription_enabled: bool,
    /// Music on hold: a built-in piece ([crate::audio::MUSIC]), empty for
    /// all pieces shuffled. An own clip takes precedence.
    #[serde(default)]
    pub hold_music: String,
    #[serde(default)]
    pub hold_music_clip_id: Option<Uuid>,
    /// Engine of the first (quick) pass: `fast`, `accurate`, `best`,
    /// `german` (see [TranscriptionQuality]) or `api` (see [TranscriptionEngine]).
    #[serde(default = "default_quality")]
    pub transcription_quality: String,
    /// Engine of an optional second, more accurate pass that replaces the
    /// first transcript; empty = none.
    #[serde(default)]
    pub transcription_refine: String,
    /// Names and terms passed to Whisper as context, comma-separated.
    #[serde(default)]
    pub transcription_vocabulary: String,
}

fn default_quality() -> String {
    "fast".into()
}

/// Accuracy of transcriptions; better costs time and memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionQuality {
    /// The worker's configured model (default `large-v3-turbo-q5_0`).
    Fast,
    /// `large-v3-q5_0`: full decoder, about 2 GB RAM, several times slower.
    Accurate,
    /// `large-v3`: unquantized, about 4 GB RAM, slowest.
    Best,
    /// large-v3-turbo fine-tuned on German speech (primeLine), about 2 GB
    /// RAM; for German calls.
    German,
}

impl TranscriptionQuality {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fast" => Some(Self::Fast),
            "accurate" => Some(Self::Accurate),
            "best" => Some(Self::Best),
            "german" => Some(Self::German),
            _ => None,
        }
    }
}

/// Where a transcription runs: a local Whisper model or an
/// OpenAI-compatible API ([crate::transcription_api]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionEngine {
    Local(TranscriptionQuality),
    Api,
}

impl TranscriptionEngine {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "api" => Some(Self::Api),
            other => TranscriptionQuality::parse(other).map(Self::Local),
        }
    }

    /// The engine of the second pass, if one is configured and differs
    /// from the first.
    pub fn refine(settings: &TenantSettings) -> Option<Self> {
        if settings.transcription_refine.is_empty()
            || settings.transcription_refine == settings.transcription_quality
        {
            return None;
        }
        Self::parse(&settings.transcription_refine)
    }
}

/// Longest accepted vocabulary (Whisper's prompt holds ~220 tokens).
pub const MAX_VOCABULARY: usize = 600;

fn yes() -> bool {
    true
}

fn default_retention() -> i32 {
    90
}

impl TenantSettings {
    pub fn dial_plan(&self) -> DialPlanSettings {
        DialPlanSettings {
            country_code: self.country_code.clone(),
            area_code: self.area_code.clone(),
            national_prefix: self.national_prefix.clone(),
            international_prefix: self.international_prefix.clone(),
            emergency_numbers: self.emergency_numbers.clone(),
        }
    }
}

const COLUMNS: &str = "country_code, area_code, national_prefix, international_prefix, \
                       emergency_numbers, external_ip, default_language, default_number_id, timezone, \
                       record_inbound, record_outbound, record_internal, recording_announcement, \
                       recording_retention_days, transcription_enabled, hold_music, hold_music_clip_id, \
                       transcription_quality, transcription_vocabulary, transcription_refine";

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<TenantSettings> {
    let sql = format!("SELECT {COLUMNS} FROM tenant_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(db).await?)
}

pub async fn update<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    s: &TenantSettings,
) -> CoreResult<TenantSettings> {
    let sql = format!(
        "UPDATE tenant_settings SET country_code = $2, area_code = $3, national_prefix = $4,
             international_prefix = $5, emergency_numbers = $6, external_ip = $7,
             default_language = $8, default_number_id = $9, timezone = $10,
             record_inbound = $11, record_outbound = $12, record_internal = $13,
             recording_announcement = $14, recording_retention_days = $15,
             transcription_enabled = $16, hold_music = $17, hold_music_clip_id = $18,
             transcription_quality = $19, transcription_vocabulary = $20,
             transcription_refine = $21, updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(&s.country_code)
        .bind(&s.area_code)
        .bind(&s.national_prefix)
        .bind(&s.international_prefix)
        .bind(&s.emergency_numbers)
        .bind(&s.external_ip)
        .bind(&s.default_language)
        .bind(s.default_number_id)
        .bind(&s.timezone)
        .bind(s.record_inbound)
        .bind(s.record_outbound)
        .bind(s.record_internal)
        .bind(s.recording_announcement)
        .bind(s.recording_retention_days)
        .bind(s.transcription_enabled)
        .bind(&s.hold_music)
        .bind(s.hold_music_clip_id)
        .bind(&s.transcription_quality)
        .bind(s.transcription_vocabulary.trim())
        .bind(&s.transcription_refine)
        .fetch_one(db)
        .await?)
}
