//! Call blocking: an own list of numbers and prefixes, anonymous callers,
//! and settings for the PhoneBlock community list (looked up by talkops-api).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct CallAllow {
    pub id: Uuid,
    pub number: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CallAllowInput {
    /// Exact international number. Prefix exceptions are not supported.
    pub number: String,
    #[serde(default)]
    pub label: String,
}

/// Exact E.164 syntax; do not interpret an extension or a prefix as a number.
pub fn normalize_allow_number(input: &str) -> CoreResult<String> {
    let number: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = number.strip_prefix('+').unwrap_or("");
    if !(3..=15).contains(&digits.len())
        || digits.starts_with('0')
        || !digits.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(CoreError::Validation(
            "number must be an exact international number, e.g. +4930123456".into(),
        ));
    }
    Ok(number)
}

/// Only recognize the marker written by the inbound dialplan.
pub fn valid_block_reason(reason: &str) -> bool {
    reason == "anonymous"
        || reason
            .strip_prefix("list:")
            .is_some_and(|p| normalize_pattern(p).is_ok_and(|normalized| normalized == p))
        || reason.strip_prefix("phoneblock:").is_some_and(|v| {
            !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()) && v.parse::<u32>().is_ok()
        })
}

pub async fn allowlist<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<Vec<CallAllow>> {
    Ok(sqlx::query_as(
        "SELECT id, number, label, created_at FROM call_allowlist WHERE tenant_id = $1 ORDER BY number",
    ).bind(tenant).fetch_all(db).await?)
}

/// Batch membership lookup, also used before the PhoneBlock cache lookup.
pub async fn allowed_numbers<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    numbers: &[String],
) -> CoreResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT number FROM call_allowlist WHERE tenant_id = $1 AND number = ANY($2)",
    )
    .bind(tenant)
    .bind(numbers)
    .fetch_all(db)
    .await?)
}

/// The audit entry and the exception are committed together. Duplicate requests
/// return the existing exception without changing its label or creating an audit.
pub async fn allow(
    db: &PgPool,
    actor: &crate::audit::Actor,
    input: &CallAllowInput,
) -> CoreResult<(CallAllow, bool)> {
    let number = normalize_allow_number(&input.number)?;
    let label: String = input.label.trim().chars().take(100).collect();
    let mut tx = db.begin().await?;
    let created: Option<CallAllow> = sqlx::query_as(
        "INSERT INTO call_allowlist (tenant_id, number, label) VALUES ($1, $2, $3)
         ON CONFLICT (tenant_id, number) DO NOTHING RETURNING id, number, label, created_at",
    )
    .bind(actor.tenant)
    .bind(&number)
    .bind(&label)
    .fetch_optional(&mut *tx)
    .await?;
    let is_new = created.is_some();
    let entry = match created {
        Some(entry) => {
            crate::audit::record(&mut *tx, actor, "create", "call_allowlist",
                Some(entry.id.to_string()), serde_json::json!({"number": number, "label": label})).await?;
            entry
        }
        None => sqlx::query_as(
            "SELECT id, number, label, created_at FROM call_allowlist WHERE tenant_id = $1 AND number = $2",
        ).bind(actor.tenant).bind(&number).fetch_one(&mut *tx).await?,
    };
    tx.commit().await?;
    Ok((entry, is_new))
}

pub async fn disallow(db: &PgPool, actor: &crate::audit::Actor, id: Uuid) -> CoreResult<()> {
    let mut tx = db.begin().await?;
    let number: String = sqlx::query_scalar(
        "DELETE FROM call_allowlist WHERE tenant_id = $1 AND id = $2 RETURNING number",
    )
    .bind(actor.tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    crate::audit::record(
        &mut *tx,
        actor,
        "delete",
        "call_allowlist",
        Some(id.to_string()),
        serde_json::json!({"number": number}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct CallBlock {
    pub id: Uuid,
    /// E.164 number (`+4930123456`) or prefix ending in `*` (`+49900*`).
    pub pattern: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CallBlockInput {
    pub pattern: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BlockSettings {
    pub block_anonymous: bool,
    pub phoneblock_enabled: bool,
    /// Whether an API token is stored (the token itself is never returned).
    pub phoneblock_token_set: bool,
    /// Spam reports needed before PhoneBlock blocks a number.
    pub phoneblock_min_votes: i32,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BlockSettingsInput {
    pub block_anonymous: bool,
    pub phoneblock_enabled: bool,
    /// New token; `None` keeps it, `""` removes it.
    #[serde(default)]
    pub phoneblock_token: Option<String>,
    #[serde(default = "default_votes")]
    pub phoneblock_min_votes: i32,
}

fn default_votes() -> i32 {
    4
}

#[derive(FromRow)]
struct SettingsRow {
    block_anonymous: bool,
    phoneblock_enabled: bool,
    phoneblock_token_enc: Option<String>,
    phoneblock_min_votes: i32,
}

/// Settings as the dialplan needs them, with the decrypted token.
#[derive(Debug, Clone, Default)]
pub struct EffectiveSettings {
    pub block_anonymous: bool,
    /// Set when PhoneBlock is enabled and a token is stored.
    pub phoneblock_token: Option<String>,
    pub phoneblock_min_votes: i32,
}

/// Normalises a pattern: digits with a leading `+`, optionally ending in `*`.
pub fn normalize_pattern(input: &str) -> CoreResult<String> {
    let trimmed: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let (body, prefix) = match trimmed.strip_suffix('*') {
        Some(b) => (b, true),
        None => (trimmed.as_str(), false),
    };
    let digits = body.strip_prefix('+').unwrap_or(body);
    let valid = body.starts_with('+')
        && (3..=16).contains(&digits.len())
        && digits.chars().all(|c| c.is_ascii_digit());
    if !valid {
        return Err(CoreError::Validation(
            "number must be international, e.g. +4930123456 or +49900* for a prefix".into(),
        ));
    }
    Ok(if prefix {
        format!("{body}*")
    } else {
        body.to_owned()
    })
}

/// Whether `caller` (E.164) matches `pattern`.
pub fn matches(pattern: &str, caller: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => caller.starts_with(prefix),
        None => pattern == caller,
    }
}

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<CallBlock>> {
    Ok(sqlx::query_as(
        "SELECT id, pattern, label, created_at FROM call_blocks
         WHERE tenant_id = $1 ORDER BY pattern",
    )
    .bind(tenant)
    .fetch_all(db)
    .await?)
}

pub async fn create<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    input: &CallBlockInput,
) -> CoreResult<CallBlock> {
    let pattern = normalize_pattern(&input.pattern)?;
    let label: String = input.label.trim().chars().take(100).collect();
    Ok(sqlx::query_as(
        "INSERT INTO call_blocks (tenant_id, pattern, label) VALUES ($1, $2, $3)
         RETURNING id, pattern, label, created_at",
    )
    .bind(tenant)
    .bind(pattern)
    .bind(label)
    .fetch_one(db)
    .await?)
}

pub async fn delete<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let done = sqlx::query("DELETE FROM call_blocks WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// The own-list entry matching `caller`, if any.
pub async fn find<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    caller: &str,
) -> CoreResult<Option<CallBlock>> {
    Ok(sqlx::query_as(
        "SELECT id, pattern, label, created_at FROM call_blocks
         WHERE tenant_id = $1
           AND (pattern = $2 OR (right(pattern, 1) = '*'
                AND starts_with($2, left(pattern, length(pattern) - 1))))
         ORDER BY length(pattern) DESC LIMIT 1",
    )
    .bind(tenant)
    .bind(caller)
    .fetch_optional(db)
    .await?)
}

async fn row<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Option<SettingsRow>> {
    Ok(sqlx::query_as(
        "SELECT block_anonymous, phoneblock_enabled, phoneblock_token_enc, phoneblock_min_votes
         FROM call_block_settings WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_optional(db)
    .await?)
}

fn view(r: Option<SettingsRow>) -> BlockSettings {
    match r {
        Some(r) => BlockSettings {
            block_anonymous: r.block_anonymous,
            phoneblock_enabled: r.phoneblock_enabled,
            phoneblock_token_set: r.phoneblock_token_enc.is_some(),
            phoneblock_min_votes: r.phoneblock_min_votes,
        },
        None => BlockSettings {
            block_anonymous: false,
            phoneblock_enabled: false,
            phoneblock_token_set: false,
            phoneblock_min_votes: default_votes(),
        },
    }
}

pub async fn get_settings<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
) -> CoreResult<BlockSettings> {
    Ok(view(row(db, tenant).await?))
}

pub async fn effective<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    secrets: &SecretBox,
) -> CoreResult<EffectiveSettings> {
    let Some(r) = row(db, tenant).await? else {
        return Ok(EffectiveSettings {
            phoneblock_min_votes: default_votes(),
            ..Default::default()
        });
    };
    let token = match (&r.phoneblock_token_enc, r.phoneblock_enabled) {
        (Some(enc), true) => Some(secrets.decrypt(enc)?),
        _ => None,
    };
    Ok(EffectiveSettings {
        block_anonymous: r.block_anonymous,
        phoneblock_token: token,
        phoneblock_min_votes: r.phoneblock_min_votes,
    })
}

pub async fn update_settings(
    db: &sqlx::PgPool,
    tenant: TenantId,
    input: &BlockSettingsInput,
    secrets: &SecretBox,
) -> CoreResult<BlockSettings> {
    if !(1..=100).contains(&input.phoneblock_min_votes) {
        return Err(CoreError::Validation("min votes must be 1 to 100".into()));
    }
    let token_enc = match input.phoneblock_token.as_deref().map(str::trim) {
        None => None,
        Some("") => Some(None),
        Some(t) => {
            if t.len() > 200 || !t.chars().all(|c| c.is_ascii_graphic()) {
                return Err(CoreError::Validation("invalid API token".into()));
            }
            Some(Some(secrets.encrypt(t)?))
        }
    };
    let current = row(db, tenant).await?;
    let token_enc = match token_enc {
        Some(new) => new,
        None => current.and_then(|r| r.phoneblock_token_enc),
    };
    if input.phoneblock_enabled && token_enc.is_none() {
        return Err(CoreError::Validation(
            "PhoneBlock needs an API token".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO call_block_settings
             (tenant_id, block_anonymous, phoneblock_enabled, phoneblock_token_enc, phoneblock_min_votes)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id) DO UPDATE SET block_anonymous = $2, phoneblock_enabled = $3,
             phoneblock_token_enc = $4, phoneblock_min_votes = $5, updated_at = now()",
    )
    .bind(tenant)
    .bind(input.block_anonymous)
    .bind(input.phoneblock_enabled)
    .bind(&token_enc)
    .bind(input.phoneblock_min_votes)
    .execute(db)
    .await?;
    get_settings(db, tenant).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_allow_numbers_and_stored_reasons() {
        assert_eq!(
            normalize_allow_number(" +49 30 123456 ").unwrap(),
            "+4930123456"
        );
        for bad in [
            "101",
            "anonymous",
            "+4930*",
            "004930123456",
            "+012345",
            "+1234567890123456",
            "+49${x}",
        ] {
            assert!(normalize_allow_number(bad).is_err(), "{bad}");
        }
        for reason in [
            "anonymous",
            "list:+4930123456",
            "list:+4930*",
            "phoneblock:4",
            "phoneblock:4294967295",
        ] {
            assert!(valid_block_reason(reason), "{reason}");
        }
        for reason in [
            "",
            "CALL_REJECTED",
            "list:101",
            "list:+49 ${x}",
            "phoneblock:-1",
            "phoneblock:",
            "phoneblock:4294967296",
        ] {
            assert!(!valid_block_reason(reason), "{reason}");
        }
    }

    #[test]
    fn patterns() {
        assert_eq!(normalize_pattern(" +49 30 123456 ").unwrap(), "+4930123456");
        assert_eq!(normalize_pattern("+49900*").unwrap(), "+49900*");
        assert!(normalize_pattern("030123456").is_err());
        assert!(normalize_pattern("+49${x}").is_err());
        assert!(normalize_pattern("+4*").is_err());
        assert!(matches("+49900*", "+499001234"));
        assert!(!matches("+49900*", "+4930900"));
        assert!(matches("+4930123", "+4930123"));
        assert!(!matches("+4930123", "+49301234"));
    }
}
