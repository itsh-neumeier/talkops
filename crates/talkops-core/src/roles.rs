//! Roles and their fine-grained permissions (ADR 0029).
//!
//! Each user has one role. The built-in `admin` role has every permission
//! and cannot be changed. The built-in `operator` and `user` roles and own
//! roles have a list of permissions that an admin can change. Access to own
//! data (own extensions, calls, recordings, voicemail) needs no permission.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::tenant::TenantId;
use crate::users::Role;

macro_rules! permissions {
    ($($variant:ident = $key:literal, $area:literal, $sensitive:literal;)*) => {
        /// A permission. The serialized form is its key, e.g. `calls.view_all`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, utoipa::ToSchema)]
        pub enum Permission {
            $(#[serde(rename = $key)] $variant,)*
        }

        impl Permission {
            /// All permissions in the order of the editor.
            pub const ALL: &'static [Permission] = &[$(Permission::$variant,)*];

            pub fn key(self) -> &'static str {
                match self { $(Permission::$variant => $key,)* }
            }

            /// Group of the permission in the editor.
            pub fn area(self) -> &'static str {
                match self { $(Permission::$variant => $area,)* }
            }

            /// Permissions that allow access to secrets, the system or more
            /// rights. The editor marks them.
            pub fn sensitive(self) -> bool {
                match self { $(Permission::$variant => $sensitive,)* }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key { $($key => Some(Permission::$variant),)* _ => None }
            }
        }
    };
}

permissions! {
    CallsActive = "calls.active", "calls", false;
    CallsViewAll = "calls.view_all", "calls", false;
    CallsLiveTranscript = "calls.live_transcript", "calls", false;
    CallsLiveTranscriptAll = "calls.live_transcript_all", "calls", false;
    RecordingsAccessAll = "recordings.access_all", "recordings", false;
    RecordingsDelete = "recordings.delete", "recordings", false;
    RecordingsRetranscribe = "recordings.retranscribe", "recordings", false;
    TranscriptionsView = "transcriptions.view", "recordings", false;
    TranscriptionsManage = "transcriptions.manage", "recordings", false;
    VoicemailAccessAll = "voicemail.access_all", "voicemail", false;
    VoicemailSettings = "voicemail.settings", "voicemail", false;
    PhonebookEdit = "phonebook.edit", "phonebook", false;
    ExtensionsView = "extensions.view", "extensions", false;
    ExtensionsManage = "extensions.manage", "extensions", false;
    SoftphoneAnyExtension = "softphone.any_extension", "extensions", false;
    UsersView = "users.view", "users", false;
    UsersManage = "users.manage", "users", true;
    RoutingView = "routing.view", "routing", false;
    RoutingManage = "routing.manage", "routing", false;
    RoutingOverride = "routing.override", "routing", false;
    BlockingManage = "blocking.manage", "routing", false;
    BlockingSettings = "blocking.settings", "routing", false;
    TrunksView = "trunks.view", "trunks", false;
    TrunksManage = "trunks.manage", "trunks", true;
    PhonesView = "phones.view", "phones", false;
    PhonesManage = "phones.manage", "phones", false;
    DoorsManage = "doors.manage", "doors", false;
    SettingsManage = "settings.manage", "system", false;
    TranscriptionSettings = "transcription.settings", "system", true;
    SecurityManage = "security.manage", "system", true;
    IdentityManage = "identity.manage", "system", true;
    BackupsManage = "backups.manage", "system", true;
    StorageManage = "storage.manage", "system", true;
    DiagnosticsManage = "diagnostics.manage", "system", true;
    NotificationsManage = "notifications.manage", "system", false;
    AuditView = "audit.view", "system", false;
    AuditManage = "audit.manage", "system", true;
}

/// Permissions of the built-in `operator` role after the migration (the
/// rights of the fixed operator role before roles could be changed).
pub const OPERATOR_DEFAULTS: &[Permission] = &[
    Permission::CallsActive,
    Permission::CallsViewAll,
    Permission::CallsLiveTranscript,
    Permission::PhonebookEdit,
    Permission::BlockingManage,
    Permission::RoutingView,
    Permission::RoutingOverride,
    Permission::ExtensionsView,
    Permission::UsersView,
    Permission::TrunksView,
    Permission::PhonesView,
];

/// A set of permissions; the admin role has all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions {
    all: bool,
    set: BTreeSet<Permission>,
}

impl Permissions {
    pub fn all() -> Self {
        Self {
            all: true,
            set: Permission::ALL.iter().copied().collect(),
        }
    }

    /// Permissions from stored keys; unknown keys are ignored.
    pub fn from_keys<S: AsRef<str>>(keys: &[S]) -> Self {
        Self {
            all: false,
            set: keys
                .iter()
                .filter_map(|k| Permission::from_key(k.as_ref()))
                .collect(),
        }
    }

    pub fn has(&self, p: Permission) -> bool {
        self.all || self.set.contains(&p)
    }

    pub fn is_all(&self) -> bool {
        self.all
    }

    /// True if every permission of `other` is also in this set.
    pub fn covers(&self, other: &Permissions) -> bool {
        self.all || (!other.all && other.set.is_subset(&self.set))
    }

    pub fn keys(&self) -> Vec<&'static str> {
        self.set.iter().map(|p| p.key()).collect()
    }
}

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct RoleRecord {
    pub id: Uuid,
    /// Built-in role, or none for an own role.
    pub builtin: Option<Role>,
    pub name: String,
    pub description: String,
    /// Permission keys. The admin role has all permissions; its list is empty.
    pub permissions: Vec<String>,
    pub directory_group: String,
    /// Number of users with this role.
    pub users: i64,
    pub updated_at: DateTime<Utc>,
}

impl RoleRecord {
    pub fn permission_set(&self) -> Permissions {
        if self.builtin == Some(Role::Admin) {
            Permissions::all()
        } else {
            Permissions::from_keys(&self.permissions)
        }
    }
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RoleInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub directory_group: String,
}

const COLUMNS: &str =
    "r.id, r.builtin, r.name, r.description, r.permissions, r.directory_group, r.updated_at,
     (SELECT count(*) FROM users u WHERE u.tenant_id = r.tenant_id AND
        (u.role_id = r.id OR (u.role_id IS NULL AND u.role = r.builtin))) AS users";

/// All roles: built-in roles first (admin, operator, user), then by name.
pub async fn list<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<Vec<RoleRecord>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM roles r WHERE r.tenant_id = $1
         ORDER BY CASE r.builtin WHEN 'admin' THEN 0 WHEN 'operator' THEN 1 WHEN 'user' THEN 2 ELSE 3 END,
                  lower(r.name)"
    );
    Ok(sqlx::query_as(&sql).bind(tenant).fetch_all(db).await?)
}

pub async fn get<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<RoleRecord> {
    let sql = format!("SELECT {COLUMNS} FROM roles r WHERE r.tenant_id = $1 AND r.id = $2");
    sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

fn validate(input: &RoleInput) -> CoreResult<(String, Vec<String>)> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(CoreError::Validation(
            "name must have 1 to 64 characters".into(),
        ));
    }
    let mut keys: Vec<String> = input
        .permissions
        .iter()
        .map(|p| p.key().to_owned())
        .collect();
    keys.sort();
    keys.dedup();
    Ok((name.to_owned(), keys))
}

pub async fn create(pool: &PgPool, tenant: TenantId, input: &RoleInput) -> CoreResult<RoleRecord> {
    let (name, keys) = validate(input)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (tenant_id, name, description, permissions, directory_group)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(&name)
    .bind(input.description.trim())
    .bind(&keys)
    .bind(input.directory_group.trim())
    .fetch_one(pool)
    .await?;
    get(pool, tenant, id).await
}

/// Changes a role. The admin role cannot be changed.
pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &RoleInput,
) -> CoreResult<RoleRecord> {
    let current = get(pool, tenant, id).await?;
    if current.builtin == Some(Role::Admin) {
        return Err(CoreError::Validation(
            "the administrator role cannot be changed".into(),
        ));
    }
    let (name, keys) = validate(input)?;
    sqlx::query(
        "UPDATE roles SET name = $3, description = $4, permissions = $5, directory_group = $6,
                          updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant)
    .bind(id)
    .bind(&name)
    .bind(input.description.trim())
    .bind(&keys)
    .bind(input.directory_group.trim())
    .execute(pool)
    .await?;
    get(pool, tenant, id).await
}

/// Deletes an own role. Built-in roles and roles with users stay.
pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<RoleRecord> {
    let current = get(pool, tenant, id).await?;
    if current.builtin.is_some() {
        return Err(CoreError::Validation(
            "built-in roles cannot be deleted".into(),
        ));
    }
    if current.users > 0 {
        return Err(CoreError::Conflict(format!(
            "the role has {} users",
            current.users
        )));
    }
    sqlx::query("DELETE FROM roles WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(current)
}

/// The stored form of a role choice: the built-in role for `users.role`
/// and the own role for `users.role_id`.
pub async fn assignment<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    id: Uuid,
) -> CoreResult<(Role, Option<Uuid>, RoleRecord)> {
    let role = get(db, tenant, id).await?;
    Ok(match role.builtin {
        Some(b) => (b, None, role),
        None => (Role::User, Some(role.id), role),
    })
}

/// Permissions of a user from the built-in role and the own role.
pub async fn of_user<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    role: Role,
    role_id: Option<Uuid>,
) -> CoreResult<Permissions> {
    if role == Role::Admin {
        return Ok(Permissions::all());
    }
    let keys: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT permissions FROM roles
         WHERE tenant_id = $1 AND (id = $2 OR ($2 IS NULL AND builtin = $3))",
    )
    .bind(tenant)
    .bind(role_id)
    .bind(role)
    .fetch_optional(db)
    .await?;
    Ok(Permissions::from_keys(&keys.unwrap_or_default()))
}

/// The own role whose directory group the user is in (first by name).
pub async fn directory_role<'e>(
    db: impl PgExecutor<'e>,
    tenant: TenantId,
    groups: &[String],
) -> CoreResult<Option<Uuid>> {
    if groups.is_empty() {
        return Ok(None);
    }
    let lower: Vec<String> = groups.iter().map(|g| g.to_lowercase()).collect();
    Ok(sqlx::query_scalar(
        "SELECT id FROM roles
         WHERE tenant_id = $1 AND builtin IS NULL AND directory_group <> ''
           AND lower(directory_group) = ANY($2)
         ORDER BY lower(name) LIMIT 1",
    )
    .bind(tenant)
    .bind(&lower)
    .fetch_optional(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_unknown_keys_are_ignored() {
        for p in Permission::ALL {
            assert_eq!(Permission::from_key(p.key()), Some(*p));
            assert_eq!(serde_json::to_value(p).unwrap(), serde_json::json!(p.key()));
        }
        let set = Permissions::from_keys(&["calls.view_all", "no.such"]);
        assert!(set.has(Permission::CallsViewAll));
        assert_eq!(set.keys(), ["calls.view_all"]);
    }

    #[test]
    fn admin_covers_everything_and_subsets_are_checked() {
        let all = Permissions::all();
        let op = Permissions::from_keys(
            &OPERATOR_DEFAULTS
                .iter()
                .map(|p| p.key())
                .collect::<Vec<_>>(),
        );
        assert!(all.covers(&op));
        assert!(!op.covers(&all));
        assert!(op.covers(&Permissions::from_keys(&["calls.active"])));
        assert!(!op.covers(&Permissions::from_keys(&["users.manage"])));
        assert!(Permission::ALL.iter().all(|p| all.has(*p)));
    }
}
