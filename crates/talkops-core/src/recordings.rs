//! Call recordings and transcripts (recordings and voicemails), full-text
//! search and retention.
//!
//! Recordings are stereo WAV files in the shared recordings volume
//! (`<tenant>/<yyyy-mm>/<call uuid>.wav`, left: caller, right: called party).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::cdr::Direction;
use crate::error::{CoreError, CoreResult};
use crate::extensions::Extension;
use crate::jobs::{self, NewJob};
use crate::settings::TenantSettings;
use crate::tenant::TenantId;

/// Job kind: transcribe a recording or voicemail (media worker).
pub const JOB_TRANSCRIBE: &str = "transcribe";

/// Should a call be recorded? `never` on any involved extension wins over
/// `always`, which wins over the tenant default for the call's direction.
pub fn should_record(
    settings: &TenantSettings,
    direction: Direction,
    extensions: &[&Extension],
) -> bool {
    if extensions.iter().any(|e| e.record_calls == "never") {
        return false;
    }
    if extensions.iter().any(|e| e.record_calls == "always") {
        return true;
    }
    match direction {
        Direction::Inbound => settings.record_inbound,
        Direction::Outbound => settings.record_outbound,
        Direction::Internal => settings.record_internal,
    }
}

/// Recording file for a call, relative to the recordings volume. `uuid` may
/// be the FreeSWITCH variable `${uuid}`.
pub fn recording_file(tenant: TenantId, now: DateTime<Utc>, uuid: &str) -> String {
    format!("{tenant}/{}/{uuid}.wav", now.format("%Y-%m"))
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Recording {
    pub id: Uuid,
    pub call_uuid: String,
    pub cdr_id: Option<Uuid>,
    #[serde(skip)]
    pub file: String,
    pub duration_secs: i32,
    pub size_bytes: i64,
    /// `none`, `pending`, `done` or `failed`.
    pub transcript_status: String,
    pub created_at: DateTime<Utc>,
}

const COLUMNS: &str =
    "id, call_uuid, cdr_id, file, duration_secs, size_bytes, transcript_status, created_at";

/// Stores a finished recording of a call (from the CDR) and queues its
/// transcription if enabled.
pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    call_uuid: &str,
    file: &str,
    duration_secs: i32,
    size_bytes: i64,
    transcribe: bool,
) -> CoreResult<Option<Recording>> {
    let mut tx = pool.begin().await?;
    let sql = format!(
        "INSERT INTO recordings (tenant_id, call_uuid, cdr_id, file, duration_secs, size_bytes,
                                 transcript_status)
         VALUES ($1, $2, (SELECT id FROM cdr WHERE call_uuid = $2 AND tenant_id = $1), $3, $4, $5, $6)
         ON CONFLICT (call_uuid) DO NOTHING RETURNING {COLUMNS}"
    );
    let rec: Option<Recording> = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(call_uuid)
        .bind(file)
        .bind(duration_secs)
        .bind(size_bytes)
        .bind(if transcribe { "pending" } else { "none" })
        .fetch_optional(&mut *tx)
        .await?;
    if let (Some(rec), true) = (&rec, transcribe) {
        let mut job = NewJob::new(
            JOB_TRANSCRIBE,
            serde_json::json!({ "recording_id": rec.id }),
        );
        job.tenant_id = tenant;
        jobs::enqueue(&mut *tx, job).await?;
    }
    tx.commit().await?;
    Ok(rec)
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Recording> {
    let sql = format!("SELECT {COLUMNS} FROM recordings WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
}

/// The extensions involved in a recorded call (for access checks).
pub async fn extensions_of(pool: &PgPool, recording: Uuid) -> CoreResult<Vec<Uuid>> {
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT c.extension_id, c.dest_extension_id FROM recordings r
         JOIN cdr c ON c.id = r.cdr_id WHERE r.id = $1",
    )
    .bind(recording)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|(a, b)| [a, b].into_iter().flatten().collect())
        .unwrap_or_default())
}

/// Deletes a recording (its transcript goes with it) and returns it so the
/// caller can remove the file.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<Recording> {
    let sql =
        format!("DELETE FROM recordings WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?)
}

/// Recordings past their tenant's retention period (oldest first).
pub async fn expired(pool: &PgPool, limit: i64) -> CoreResult<Vec<(TenantId, Recording)>> {
    #[derive(FromRow)]
    struct Row {
        tenant_id: TenantId,
        #[sqlx(flatten)]
        rec: Recording,
    }
    let sql = format!(
        "SELECT r.tenant_id, {cols} FROM recordings r
         JOIN tenant_settings s ON s.tenant_id = r.tenant_id
         WHERE s.recording_retention_days > 0
           AND r.created_at < now() - make_interval(days => s.recording_retention_days)
         ORDER BY r.created_at LIMIT $1",
        cols = COLUMNS
            .split(", ")
            .map(|c| format!("r.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let rows: Vec<Row> = sqlx::query_as(&sql).bind(limit).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| (r.tenant_id, r.rec)).collect())
}

// --- transcripts -----------------------------------------------------------------

/// What a transcript belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "id")]
pub enum Source {
    Recording(Uuid),
    Voicemail(Uuid),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Segment {
    pub start: f32,
    pub end: f32,
    /// `caller`, `called` or empty (mono recordings, voicemail).
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct Transcript {
    pub id: Uuid,
    pub recording_id: Option<Uuid>,
    pub voicemail_id: Option<Uuid>,
    pub language: String,
    pub text: String,
    #[schema(value_type = Vec<Segment>)]
    pub segments: sqlx::types::Json<Vec<Segment>>,
    /// e.g. `whisper:large-v3-turbo-q5_0` or `api:whisper-1`.
    pub engine: String,
    /// False while a more accurate second pass is still to come.
    #[sqlx(rename = "final")]
    #[serde(rename = "final")]
    pub is_final: bool,
    pub created_at: DateTime<Utc>,
}

const T_COLUMNS: &str =
    "id, recording_id, voicemail_id, language, text, segments, engine, final, created_at";

/// How a transcript was made.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptMeta<'a> {
    pub engine: &'a str,
    /// False if a second, more accurate pass follows.
    pub is_final: bool,
}

impl Default for TranscriptMeta<'_> {
    fn default() -> Self {
        Self {
            engine: "",
            is_final: true,
        }
    }
}

/// Stores a finished transcript (replacing an earlier pass) and marks its
/// source done.
pub async fn save_transcript(
    pool: &PgPool,
    tenant: TenantId,
    source: Source,
    language: &str,
    segments: &[Segment],
    meta: TranscriptMeta<'_>,
) -> CoreResult<Transcript> {
    let text = segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let (recording, voicemail) = match source {
        Source::Recording(id) => (Some(id), None),
        Source::Voicemail(id) => (None, Some(id)),
    };
    let mut tx = pool.begin().await?;
    // Replace an older transcript of the same source.
    sqlx::query("DELETE FROM transcripts WHERE recording_id = $1 OR voicemail_id = $2")
        .bind(recording)
        .bind(voicemail)
        .execute(&mut *tx)
        .await?;
    let sql = format!(
        "INSERT INTO transcripts (tenant_id, recording_id, voicemail_id, language, text, segments,
                                  engine, final)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {T_COLUMNS}"
    );
    let t: Transcript = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(recording)
        .bind(voicemail)
        .bind(language)
        .bind(&text)
        .bind(sqlx::types::Json(segments))
        .bind(meta.engine)
        .bind(meta.is_final)
        .fetch_one(&mut *tx)
        .await?;
    set_status(&mut tx, source, "done").await?;
    tx.commit().await?;
    Ok(t)
}

async fn set_status(db: &mut sqlx::PgConnection, source: Source, status: &str) -> CoreResult<()> {
    let (sql, id) = match source {
        Source::Recording(id) => (
            "UPDATE recordings SET transcript_status = $2 WHERE id = $1",
            id,
        ),
        Source::Voicemail(id) => (
            "UPDATE voicemail_messages SET transcript_status = $2 WHERE id = $1",
            id,
        ),
    };
    sqlx::query(sql).bind(id).bind(status).execute(db).await?;
    Ok(())
}

/// The second pass failed for good: the first transcript stays and is
/// no longer marked preliminary.
pub async fn finalize_transcript(pool: &PgPool, source: Source) -> CoreResult<()> {
    let (col, id) = match source {
        Source::Recording(id) => ("recording_id", id),
        Source::Voicemail(id) => ("voicemail_id", id),
    };
    sqlx::query(&format!(
        "UPDATE transcripts SET final = true WHERE {col} = $1"
    ))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks a transcription as failed.
pub async fn transcript_failed(pool: &PgPool, source: Source) -> CoreResult<()> {
    let mut conn = pool.acquire().await?;
    set_status(&mut conn, source, "failed").await
}

pub async fn transcript_of(
    pool: &PgPool,
    tenant: TenantId,
    source: Source,
) -> CoreResult<Transcript> {
    let (col, id) = match source {
        Source::Recording(id) => ("recording_id", id),
        Source::Voicemail(id) => ("voicemail_id", id),
    };
    let sql = format!("SELECT {T_COLUMNS} FROM transcripts WHERE tenant_id = $1 AND {col} = $2");
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(CoreError::NotFound)
}

/// A full-text search hit.
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct SearchHit {
    pub transcript_id: Uuid,
    pub recording_id: Option<Uuid>,
    pub voicemail_id: Option<Uuid>,
    /// Text around the matches; matches are wrapped in `[` `]`.
    pub snippet: String,
    pub created_at: DateTime<Utc>,
    pub caller_number: Option<String>,
    pub destination: Option<String>,
    pub rank: f32,
}

/// Searches transcripts. `extensions` limits hits to calls and voicemails
/// of these extensions (`None` = all, for admins).
pub async fn search(
    pool: &PgPool,
    tenant: TenantId,
    query: &str,
    extensions: Option<&[Uuid]>,
    limit: i64,
) -> CoreResult<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_as(
        "SELECT t.id AS transcript_id, t.recording_id, t.voicemail_id,
                ts_headline('simple', t.text, websearch_to_tsquery('simple', $2),
                            'StartSel=[, StopSel=], MaxFragments=2, MaxWords=18, MinWords=6') AS snippet,
                t.created_at,
                COALESCE(c.caller_number, m.caller_number) AS caller_number,
                COALESCE(c.destination, e.number) AS destination,
                ts_rank(t.search, websearch_to_tsquery('simple', $2)) AS rank
         FROM transcripts t
         LEFT JOIN recordings r ON r.id = t.recording_id
         LEFT JOIN cdr c ON c.id = r.cdr_id
         LEFT JOIN voicemail_messages m ON m.id = t.voicemail_id
         LEFT JOIN extensions e ON e.id = m.extension_id
         WHERE t.tenant_id = $1 AND t.search @@ websearch_to_tsquery('simple', $2)
           AND ($3::uuid[] IS NULL
                OR c.extension_id = ANY($3) OR c.dest_extension_id = ANY($3)
                OR m.extension_id = ANY($3))
         ORDER BY rank DESC, t.created_at DESC
         LIMIT $4",
    )
    .bind(tenant)
    .bind(q)
    .bind(extensions)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?)
}
