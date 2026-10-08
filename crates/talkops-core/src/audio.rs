//! Audio clips: generated with Piper, uploaded, or recorded in the browser.
//! Greetings and menu prompts reference a clip. The WAV lives in the sounds
//! volume at [`clip_file`]; generated clips are rendered by the media worker
//! (job [`JOB_TTS_CLIP`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::jobs::{self, NewJob};
use crate::tenant::TenantId;

/// Job kind: render a generated clip.
pub const JOB_TTS_CLIP: &str = "tts.clip";
/// Longest text for a generated clip.
pub const MAX_TEXT: usize = 1000;
/// Longest uploaded or recorded clip.
pub const MAX_DURATION_MS: i64 = 10 * 60 * 1000;

/// A selectable computer voice.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Voice {
    pub language: &'static str,
    /// 1 or 2.
    pub voice: i16,
    pub name: &'static str,
    /// `female` or `male`.
    pub gender: &'static str,
    /// Piper model (file `<model>.onnx`).
    pub model: &'static str,
}

/// All voices: two per language; voice 1 also speaks the system prompts.
pub const VOICES: &[Voice] = &[
    Voice {
        language: "de",
        voice: 1,
        name: "Thorsten",
        gender: "male",
        model: "de_DE-thorsten-medium",
    },
    Voice {
        language: "de",
        voice: 2,
        name: "Kerstin",
        gender: "female",
        model: "de_DE-kerstin-low",
    },
    Voice {
        language: "en",
        voice: 1,
        name: "Linda",
        gender: "female",
        model: "en_US-ljspeech-medium",
    },
    Voice {
        language: "en",
        voice: 2,
        name: "Joe",
        gender: "male",
        model: "en_US-joe-medium",
    },
];

/// The voice for a language and number (falls back to voice 1, German).
pub fn voice(language: &str, voice: i16) -> &'static Voice {
    VOICES
        .iter()
        .find(|v| v.language == language && v.voice == voice)
        .or_else(|| VOICES.iter().find(|v| v.language == language))
        .unwrap_or(&VOICES[0])
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Clip {
    pub id: Uuid,
    /// `tts`, `upload` or `recording`.
    pub source: String,
    pub text: String,
    pub language: String,
    pub voice: i16,
    /// `pending`, `ready` or `failed`.
    pub status: String,
    pub duration_ms: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TtsInput {
    pub text: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_voice")]
    pub voice: i16,
}

fn default_language() -> String {
    "de".into()
}
fn default_voice() -> i16 {
    1
}

const COLUMNS: &str = "id, source, text, language, voice, status, duration_ms, created_at";

/// Relative path (sounds volume) of a clip's audio.
pub fn clip_file(tenant: TenantId, id: Uuid) -> String {
    format!("clips/{tenant}/{id}.wav")
}

/// Creates a generated clip and queues its rendering.
pub async fn create_tts(
    pool: &PgPool,
    tenant: TenantId,
    user: Option<Uuid>,
    input: &TtsInput,
) -> CoreResult<Clip> {
    let text = input.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(CoreError::Validation("the text is empty".into()));
    }
    if text.chars().count() > MAX_TEXT {
        return Err(CoreError::Validation(format!(
            "at most {MAX_TEXT} characters"
        )));
    }
    if !VOICES
        .iter()
        .any(|v| v.language == input.language && v.voice == input.voice)
    {
        return Err(CoreError::Validation("unknown voice".into()));
    }
    let mut tx = pool.begin().await?;
    let sql = format!(
        "INSERT INTO audio_clips (tenant_id, source, text, language, voice, created_by)
         VALUES ($1, 'tts', $2, $3, $4, $5) RETURNING {COLUMNS}"
    );
    let clip: Clip = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(&text)
        .bind(&input.language)
        .bind(input.voice)
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    let mut job = NewJob::new(JOB_TTS_CLIP, serde_json::json!({ "clip_id": clip.id }));
    job.tenant_id = tenant;
    // Someone is waiting for the preview: before transcriptions and the like.
    job.priority = 10;
    job.max_attempts = 2;
    jobs::enqueue(&mut *tx, job).await?;
    tx.commit().await?;
    Ok(clip)
}

/// Registers an uploaded or recorded clip whose file is already written.
pub async fn create_file(
    pool: &PgPool,
    tenant: TenantId,
    user: Option<Uuid>,
    id: Uuid,
    source: &str,
    duration_ms: i32,
) -> CoreResult<Clip> {
    if !["upload", "recording"].contains(&source) {
        return Err(CoreError::Validation("invalid source".into()));
    }
    let sql = format!(
        "INSERT INTO audio_clips (id, tenant_id, source, status, duration_ms, created_by)
         VALUES ($1, $2, $3, 'ready', $4, $5) RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(id)
        .bind(tenant)
        .bind(source)
        .bind(duration_ms)
        .bind(user)
        .fetch_one(pool)
        .await?)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Clip> {
    let sql = format!("SELECT {COLUMNS} FROM audio_clips WHERE tenant_id = $1 AND id = $2");
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(CoreError::NotFound)
}

/// What the worker needs to render a clip.
#[derive(Debug, Clone, FromRow)]
pub struct RenderJob {
    pub tenant_id: TenantId,
    pub text: String,
    pub language: String,
    pub voice: i16,
}

pub async fn render_job(pool: &PgPool, id: Uuid) -> CoreResult<RenderJob> {
    sqlx::query_as(
        "SELECT tenant_id, text, language, voice FROM audio_clips
         WHERE id = $1 AND source = 'tts'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(CoreError::NotFound)
}

/// Stores the outcome of a rendering.
pub async fn set_rendered(pool: &PgPool, id: Uuid, duration_ms: Option<i32>) -> CoreResult<()> {
    sqlx::query(
        "UPDATE audio_clips SET status = CASE WHEN $2::int IS NULL THEN 'failed' ELSE 'ready' END,
             duration_ms = COALESCE($2, 0)
         WHERE id = $1",
    )
    .bind(id)
    .bind(duration_ms)
    .execute(pool)
    .await?;
    Ok(())
}

/// Checks that a clip referenced by a greeting exists in the tenant.
pub async fn ensure_exists(pool: &PgPool, tenant: TenantId, id: Option<Uuid>) -> CoreResult<()> {
    if let Some(id) = id {
        get(pool, tenant, id)
            .await
            .map_err(|_| CoreError::Validation("unknown audio clip".into()))?;
    }
    Ok(())
}

/// Where clips are used (`r` is the referencing row, `c` the clip);
/// unreferenced clips are cleaned up.
pub const REFERENCES: &[&str] = &[
    "voicemail_boxes r WHERE r.greeting_clip_id = c.id",
    "ivr_menus r WHERE c.id = ANY (r.clip_ids)",
];

/// Deletes clips nobody references that are older than a day (previews,
/// replaced greetings); returns `(tenant, id)` of the deleted clips so the
/// caller can remove their files.
pub async fn purge_unused(pool: &PgPool) -> CoreResult<Vec<(TenantId, Uuid)>> {
    let refs = REFERENCES
        .iter()
        .map(|r| format!("AND NOT EXISTS (SELECT 1 FROM {r})"))
        .collect::<Vec<_>>()
        .join("\n");
    let sql = format!(
        "DELETE FROM audio_clips c WHERE c.created_at < now() - interval '1 day' {refs}
         RETURNING c.tenant_id, c.id"
    );
    Ok(sqlx::query_as(&sql).fetch_all(pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices() {
        assert_eq!(voice("de", 2).name, "Kerstin");
        assert_eq!(voice("en", 1).model, "en_US-ljspeech-medium");
        assert_eq!(voice("de", 9).voice, 1, "unknown number: voice 1");
        assert_eq!(voice("fr", 1).language, "de");
        for lang in ["de", "en"] {
            assert_eq!(VOICES.iter().filter(|v| v.language == lang).count(), 2);
        }
    }
}
