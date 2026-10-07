//! Logins against an LDAP directory (OpenLDAP, Active Directory, …):
//! search the user with the service account, then bind as the user with
//! the entered password. Group membership (`memberOf`) decides the role.

use std::time::Duration;

use ldap3::{LdapConnAsync, LdapConnSettings, Scope, SearchEntry, ldap_escape};
use talkops_core::identity::{ExternalAccount, IdentitySettings, map_role};

const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, thiserror::Error)]
pub enum LdapError {
    #[error("directory: {0}")]
    Ldap(#[from] ldap3::LdapError),
    #[error("{0}")]
    Config(String),
    /// Unknown user, ambiguous search result or wrong password.
    #[error("invalid credentials")]
    Invalid,
    /// Valid credentials, but not in an allowed group.
    #[error("not in an allowed group")]
    Forbidden,
}

/// Result of a directory search for one user.
#[derive(Debug, Clone)]
pub struct DirectoryUser {
    pub dn: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub groups: Vec<String>,
}

async fn connect(s: &IdentitySettings) -> Result<ldap3::Ldap, LdapError> {
    if !(s.ldap_url.starts_with("ldap://") || s.ldap_url.starts_with("ldaps://")) {
        return Err(LdapError::Config("no LDAP URL configured".into()));
    }
    crate::doors::ensure_tls_provider();
    let settings = LdapConnSettings::new()
        .set_conn_timeout(TIMEOUT)
        .set_starttls(s.ldap_starttls && s.ldap_url.starts_with("ldap://"));
    let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &s.ldap_url).await?;
    ldap3::drive!(conn);
    ldap.with_timeout(TIMEOUT);
    Ok(ldap)
}

fn first(entry: &SearchEntry, attr: &str) -> Option<String> {
    entry
        .attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(attr))
        .and_then(|(_, v)| v.first().cloned())
}

fn all(entry: &SearchEntry, attr: &str) -> Vec<String> {
    entry
        .attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(attr))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// Group values plus their CN, so `pbx-admins` matches
/// `CN=pbx-admins,OU=Groups,DC=example,DC=com`.
pub fn group_names(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for v in values {
        out.push(v.clone());
        if let Some(cn) = v
            .split(',')
            .next()
            .and_then(|rdn| rdn.split_once('='))
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case("cn"))
            .map(|(_, v)| v.trim().to_owned())
        {
            out.push(cn);
        }
    }
    out
}

/// Finds a user with the service account (anonymous if no bind DN).
async fn find(
    ldap: &mut ldap3::Ldap,
    s: &IdentitySettings,
    bind_password: Option<&str>,
    username: &str,
) -> Result<DirectoryUser, LdapError> {
    if !s.ldap_bind_dn.is_empty() {
        ldap.simple_bind(&s.ldap_bind_dn, bind_password.unwrap_or_default())
            .await?
            .success()?;
    }
    let filter = s
        .ldap_user_filter
        .replace("{username}", &ldap_escape(username));
    let attrs = [
        s.ldap_username_attr.as_str(),
        s.ldap_display_attr.as_str(),
        s.ldap_email_attr.as_str(),
        s.ldap_group_attr.as_str(),
    ];
    let (entries, _) = ldap
        .search(&s.ldap_base_dn, Scope::Subtree, &filter, attrs.to_vec())
        .await?
        .success()?;
    let mut entries: Vec<SearchEntry> = entries.into_iter().map(SearchEntry::construct).collect();
    // Referrals and the like have no DN; ignore them.
    entries.retain(|e| !e.dn.is_empty());
    if entries.len() != 1 {
        return Err(LdapError::Invalid);
    }
    let e = entries.remove(0);
    Ok(DirectoryUser {
        username: first(&e, &s.ldap_username_attr).unwrap_or_else(|| username.to_owned()),
        display_name: first(&e, &s.ldap_display_attr).unwrap_or_default(),
        email: first(&e, &s.ldap_email_attr),
        groups: all(&e, &s.ldap_group_attr),
        dn: e.dn,
    })
}

/// Checks a user's password against the directory.
pub async fn authenticate(
    s: &IdentitySettings,
    bind_password: Option<&str>,
    username: &str,
    password: &str,
) -> Result<ExternalAccount, LdapError> {
    // An empty password would be an unauthenticated bind, which succeeds.
    if password.is_empty() || username.trim().is_empty() {
        return Err(LdapError::Invalid);
    }
    let mut ldap = connect(s).await?;
    let user = find(&mut ldap, s, bind_password, username.trim()).await?;
    let ok = ldap
        .simple_bind(&user.dn, password)
        .await
        .map(|r| r.rc == 0)
        .unwrap_or(false);
    let _ = ldap.unbind().await;
    if !ok {
        return Err(LdapError::Invalid);
    }
    let role = map_role(s, &group_names(&user.groups)).ok_or(LdapError::Forbidden)?;
    Ok(ExternalAccount {
        source: "ldap",
        external_id: user.dn.to_lowercase(),
        username: user.username,
        display_name: user.display_name,
        email: user.email,
        role,
    })
}

/// Connection test for the settings page: binds with the service account
/// and, if given, looks up a user (without a password check).
pub async fn test(
    s: &IdentitySettings,
    bind_password: Option<&str>,
    username: Option<&str>,
) -> Result<Option<DirectoryUser>, LdapError> {
    let mut ldap = connect(s).await?;
    let result = match username.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => find(&mut ldap, s, bind_password, u).await.map(Some),
        None => {
            if !s.ldap_bind_dn.is_empty() {
                ldap.simple_bind(&s.ldap_bind_dn, bind_password.unwrap_or_default())
                    .await?
                    .success()?;
            }
            Ok(None)
        }
    };
    let _ = ldap.unbind().await;
    result
}

/// Hourly: disables directory accounts that were removed from the directory
/// or lost their group (and ends their sessions), and refreshes the role of
/// the others. An admin re-enables an account once it is allowed again.
pub async fn sync_users(
    pool: &sqlx::PgPool,
    secrets: &talkops_core::crypto::SecretBox,
) -> Result<(usize, usize), LdapError> {
    use talkops_core::tenant::TenantId;
    use talkops_core::{identity, users};
    let tenant = TenantId::DEFAULT;
    let s = identity::get(pool, tenant)
        .await
        .map_err(|e| LdapError::Config(e.to_string()))?;
    if !s.ldap_enabled {
        return Ok((0, 0));
    }
    let (_, bind_password) = identity::secrets(pool, tenant, secrets)
        .await
        .map_err(|e| LdapError::Config(e.to_string()))?;
    let accounts: Vec<(uuid::Uuid, String, users::Role)> = sqlx::query_as(
        "SELECT id, username, role FROM users
         WHERE tenant_id = $1 AND auth_source = 'ldap' AND enabled",
    )
    .bind(tenant)
    .fetch_all(pool)
    .await
    .map_err(|e| LdapError::Config(e.to_string()))?;
    if accounts.is_empty() {
        return Ok((0, 0));
    }
    let mut ldap = connect(&s).await?;
    let (mut disabled, mut changed) = (0, 0);
    for (id, username, role) in accounts {
        let wanted = match find(&mut ldap, &s, bind_password.as_deref(), &username).await {
            Ok(u) => map_role(&s, &group_names(&u.groups)),
            Err(LdapError::Invalid) => None,
            // Directory unreachable or misconfigured: change nothing.
            Err(err) => return Err(err),
        };
        let result = match wanted {
            None => {
                disabled += 1;
                tracing::info!(%username, "directory account removed or without group, disabling");
                sqlx::query("UPDATE users SET enabled = false, updated_at = now() WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await
                    .map(|_| ())
            }
            Some(r) if r != role => {
                changed += 1;
                sqlx::query("UPDATE users SET role = $2, updated_at = now() WHERE id = $1")
                    .bind(id)
                    .bind(r)
                    .execute(pool)
                    .await
                    .map(|_| ())
            }
            Some(_) => Ok(()),
        };
        result.map_err(|e| LdapError::Config(e.to_string()))?;
        if wanted.is_none() || wanted != Some(role) {
            // A changed role or a disabled account needs a fresh login.
            let _ = users::delete_user_sessions(pool, id).await;
        }
    }
    let _ = ldap.unbind().await;
    Ok((disabled, changed))
}

/// Runs [`sync_users`] every hour.
pub fn spawn_sync(pool: sqlx::PgPool, secrets: talkops_core::crypto::SecretBox) {
    tokio::spawn(async move {
        loop {
            match sync_users(&pool, &secrets).await {
                Ok((0, 0)) => {}
                Ok((d, c)) => tracing::info!(disabled = d, role_changed = c, "directory sync"),
                Err(err) => tracing::warn!(error = %err, "directory sync failed"),
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_cn_extraction() {
        let g = group_names(&[
            "CN=PBX-Admins,OU=Groups,DC=example,DC=com".into(),
            "staff".into(),
        ]);
        assert_eq!(
            g,
            [
                "CN=PBX-Admins,OU=Groups,DC=example,DC=com",
                "PBX-Admins",
                "staff"
            ]
        );
        assert_eq!(ldap_escape("a*(b)\\"), "a\\2a\\28b\\29\\5c");
    }
}
