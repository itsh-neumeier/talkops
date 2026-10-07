//! External identities: single sign-on with OpenID Connect and logins
//! against an LDAP/Active Directory server. The directory decides who may
//! log in and with which role (group membership); TalkOps keeps a user row
//! per external account (matched by a stable id, never by user name).

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto::{self, SecretBox};
use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;
use crate::users::{Role, User};

/// Identity settings as shown to admins (secrets only as flags).
#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct IdentitySettings {
    pub public_url: String,
    pub oidc_enabled: bool,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    pub oidc_has_secret: bool,
    pub oidc_scopes: String,
    pub oidc_username_claim: String,
    pub oidc_groups_claim: String,
    pub oidc_button_label: String,
    pub ldap_enabled: bool,
    pub ldap_url: String,
    pub ldap_starttls: bool,
    pub ldap_bind_dn: String,
    pub ldap_has_password: bool,
    pub ldap_base_dn: String,
    pub ldap_user_filter: String,
    pub ldap_username_attr: String,
    pub ldap_display_attr: String,
    pub ldap_email_attr: String,
    pub ldap_group_attr: String,
    pub admin_group: String,
    pub operator_group: String,
    pub user_group: String,
}

const COLUMNS: &str = "public_url, oidc_enabled, oidc_issuer, oidc_client_id, \
     oidc_client_secret_enc IS NOT NULL AS oidc_has_secret, oidc_scopes, oidc_username_claim, \
     oidc_groups_claim, oidc_button_label, ldap_enabled, ldap_url, ldap_starttls, ldap_bind_dn, \
     ldap_bind_password_enc IS NOT NULL AS ldap_has_password, ldap_base_dn, ldap_user_filter, \
     ldap_username_attr, ldap_display_attr, ldap_email_attr, ldap_group_attr, admin_group, \
     operator_group, user_group";

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct IdentityInput {
    #[serde(default)]
    pub public_url: String,
    #[serde(default)]
    pub oidc_enabled: bool,
    #[serde(default)]
    pub oidc_issuer: String,
    #[serde(default)]
    pub oidc_client_id: String,
    /// `None` keeps the stored secret, `""` removes it.
    #[serde(default)]
    pub oidc_client_secret: Option<String>,
    #[serde(default = "default_scopes")]
    pub oidc_scopes: String,
    #[serde(default = "default_username_claim")]
    pub oidc_username_claim: String,
    #[serde(default = "default_groups_claim")]
    pub oidc_groups_claim: String,
    #[serde(default)]
    pub oidc_button_label: String,
    #[serde(default)]
    pub ldap_enabled: bool,
    #[serde(default)]
    pub ldap_url: String,
    #[serde(default)]
    pub ldap_starttls: bool,
    #[serde(default)]
    pub ldap_bind_dn: String,
    /// `None` keeps the stored password, `""` removes it.
    #[serde(default)]
    pub ldap_bind_password: Option<String>,
    #[serde(default)]
    pub ldap_base_dn: String,
    #[serde(default = "default_filter")]
    pub ldap_user_filter: String,
    #[serde(default = "default_uid")]
    pub ldap_username_attr: String,
    #[serde(default = "default_cn")]
    pub ldap_display_attr: String,
    #[serde(default = "default_mail")]
    pub ldap_email_attr: String,
    #[serde(default = "default_member_of")]
    pub ldap_group_attr: String,
    #[serde(default)]
    pub admin_group: String,
    #[serde(default)]
    pub operator_group: String,
    #[serde(default)]
    pub user_group: String,
}

fn default_scopes() -> String {
    "openid profile email".into()
}
fn default_username_claim() -> String {
    "preferred_username".into()
}
fn default_groups_claim() -> String {
    "groups".into()
}
fn default_filter() -> String {
    "(&(objectClass=person)(uid={username}))".into()
}
fn default_uid() -> String {
    "uid".into()
}
fn default_cn() -> String {
    "cn".into()
}
fn default_mail() -> String {
    "mail".into()
}
fn default_member_of() -> String {
    "memberOf".into()
}

fn valid_attr(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 64
        && a.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

fn validate(i: &IdentityInput) -> CoreResult<()> {
    let invalid = |m: &str| Err(CoreError::Validation(m.into()));
    if !i.public_url.is_empty()
        && !(i.public_url.starts_with("https://") || i.public_url.starts_with("http://"))
    {
        return invalid("the public URL must start with http:// or https://");
    }
    if i.oidc_enabled {
        if !i.oidc_issuer.starts_with("https://") && !i.oidc_issuer.starts_with("http://") {
            return invalid("the OIDC issuer must be an http(s) URL");
        }
        if i.oidc_client_id.trim().is_empty() {
            return invalid("the OIDC client ID is missing");
        }
        if !i.oidc_scopes.split_whitespace().any(|s| s == "openid") {
            return invalid("the scopes must contain openid");
        }
    }
    for claim in [&i.oidc_username_claim, &i.oidc_groups_claim] {
        if !valid_attr(claim) {
            return invalid("invalid claim name");
        }
    }
    if i.ldap_enabled {
        if !(i.ldap_url.starts_with("ldap://") || i.ldap_url.starts_with("ldaps://")) {
            return invalid("the LDAP URL must start with ldap:// or ldaps://");
        }
        if i.ldap_base_dn.trim().is_empty() {
            return invalid("the LDAP base DN is missing");
        }
        if !i.ldap_user_filter.contains("{username}")
            || !i.ldap_user_filter.starts_with('(')
            || !i.ldap_user_filter.ends_with(')')
        {
            return invalid("the LDAP filter must be in parentheses and contain {username}");
        }
    }
    for attr in [
        &i.ldap_username_attr,
        &i.ldap_display_attr,
        &i.ldap_email_attr,
        &i.ldap_group_attr,
    ] {
        if !valid_attr(attr) {
            return invalid("invalid LDAP attribute name");
        }
    }
    Ok(())
}

pub async fn get(pool: &PgPool, tenant: TenantId) -> CoreResult<IdentitySettings> {
    let sql = format!("SELECT {COLUMNS} FROM identity_settings WHERE tenant_id = $1");
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_one(pool).await?)
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    i: &IdentityInput,
) -> CoreResult<IdentitySettings> {
    validate(i)?;
    let enc = |v: Option<&str>| -> CoreResult<Option<Option<String>>> {
        Ok(match v {
            None => None,
            Some("") => Some(None),
            Some(v) => Some(Some(secrets.encrypt(v)?)),
        })
    };
    let oidc_secret = enc(i.oidc_client_secret.as_deref())?;
    let ldap_pw = enc(i.ldap_bind_password.as_deref())?;
    let sql = format!(
        "UPDATE identity_settings SET public_url = $2, oidc_enabled = $3, oidc_issuer = $4,
             oidc_client_id = $5,
             oidc_client_secret_enc = CASE WHEN $6 THEN $7 ELSE oidc_client_secret_enc END,
             oidc_scopes = $8, oidc_username_claim = $9, oidc_groups_claim = $10,
             oidc_button_label = $11, ldap_enabled = $12, ldap_url = $13, ldap_starttls = $14,
             ldap_bind_dn = $15,
             ldap_bind_password_enc = CASE WHEN $16 THEN $17 ELSE ldap_bind_password_enc END,
             ldap_base_dn = $18, ldap_user_filter = $19, ldap_username_attr = $20,
             ldap_display_attr = $21, ldap_email_attr = $22, ldap_group_attr = $23,
             admin_group = $24, operator_group = $25, user_group = $26, updated_at = now()
         WHERE tenant_id = $1 RETURNING {COLUMNS}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(tenant)
        .bind(i.public_url.trim().trim_end_matches('/'))
        .bind(i.oidc_enabled)
        .bind(i.oidc_issuer.trim().trim_end_matches('/'))
        .bind(i.oidc_client_id.trim())
        .bind(oidc_secret.is_some())
        .bind(oidc_secret.flatten())
        .bind(i.oidc_scopes.trim())
        .bind(i.oidc_username_claim.trim())
        .bind(i.oidc_groups_claim.trim())
        .bind(i.oidc_button_label.trim())
        .bind(i.ldap_enabled)
        .bind(i.ldap_url.trim())
        .bind(i.ldap_starttls)
        .bind(i.ldap_bind_dn.trim())
        .bind(ldap_pw.is_some())
        .bind(ldap_pw.flatten())
        .bind(i.ldap_base_dn.trim())
        .bind(i.ldap_user_filter.trim())
        .bind(i.ldap_username_attr.trim())
        .bind(i.ldap_display_attr.trim())
        .bind(i.ldap_email_attr.trim())
        .bind(i.ldap_group_attr.trim())
        .bind(i.admin_group.trim())
        .bind(i.operator_group.trim())
        .bind(i.user_group.trim())
        .fetch_one(pool)
        .await?)
}

/// Decrypted secrets: (OIDC client secret, LDAP bind password).
pub async fn secrets(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
) -> CoreResult<(Option<String>, Option<String>)> {
    let (oidc, ldap): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT oidc_client_secret_enc, ldap_bind_password_enc FROM identity_settings
         WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await?;
    Ok((
        oidc.map(|v| secrets.decrypt(&v)).transpose()?,
        ldap.map(|v| secrets.decrypt(&v)).transpose()?,
    ))
}

/// Role for a directory account from its groups; `None` = not allowed.
/// Group names compare case-insensitively (LDAP DNs often differ in case).
pub fn map_role(s: &IdentitySettings, groups: &[String]) -> Option<Role> {
    let member = |g: &str| !g.is_empty() && groups.iter().any(|x| x.eq_ignore_ascii_case(g));
    if member(&s.admin_group) {
        Some(Role::Admin)
    } else if member(&s.operator_group) {
        Some(Role::Operator)
    } else if s.user_group.is_empty() || member(&s.user_group) {
        Some(Role::User)
    } else {
        None
    }
}

/// An authenticated directory account.
#[derive(Debug, Clone)]
pub struct ExternalAccount {
    /// `oidc` or `ldap`.
    pub source: &'static str,
    /// Stable id (OIDC `iss|sub`, LDAP DN).
    pub external_id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: Role,
}

/// Makes a user name valid for TalkOps (`[a-zA-Z0-9._@-]{1,64}`).
pub fn clean_username(raw: &str) -> String {
    let u: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "._@-".contains(*c))
        .take(64)
        .collect();
    if u.is_empty() { "user".into() } else { u }
}

/// Finds or creates the user of an external account and refreshes name,
/// e-mail and role from the directory. A local account with the same user
/// name is never taken over.
pub async fn upsert_user(pool: &PgPool, tenant: TenantId, a: &ExternalAccount) -> CoreResult<User> {
    let username = clean_username(&a.username);
    let display = if a.display_name.trim().is_empty() {
        username.clone()
    } else {
        a.display_name.trim().chars().take(128).collect()
    };
    let email = a.email.as_deref().map(str::trim).filter(|e| !e.is_empty());
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE tenant_id = $1 AND auth_source = $2 AND external_id = $3",
    )
    .bind(tenant)
    .bind(a.source)
    .bind(&a.external_id)
    .fetch_optional(pool)
    .await?;
    let id = match existing {
        Some(id) => {
            sqlx::query(
                "UPDATE users SET display_name = $2, email = $3, role = $4, last_login_at = now(),
                     updated_at = now()
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&display)
            .bind(email)
            .bind(a.role)
            .execute(pool)
            .await?;
            id
        }
        None => {
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM users WHERE tenant_id = $1 AND lower(username) = lower($2))",
            )
            .bind(tenant)
            .bind(&username)
            .fetch_one(pool)
            .await?;
            if taken {
                return Err(CoreError::Conflict(format!(
                    "user name {username} is already used by another account"
                )));
            }
            sqlx::query_scalar(
                "INSERT INTO users (tenant_id, username, display_name, email, role, auth_source,
                                    external_id, last_login_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, now()) RETURNING id",
            )
            .bind(tenant)
            .bind(&username)
            .bind(&display)
            .bind(email)
            .bind(a.role)
            .bind(a.source)
            .bind(&a.external_id)
            .fetch_one(pool)
            .await?
        }
    };
    crate::users::get(pool, tenant, id).await
}

// --- OIDC login state ----------------------------------------------------------------

/// Data kept between the redirect to the provider and its callback.
#[derive(Debug, Clone, FromRow)]
pub struct PendingLogin {
    pub tenant_id: TenantId,
    pub verifier: String,
    pub nonce: String,
    pub redirect_uri: String,
}

/// Stores a pending login; returns the `state` value for the redirect.
pub async fn begin_oidc(
    pool: &PgPool,
    tenant: TenantId,
    verifier: &str,
    nonce: &str,
    redirect_uri: &str,
) -> CoreResult<String> {
    let state = crypto::random_token(24)?;
    sqlx::query("DELETE FROM oidc_logins WHERE expires_at < now()")
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO oidc_logins (state_digest, tenant_id, verifier, nonce, redirect_uri, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(crypto::token_digest(&state))
    .bind(tenant)
    .bind(verifier)
    .bind(nonce)
    .bind(redirect_uri)
    .bind(Utc::now() + Duration::minutes(10))
    .execute(pool)
    .await?;
    Ok(state)
}

/// Takes (and removes) the pending login of a `state` value.
pub async fn take_oidc(pool: &PgPool, state: &str) -> CoreResult<Option<PendingLogin>> {
    Ok(sqlx::query_as(
        "DELETE FROM oidc_logins WHERE state_digest = $1 AND expires_at > now()
         RETURNING tenant_id, verifier, nonce, redirect_uri",
    )
    .bind(crypto::token_digest(state))
    .fetch_optional(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(admin: &str, operator: &str, user: &str) -> IdentitySettings {
        IdentitySettings {
            public_url: String::new(),
            oidc_enabled: true,
            oidc_issuer: String::new(),
            oidc_client_id: String::new(),
            oidc_has_secret: false,
            oidc_scopes: default_scopes(),
            oidc_username_claim: default_username_claim(),
            oidc_groups_claim: default_groups_claim(),
            oidc_button_label: String::new(),
            ldap_enabled: false,
            ldap_url: String::new(),
            ldap_starttls: false,
            ldap_bind_dn: String::new(),
            ldap_has_password: false,
            ldap_base_dn: String::new(),
            ldap_user_filter: default_filter(),
            ldap_username_attr: default_uid(),
            ldap_display_attr: default_cn(),
            ldap_email_attr: default_mail(),
            ldap_group_attr: default_member_of(),
            admin_group: admin.into(),
            operator_group: operator.into(),
            user_group: user.into(),
        }
    }

    #[test]
    fn maps_roles_from_groups() {
        let g = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let s = settings("pbx-admins", "cn=Ops,dc=x", "");
        assert_eq!(map_role(&s, &g(&["PBX-Admins"])), Some(Role::Admin));
        assert_eq!(map_role(&s, &g(&["CN=ops,DC=x"])), Some(Role::Operator));
        assert_eq!(map_role(&s, &g(&[])), Some(Role::User));
        let s = settings("", "", "staff");
        assert_eq!(map_role(&s, &g(&["guests"])), None);
        assert_eq!(map_role(&s, &g(&["staff"])), Some(Role::User));
        assert_eq!(clean_username("Anna Müller@corp"), "AnnaMller@corp");
        assert_eq!(clean_username("äöü"), "user");
    }
}
