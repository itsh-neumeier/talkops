//! Tenancy. Every domain table carries a `tenant_id`; the UI currently runs
//! single-tenant against [`TenantId::DEFAULT`].

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(pub Uuid);

impl TenantId {
    /// The tenant created by the initial migration.
    pub const DEFAULT: TenantId =
        TenantId(Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001));
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
