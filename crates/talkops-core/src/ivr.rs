//! IVR menus: a greeting, then keypad choices leading to destinations.
//!
//! Greeting audio lives in the shared sounds volume:
//! `ivr/<tenant>/<menu>.wav` (TTS rendered by the media worker or uploaded).

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::jobs::{self, NewJob};
use crate::numbering;
use crate::tenant::TenantId;
use crate::trunks::NumberDestination;

/// Job kind: render an IVR greeting (media worker).
pub const JOB_TTS_IVR: &str = "tts.ivr";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MenuOption {
    /// `0`–`9`, `*` or `#`.
    pub digit: String,
    #[serde(rename = "type")]
    pub kind: NumberDestination,
    pub id: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct IvrMenu {
    pub id: Uuid,
    pub number: Option<String>,
    pub name: String,
    pub language: Option<String>,
    /// `tts` or `upload`.
    pub greeting: String,
    pub greeting_text: String,
    /// `none`, `pending`, `ready` or `failed`.
    pub greeting_status: String,
    pub timeout_secs: i32,
    pub max_tries: i32,
    pub direct_dial: bool,
    #[schema(value_type = Vec<MenuOption>)]
    pub options: sqlx::types::Json<Vec<MenuOption>>,
    pub timeout_type: NumberDestination,
    pub timeout_id: Option<Uuid>,
}

impl IvrMenu {
    /// Destination for a pressed digit.
    pub fn option(&self, digit: &str) -> Option<&MenuOption> {
        self.options.iter().find(|o| o.digit == digit)
    }
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct IvrMenuInput {
    #[serde(default)]
    pub number: Option<String>,
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default = "tts")]
    pub greeting: String,
    #[serde(default)]
    pub greeting_text: String,
    #[serde(default = "five")]
    pub timeout_secs: i32,
    #[serde(default = "three")]
    pub max_tries: i32,
    #[serde(default)]
    pub direct_dial: bool,
    #[serde(default)]
    pub options: Vec<MenuOption>,
    #[serde(default = "no_destination")]
    pub timeout_type: NumberDestination,
    #[serde(default)]
    pub timeout_id: Option<Uuid>,
}

fn tts() -> String {
    "tts".into()
}
fn five() -> i32 {
    5
}
fn three() -> i32 {
    3
}
fn no_destination() -> NumberDestination {
    NumberDestination::None
}

/// Greeting file relative to the sounds directory.
pub fn greeting_file(tenant: TenantId, menu: Uuid) -> String {
    format!("ivr/{tenant}/{menu}.wav")
}

const COLUMNS: &str = "id, number, name, language, greeting, greeting_text, greeting_status, \
                       timeout_secs, max_tries, direct_dial, options, timeout_type, timeout_id";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<IvrMenu>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM ivr_menus WHERE tenant_id = $1 ORDER BY number NULLS LAST, name"
    );
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<IvrMenu> {
    let sql = format!("SELECT {COLUMNS} FROM ivr_menus WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
}

async fn validate(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &IvrMenuInput,
    emergency: &[String],
) -> CoreResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 64 {
        return Err(CoreError::Validation(
            "name is required (max. 64 characters)".into(),
        ));
    }
    if let Some(n) = input.number.as_deref().filter(|n| !n.is_empty()) {
        numbering::validate_number(n, emergency)?;
        numbering::ensure_free(pool, tenant, n, id).await?;
    }
    if let Some(lang) = input.language.as_deref() {
        if !crate::prompts::LANGUAGES.contains(&lang) {
            return Err(CoreError::Validation(format!(
                "unsupported language `{lang}`"
            )));
        }
    }
    match input.greeting.as_str() {
        "tts" if input.greeting_text.trim().is_empty() => {
            return Err(CoreError::Validation("greeting text is required".into()));
        }
        "tts" | "upload" => {}
        _ => return Err(CoreError::Validation("invalid greeting type".into())),
    }
    if input.greeting_text.chars().count() > 2000 {
        return Err(CoreError::Validation("greeting text is too long".into()));
    }
    if !(1..=30).contains(&input.timeout_secs) || !(1..=10).contains(&input.max_tries) {
        return Err(CoreError::Validation(
            "invalid timeout or number of tries".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for o in &input.options {
        let valid = o.digit.len() == 1
            && o.digit
                .chars()
                .all(|c| c.is_ascii_digit() || c == '*' || c == '#');
        if !valid {
            return Err(CoreError::Validation(format!("invalid key `{}`", o.digit)));
        }
        if input.direct_dial && o.digit == "#" {
            return Err(CoreError::Validation(
                "# ends direct dialing and cannot be a menu option".into(),
            ));
        }
        if !seen.insert(o.digit.clone()) {
            return Err(CoreError::Validation(format!(
                "key {} is used twice",
                o.digit
            )));
        }
        if o.kind == NumberDestination::None {
            return Err(CoreError::Validation(format!(
                "key {} has no destination",
                o.digit
            )));
        }
        numbering::check_destination(pool, tenant, o.kind, o.id).await?;
    }
    numbering::check_destination(pool, tenant, input.timeout_type, input.timeout_id).await
}

fn number(input: &IvrMenuInput) -> Option<&str> {
    input.number.as_deref().filter(|n| !n.is_empty())
}

/// Saves a menu; a new or changed TTS greeting is queued for rendering.
pub async fn save(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &IvrMenuInput,
    emergency: &[String],
    default_language: &str,
) -> CoreResult<IvrMenu> {
    let old = match id {
        Some(id) => Some(get(pool, tenant, id).await?),
        None => None,
    };
    validate(pool, tenant, id, input, emergency).await?;
    let text = input.greeting_text.trim();
    let rerender = input.greeting == "tts"
        && old.as_ref().is_none_or(|o| {
            o.greeting != "tts"
                || o.greeting_text != text
                || o.language != input.language
                || o.greeting_status == "failed"
        });
    let status = if rerender {
        "pending".to_owned()
    } else if input.greeting == "upload" && old.as_ref().is_none_or(|o| o.greeting != "upload") {
        "none".to_owned()
    } else {
        old.as_ref()
            .map(|o| o.greeting_status.clone())
            .unwrap_or("none".into())
    };
    let timeout_id = input
        .timeout_id
        .filter(|_| input.timeout_type != NumberDestination::None);
    let mut tx = pool.begin().await?;
    let menu: IvrMenu = match id {
        None => {
            let sql = format!(
                "INSERT INTO ivr_menus (tenant_id, number, name, language, greeting, greeting_text,
                     greeting_status, timeout_secs, max_tries, direct_dial, options, timeout_type, timeout_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING {COLUMNS}"
            );
            sqlx::query_as(&sql)
                .bind(tenant)
                .bind(number(input))
                .bind(input.name.trim())
                .bind(&input.language)
                .bind(&input.greeting)
                .bind(text)
                .bind(&status)
                .bind(input.timeout_secs)
                .bind(input.max_tries)
                .bind(input.direct_dial)
                .bind(sqlx::types::Json(&input.options))
                .bind(input.timeout_type)
                .bind(timeout_id)
                .fetch_one(&mut *tx)
                .await?
        }
        Some(id) => {
            let sql = format!(
                "UPDATE ivr_menus SET number = $3, name = $4, language = $5, greeting = $6,
                     greeting_text = $7, greeting_status = $8, timeout_secs = $9, max_tries = $10,
                     direct_dial = $11, options = $12, timeout_type = $13, timeout_id = $14,
                     updated_at = now()
                 WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
            );
            sqlx::query_as(&sql)
                .bind(tenant)
                .bind(id)
                .bind(number(input))
                .bind(input.name.trim())
                .bind(&input.language)
                .bind(&input.greeting)
                .bind(text)
                .bind(&status)
                .bind(input.timeout_secs)
                .bind(input.max_tries)
                .bind(input.direct_dial)
                .bind(sqlx::types::Json(&input.options))
                .bind(input.timeout_type)
                .bind(timeout_id)
                .fetch_one(&mut *tx)
                .await?
        }
    };
    if rerender {
        let lang = input.language.as_deref().unwrap_or(default_language);
        let mut job = NewJob::new(
            JOB_TTS_IVR,
            serde_json::json!({
                "menu_id": menu.id,
                "text": text,
                "language": crate::prompts::language(lang),
                "file": greeting_file(tenant, menu.id),
            }),
        );
        job.tenant_id = tenant;
        job.priority = 10;
        jobs::enqueue(&mut *tx, job).await?;
    }
    tx.commit().await?;
    Ok(menu)
}

/// Result of rendering a TTS greeting (media worker); ignored if the text
/// changed in the meantime.
pub async fn set_greeting_status(
    pool: &PgPool,
    menu: Uuid,
    text: &str,
    ready: bool,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE ivr_menus SET greeting_status = $3, updated_at = now()
         WHERE id = $1 AND greeting = 'tts' AND greeting_text = $2",
    )
    .bind(menu)
    .bind(text)
    .bind(if ready { "ready" } else { "failed" })
    .execute(pool)
    .await?;
    Ok(())
}

/// An uploaded greeting is in place.
pub async fn set_uploaded_greeting(pool: &PgPool, tenant: TenantId, menu: Uuid) -> CoreResult<()> {
    let res = sqlx::query(
        "UPDATE ivr_menus SET greeting = 'upload', greeting_status = 'ready', updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(menu)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM ivr_menus WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
