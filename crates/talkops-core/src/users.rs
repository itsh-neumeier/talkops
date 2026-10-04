//! Users (web login identities) and their sessions.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::crypto;
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    sqlx::Type,
    utoipa::ToSchema,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Operator,
    Admin,
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct User {
    pub id: Uuid,
    #[serde(skip)]
    pub tenant_id: TenantId,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: Role,
    pub enabled: bool,
    pub auth_source: String,
    pub last_login_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub role: Role,
    /// Initial web password; `None` creates an account without local login.
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct UserUpdate {
    pub display_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub role: Role,
    pub enabled: bool,
}

const COLUMNS: &str = "id, tenant_id, username, display_name, email, role, enabled, auth_source, last_login_at, password_hash";

/// Minimum length for web passwords.
pub const MIN_PASSWORD_LEN: usize = 10;

fn check_password(password: &str) -> CoreResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(CoreError::Validation(format!(
            "password must have at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

pub async fn create<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    new: &NewUser,
) -> CoreResult<User> {
    let hash = match &new.password {
        Some(p) => {
            check_password(p)?;
            Some(crypto::hash_password(p)?)
        }
        None => None,
    };
    let sql = format!(
        "INSERT INTO users (tenant_id, username, display_name, email, role, password_hash)
         VALUES ($1, $2, $3, NULLIF($4, ''), $5, $6) RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(new.username.trim())
        .bind(new.display_name.trim())
        .bind(new.email.as_deref().map(str::trim))
        .bind(new.role)
        .bind(hash)
        .fetch_one(db)
        .await?)
}

/// Creates the first admin, but only while no user exists (setup wizard).
/// Serialized with an advisory lock so concurrent requests cannot both succeed.
pub async fn create_first_admin(
    pool: &PgPool,
    tenant: TenantId,
    new: &NewUser,
) -> CoreResult<User> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(7231001)")
        .execute(&mut *tx)
        .await?;
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE tenant_id = $1")
        .bind(tenant)
        .fetch_one(&mut *tx)
        .await?;
    if existing > 0 {
        return Err(CoreError::Conflict("setup already completed".into()));
    }
    let admin = NewUser {
        role: Role::Admin,
        ..new.clone()
    };
    if admin.password.is_none() {
        return Err(CoreError::Validation("password is required".into()));
    }
    let user = create(&mut *tx, tenant, &admin).await?;
    tx.commit().await?;
    Ok(user)
}

pub async fn count<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM users WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(db)
            .await?,
    )
}

pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<User>> {
    let sql =
        format!("SELECT {COLUMNS} FROM users WHERE tenant_id = $1 ORDER BY lower(display_name)");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<User> {
    let sql = format!("SELECT {COLUMNS} FROM users WHERE tenant_id = $1 AND id = $2");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(db)
        .await?)
}

pub async fn find_by_username<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    username: &str,
) -> CoreResult<Option<User>> {
    let sql =
        format!("SELECT {COLUMNS} FROM users WHERE tenant_id = $1 AND lower(username) = lower($2)");
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(username.trim())
        .fetch_optional(db)
        .await?)
}

pub async fn update<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    u: &UserUpdate,
) -> CoreResult<User> {
    let sql = format!(
        "UPDATE users SET display_name = $3, email = NULLIF($4, ''), role = $5, enabled = $6, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .bind(u.display_name.trim())
        .bind(u.email.as_deref().map(str::trim))
        .bind(u.role)
        .bind(u.enabled)
        .fetch_one(db)
        .await?)
}

pub async fn set_password<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
    password: &str,
) -> CoreResult<()> {
    check_password(password)?;
    let hash = crypto::hash_password(password)?;
    let res = sqlx::query(
        "UPDATE users SET password_hash = $3, updated_at = now() WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(hash)
    .execute(db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

pub async fn delete<'e>(db: impl PgExecutor<'e>, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let res = sqlx::query("DELETE FROM users WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Number of enabled admins; used to prevent locking everyone out.
pub async fn enabled_admin_count<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM users WHERE tenant_id = $1 AND role = 'admin' AND enabled",
    )
    .bind(tenant)
    .fetch_one(db)
    .await?)
}

/// Verifies credentials of a local account. Returns `None` for unknown users,
/// wrong passwords and disabled accounts alike.
pub async fn authenticate(
    pool: &PgPool,
    tenant: TenantId,
    username: &str,
    password: &str,
) -> CoreResult<Option<User>> {
    let user = find_by_username(pool, tenant, username).await?;
    let Some(user) = user else {
        // Spend comparable time on unknown users to avoid user enumeration by timing.
        let _ = crypto::verify_password(password, dummy_hash());
        return Ok(None);
    };
    let ok = user.enabled
        && user
            .password_hash
            .as_deref()
            .is_some_and(|h| crypto::verify_password(password, h));
    if !ok {
        return Ok(None);
    }
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user.id)
        .execute(pool)
        .await?;
    Ok(Some(user))
}

/// A real argon2id hash with default parameters, used for timing equalization.
fn dummy_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| crypto::hash_password("timing-equalization").unwrap_or_default())
}

// --- sessions ----------------------------------------------------------------

/// Idle sessions expire after this time; activity extends them.
pub const SESSION_TTL_HOURS: i64 = 12;

#[derive(Debug, Clone)]
pub struct NewSession {
    /// Raw token for the cookie; only its digest is stored.
    pub token: String,
    pub csrf_token: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionUser {
    pub user_id: Uuid,
    pub tenant_id: TenantId,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub csrf_token: String,
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    ip: Option<&str>,
    user_agent: Option<&str>,
) -> CoreResult<NewSession> {
    let token = crypto::random_token(32)?;
    let csrf_token = crypto::random_token(24)?;
    sqlx::query(
        "INSERT INTO sessions (token_digest, user_id, csrf_token, ip, user_agent, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(crypto::token_digest(&token))
    .bind(user_id)
    .bind(&csrf_token)
    .bind(ip)
    .bind(user_agent.map(|ua| ua.chars().take(255).collect::<String>()))
    .bind(Utc::now() + Duration::hours(SESSION_TTL_HOURS))
    .execute(pool)
    .await?;
    Ok(NewSession { token, csrf_token })
}

/// Resolves a session token and slides its expiry. Disabled users have no session.
pub async fn session_user(pool: &PgPool, token: &str) -> CoreResult<Option<SessionUser>> {
    Ok(sqlx::query_as(
        "UPDATE sessions s SET last_seen_at = now(), expires_at = now() + make_interval(hours => $2)
         FROM users u
         WHERE s.token_digest = $1 AND s.expires_at > now() AND u.id = s.user_id AND u.enabled
         RETURNING u.id AS user_id, u.tenant_id, u.username, u.display_name, u.role, s.csrf_token",
    )
    .bind(crypto::token_digest(token))
    .bind(SESSION_TTL_HOURS as i32)
    .fetch_optional(pool)
    .await?)
}

pub async fn delete_session(pool: &PgPool, token: &str) -> CoreResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_digest = $1")
        .bind(crypto::token_digest(token))
        .execute(pool)
        .await?;
    Ok(())
}

/// Ends all sessions of a user (password change, disable, delete).
pub async fn delete_user_sessions<'e>(db: impl PgExecutor<'e>, user_id: Uuid) -> CoreResult<()> {
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn purge_expired_sessions(pool: &PgPool) -> CoreResult<u64> {
    Ok(
        sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
