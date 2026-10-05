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
                       emergency_numbers, external_ip, default_language, default_number_id, timezone";

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
             default_language = $8, default_number_id = $9, timezone = $10, updated_at = now()
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
        .fetch_one(db)
        .await?)
}
