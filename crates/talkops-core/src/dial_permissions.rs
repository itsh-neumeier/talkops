//! Outbound dialing permissions (ADR 0045).
//!
//! Tenant-wide dialing profiles decide which numbers an extension can call.
//! A profile has a base mode and rules for countries, E.164 prefixes and
//! single numbers. [`evaluate`] makes the decision without I/O; the
//! functions below it load and store profiles.
//!
//! Emergency numbers are never evaluated: the dialplan sends them out
//! before it looks at a profile.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool};
use uuid::Uuid;

use crate::audit::Actor;
use crate::dialing::{DialPlanSettings, Dialed};
use crate::error::{CoreError, CoreResult};
use crate::geo::GeoIndex;
use crate::tenant::TenantId;

/// Most rules in one profile.
pub const MAX_RULES: usize = 500;

/// Allow or block; also the base mode of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Deny,
}

impl RuleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    fn parse(text: &str) -> CoreResult<Self> {
        match text {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            other => Err(CoreError::Validation(format!("unknown action {other}"))),
        }
    }
}

/// What a rule compares with the number. The order is the rank: a
/// `number` rule wins over a `prefix` rule, which wins over a `country` rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    /// ISO 3166-1 alpha-2 code of the number's country (geolocation data).
    Country,
    /// E.164 prefix, e.g. `+49900`.
    Prefix,
    /// One complete E.164 number.
    Number,
}

impl RuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Country => "country",
            Self::Prefix => "prefix",
            Self::Number => "number",
        }
    }

    fn parse(text: &str) -> CoreResult<Self> {
        match text {
            "country" => Ok(Self::Country),
            "prefix" => Ok(Self::Prefix),
            "number" => Ok(Self::Number),
            other => Err(CoreError::Validation(format!("unknown rule kind {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct DialingRule {
    pub id: Uuid,
    pub kind: RuleKind,
    /// `DE`, `+49900` or `+4930123456`.
    pub value: String,
    pub action: RuleAction,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct DialingProfile {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// The decision for numbers that no rule matches.
    pub base_mode: RuleAction,
    /// SIP address dialing (`user@host` over the Internet).
    pub sip_addresses: bool,
    /// The profile of extensions without their own profile.
    pub is_default: bool,
    pub rules: Vec<DialingRule>,
    /// Extensions that have this profile as their own profile.
    pub extensions: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DialingRuleInput {
    pub kind: RuleKind,
    /// `DE`, `+49900` (also `+49900*`) or `+4930123456`.
    pub value: String,
    pub action: RuleAction,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DialingProfileInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub base_mode: RuleAction,
    #[serde(default = "yes")]
    pub sip_addresses: bool,
    /// All rules of the profile; an update replaces the rules.
    #[serde(default)]
    pub rules: Vec<DialingRuleInput>,
}

fn yes() -> bool {
    true
}

/// A rule of the premium-rate block list (ADR 0045). The migration
/// `20261110000001_dialing_permissions.sql` seeds the same list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct PremiumRule {
    pub prefix: &'static str,
    pub label: &'static str,
    pub source: &'static str,
}

const BNETZA: &str = "Bundesnetzagentur, numbering plan";
const RTR: &str = "RTR, KEM-V 2009";
const BAKOM: &str = "BAKOM/OFCOM, E.164 numbering plan of Switzerland";
const ITU: &str = "ITU-T, list of E.164 assigned country codes";

/// Premium-rate numbers, directory services and international networks.
pub const PREMIUM_RULES: &[PremiumRule] = &[
    PremiumRule {
        prefix: "+49900",
        label: "DE premium-rate services (0900)",
        source: BNETZA,
    },
    PremiumRule {
        prefix: "+49137",
        label: "DE mass-traffic services (0137)",
        source: BNETZA,
    },
    PremiumRule {
        prefix: "+49118",
        label: "DE directory enquiry services (118xy)",
        source: BNETZA,
    },
    PremiumRule {
        prefix: "+4912",
        label: "DE innovative services (012)",
        source: BNETZA,
    },
    PremiumRule {
        prefix: "+4919",
        label: "DE online services (019)",
        source: BNETZA,
    },
    PremiumRule {
        prefix: "+43900",
        label: "AT value-added services (0900)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+43901",
        label: "AT value-added services (0901)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+43930",
        label: "AT value-added services (0930)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+43931",
        label: "AT value-added services (0931)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+43939",
        label: "AT value-added services (0939)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+43118",
        label: "AT directory enquiry services (118)",
        source: RTR,
    },
    PremiumRule {
        prefix: "+41900",
        label: "CH value-added services (0900)",
        source: BAKOM,
    },
    PremiumRule {
        prefix: "+41901",
        label: "CH value-added services (0901)",
        source: BAKOM,
    },
    PremiumRule {
        prefix: "+41906",
        label: "CH value-added services (0906)",
        source: BAKOM,
    },
    PremiumRule {
        prefix: "+870",
        label: "Inmarsat (+870)",
        source: ITU,
    },
    PremiumRule {
        prefix: "+881",
        label: "Satellite networks (+881)",
        source: ITU,
    },
    PremiumRule {
        prefix: "+882",
        label: "International networks (+882)",
        source: ITU,
    },
    PremiumRule {
        prefix: "+883",
        label: "International networks (+883)",
        source: ITU,
    },
];

// --- evaluation -----------------------------------------------------------------------

/// The result of [`evaluate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct Decision {
    pub allowed: bool,
    /// The rule that decided; none: the base mode (or no profile).
    pub rule: Option<DialingRule>,
}

impl Decision {
    fn allowed() -> Self {
        Self {
            allowed: true,
            rule: None,
        }
    }

    /// Reason for a blocked call in the call record and the log:
    /// `dialing:base` or `dialing:<kind>:<value>`.
    pub fn reason(&self) -> String {
        match &self.rule {
            Some(rule) => format!("dialing:{}:{}", rule.kind.as_str(), rule.value),
            None => "dialing:base".to_owned(),
        }
    }
}

/// Reason for a blocked SIP address call.
pub const SIP_REASON: &str = "dialing:sip";

/// Only recognize the reasons the dialplan writes (see [`Decision::reason`]).
pub fn valid_block_reason(reason: &str) -> bool {
    let Some(rest) = reason.strip_prefix("dialing:") else {
        return false;
    };
    if rest == "base" || rest == "sip" {
        return true;
    }
    let Some((kind, value)) = rest.split_once(':') else {
        return false;
    };
    RuleKind::parse(kind)
        .and_then(|kind| normalize_value(kind, value))
        .is_ok_and(|normalized| normalized == value)
}

/// Decides if `e164` can be called with `profile`. `country` is the ISO
/// code of the number from the geolocation data, if known.
///
/// The most specific matching rule wins: a `number` rule, else the longest
/// matching `prefix` rule, else a `country` rule, else the base mode. When
/// rules of the same rank match, `deny` wins.
pub fn evaluate(profile: &DialingProfile, e164: &str, country: Option<&str>) -> Decision {
    let best = |kind: RuleKind, specificity: &dyn Fn(&DialingRule) -> Option<usize>| {
        let matches: Vec<(usize, &DialingRule)> = profile
            .rules
            .iter()
            .filter(|r| r.kind == kind)
            .filter_map(|r| specificity(r).map(|s| (s, r)))
            .collect();
        let top = matches.iter().map(|(s, _)| *s).max()?;
        let mut tied = matches
            .into_iter()
            .filter(|(s, _)| *s == top)
            .map(|(_, r)| r);
        let first = tied.next()?;
        Some(
            std::iter::once(first)
                .chain(tied)
                .find(|r| r.action == RuleAction::Deny)
                .unwrap_or(first),
        )
    };
    let rule = best(RuleKind::Number, &|r| (r.value == e164).then_some(0))
        .or_else(|| {
            best(RuleKind::Prefix, &|r| {
                e164.starts_with(&r.value).then_some(r.value.len())
            })
        })
        .or_else(|| {
            let country = country?;
            best(RuleKind::Country, &|r| {
                r.value.eq_ignore_ascii_case(country).then_some(0)
            })
        });
    match rule {
        Some(rule) => Decision {
            allowed: rule.action == RuleAction::Allow,
            rule: Some(rule.clone()),
        },
        None => Decision {
            allowed: profile.base_mode == RuleAction::Allow,
            rule: None,
        },
    }
}

/// [`evaluate`] with the country from the geolocation data. Without a
/// profile (a tenant without a default profile) all numbers are allowed.
pub fn check(profile: Option<&DialingProfile>, e164: &str, geo: &GeoIndex) -> Decision {
    match profile {
        Some(profile) => evaluate(
            profile,
            e164,
            geo.lookup(e164).map(|area| area.country.as_str()),
        ),
        None => Decision::allowed(),
    }
}

/// True if the profile allows SIP address dialing.
pub fn sip_allowed(profile: Option<&DialingProfile>) -> bool {
    profile.is_none_or(|p| p.sip_addresses)
}

/// The number a profile evaluates for dialed digits: the E.164 number of an
/// external number, `+<country code><digits>` for a service number. `None`
/// for emergency numbers (never blocked) and invalid input.
pub fn number_to_check(dial_plan: &DialPlanSettings, dialed: &Dialed) -> Option<String> {
    match dialed {
        Dialed::External(e164) => Some(e164.clone()),
        Dialed::Service(digits) => Some(format!("+{}{digits}", dial_plan.country_code)),
        Dialed::Emergency(_) | Dialed::Invalid => None,
    }
}

/// Validates and normalizes a rule value.
pub fn normalize_value(kind: RuleKind, value: &str) -> CoreResult<String> {
    let compact: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '/' | '(' | ')'))
        .collect();
    let digits_ok = |d: &str, len: std::ops::RangeInclusive<usize>| {
        len.contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit())
    };
    match kind {
        RuleKind::Country => {
            let iso = compact.to_ascii_uppercase();
            if iso.len() == 2 && iso.bytes().all(|b| b.is_ascii_uppercase()) {
                Ok(iso)
            } else {
                Err(CoreError::Validation(
                    "country must be an ISO 3166-1 code, e.g. DE".into(),
                ))
            }
        }
        RuleKind::Prefix => {
            let prefix = compact.strip_suffix('*').unwrap_or(&compact);
            match prefix.strip_prefix('+') {
                Some(d) if digits_ok(d, 1..=15) => Ok(prefix.to_owned()),
                _ => Err(CoreError::Validation(
                    "prefix must be international, e.g. +49900".into(),
                )),
            }
        }
        RuleKind::Number => match compact.strip_prefix('+') {
            Some(d) if digits_ok(d, 3..=15) && !d.starts_with('0') => Ok(compact.clone()),
            _ => Err(CoreError::Validation(
                "number must be international, e.g. +4930123456".into(),
            )),
        },
    }
}

// --- storage --------------------------------------------------------------------------

#[derive(FromRow)]
struct ProfileRow {
    id: Uuid,
    name: String,
    description: String,
    base_mode: String,
    sip_addresses: bool,
    is_default: bool,
    extensions: i64,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RuleRow {
    id: Uuid,
    profile_id: Uuid,
    kind: String,
    value: String,
    action: String,
    label: String,
}

impl RuleRow {
    fn into_rule(self) -> CoreResult<DialingRule> {
        Ok(DialingRule {
            id: self.id,
            kind: RuleKind::parse(&self.kind)?,
            value: self.value,
            action: RuleAction::parse(&self.action)?,
            label: self.label,
        })
    }
}

const PROFILE_SELECT: &str = "SELECT p.id, p.name, p.description, p.base_mode, p.sip_addresses,
        p.is_default, p.updated_at,
        (SELECT count(*) FROM extensions e
          WHERE e.tenant_id = p.tenant_id AND e.dialing_profile_id = p.id) AS extensions
   FROM dialing_profiles p";

async fn with_rules(
    db: &PgPool,
    tenant: TenantId,
    rows: Vec<ProfileRow>,
) -> CoreResult<Vec<DialingProfile>> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let rules: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, profile_id, kind, value, action, label FROM dialing_rules
         WHERE tenant_id = $1 AND profile_id = ANY($2)
         ORDER BY CASE kind WHEN 'number' THEN 0 WHEN 'prefix' THEN 1 ELSE 2 END, value",
    )
    .bind(tenant)
    .bind(&ids)
    .fetch_all(db)
    .await?;
    let mut profiles = rows
        .into_iter()
        .map(|r| {
            Ok(DialingProfile {
                id: r.id,
                name: r.name,
                description: r.description,
                base_mode: RuleAction::parse(&r.base_mode)?,
                sip_addresses: r.sip_addresses,
                is_default: r.is_default,
                rules: Vec::new(),
                extensions: r.extensions,
                updated_at: r.updated_at,
            })
        })
        .collect::<CoreResult<Vec<_>>>()?;
    for rule in rules {
        if let Some(p) = profiles.iter_mut().find(|p| p.id == rule.profile_id) {
            p.rules.push(rule.into_rule()?);
        }
    }
    Ok(profiles)
}

/// All profiles of the tenant with their rules, by name.
pub async fn list(db: &PgPool, tenant: TenantId) -> CoreResult<Vec<DialingProfile>> {
    let rows: Vec<ProfileRow> = sqlx::query_as(&format!(
        "{PROFILE_SELECT} WHERE p.tenant_id = $1 ORDER BY lower(p.name)"
    ))
    .bind(tenant)
    .fetch_all(db)
    .await?;
    with_rules(db, tenant, rows).await
}

pub async fn get(db: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<DialingProfile> {
    let row: ProfileRow = sqlx::query_as(&format!(
        "{PROFILE_SELECT} WHERE p.tenant_id = $1 AND p.id = $2"
    ))
    .bind(tenant)
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(CoreError::NotFound)?;
    with_rules(db, tenant, vec![row])
        .await?
        .pop()
        .ok_or(CoreError::NotFound)
}

/// The profile that applies with an extension's own profile `own`: the own
/// profile, else the tenant's default profile. `None` if the tenant has no
/// default profile (no limit).
pub async fn effective(
    db: &PgPool,
    tenant: TenantId,
    own: Option<Uuid>,
) -> CoreResult<Option<DialingProfile>> {
    let id: Option<Uuid> = match own {
        Some(id) => Some(id),
        None => {
            sqlx::query_scalar(
                "SELECT id FROM dialing_profiles WHERE tenant_id = $1 AND is_default",
            )
            .bind(tenant)
            .fetch_optional(db)
            .await?
        }
    };
    match id {
        Some(id) => get(db, tenant, id).await.map(Some),
        None => Ok(None),
    }
}

/// The profiles that apply to a user's own outbound calls without a device
/// (fax): the profile of each enabled extension of the user, or the default
/// profile if the user has no enabled extension. A number must be allowed
/// by all of them.
pub async fn for_user(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
) -> CoreResult<Vec<Option<DialingProfile>>> {
    let own: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT DISTINCT dialing_profile_id FROM extensions
         WHERE tenant_id = $1 AND user_id = $2 AND enabled",
    )
    .bind(tenant)
    .bind(user)
    .fetch_all(db)
    .await?;
    let own = if own.is_empty() { vec![None] } else { own };
    let mut profiles = Vec::with_capacity(own.len());
    for id in own {
        profiles.push(effective(db, tenant, id).await?);
    }
    Ok(profiles)
}

/// The reason if the profiles of a user ([`for_user`]) block `e164`; a
/// number must be allowed by all of them. Used for fax (ADR 0045).
pub async fn user_blocks(
    db: &PgPool,
    tenant: TenantId,
    user: Uuid,
    e164: &str,
    geo: &GeoIndex,
) -> CoreResult<Option<String>> {
    for profile in for_user(db, tenant, user).await? {
        let decision = check(profile.as_ref(), e164, geo);
        if !decision.allowed {
            return Ok(Some(decision.reason()));
        }
    }
    Ok(None)
}

struct Validated {
    name: String,
    description: String,
    rules: Vec<(RuleKind, String, RuleAction, String)>,
}

fn validate(input: &DialingProfileInput) -> CoreResult<Validated> {
    let name = input.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(CoreError::Validation(
            "name must have 1 to 100 characters".into(),
        ));
    }
    let description = input.description.trim().to_owned();
    if description.chars().count() > 500 {
        return Err(CoreError::Validation(
            "description must have at most 500 characters".into(),
        ));
    }
    if input.rules.len() > MAX_RULES {
        return Err(CoreError::Validation(format!(
            "a profile can have at most {MAX_RULES} rules"
        )));
    }
    let mut rules: Vec<(RuleKind, String, RuleAction, String)> = Vec::new();
    for rule in &input.rules {
        let value = normalize_value(rule.kind, &rule.value)?;
        if rules
            .iter()
            .any(|(k, v, _, _)| *k == rule.kind && *v == value)
        {
            return Err(CoreError::Validation(format!(
                "rule {} {value} is in the list twice",
                rule.kind.as_str()
            )));
        }
        let label: String = rule.label.trim().chars().take(200).collect();
        rules.push((rule.kind, value, rule.action, label));
    }
    Ok(Validated {
        name,
        description,
        rules,
    })
}

fn audit_details(v: &Validated, input: &DialingProfileInput) -> serde_json::Value {
    serde_json::json!({
        "name": v.name,
        "base_mode": input.base_mode.as_str(),
        "sip_addresses": input.sip_addresses,
        "rules": v.rules.iter().map(|(kind, value, action, _)| {
            format!("{} {} {}", action.as_str(), kind.as_str(), value)
        }).collect::<Vec<_>>(),
    })
}

async fn replace_rules(
    tx: &mut sqlx::PgConnection,
    tenant: TenantId,
    profile: Uuid,
    rules: &[(RuleKind, String, RuleAction, String)],
) -> CoreResult<()> {
    sqlx::query("DELETE FROM dialing_rules WHERE tenant_id = $1 AND profile_id = $2")
        .bind(tenant)
        .bind(profile)
        .execute(&mut *tx)
        .await?;
    let kinds: Vec<&str> = rules.iter().map(|r| r.0.as_str()).collect();
    let values: Vec<&str> = rules.iter().map(|r| r.1.as_str()).collect();
    let actions: Vec<&str> = rules.iter().map(|r| r.2.as_str()).collect();
    let labels: Vec<&str> = rules.iter().map(|r| r.3.as_str()).collect();
    sqlx::query(
        "INSERT INTO dialing_rules (tenant_id, profile_id, kind, value, action, label)
         SELECT $1, $2, * FROM unnest($3::text[], $4::text[], $5::text[], $6::text[])",
    )
    .bind(tenant)
    .bind(profile)
    .bind(&kinds)
    .bind(&values)
    .bind(&actions)
    .bind(&labels)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

fn name_conflict(err: CoreError) -> CoreError {
    match err {
        CoreError::Conflict(_) => CoreError::Conflict("a profile with this name exists".into()),
        other => other,
    }
}

pub async fn create(
    db: &PgPool,
    actor: &Actor,
    input: &DialingProfileInput,
) -> CoreResult<DialingProfile> {
    let v = validate(input)?;
    let mut tx = db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO dialing_profiles (tenant_id, name, description, base_mode, sip_addresses)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(actor.tenant)
    .bind(&v.name)
    .bind(&v.description)
    .bind(input.base_mode.as_str())
    .bind(input.sip_addresses)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| name_conflict(e.into()))?;
    replace_rules(&mut tx, actor.tenant, id, &v.rules).await?;
    crate::audit::record(
        &mut *tx,
        actor,
        "create",
        "dialing_profile",
        Some(id.to_string()),
        audit_details(&v, input),
    )
    .await?;
    tx.commit().await?;
    get(db, actor.tenant, id).await
}

pub async fn update(
    db: &PgPool,
    actor: &Actor,
    id: Uuid,
    input: &DialingProfileInput,
) -> CoreResult<DialingProfile> {
    let v = validate(input)?;
    let mut tx = db.begin().await?;
    let done = sqlx::query(
        "UPDATE dialing_profiles SET name = $3, description = $4, base_mode = $5,
             sip_addresses = $6, updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(actor.tenant)
    .bind(id)
    .bind(&v.name)
    .bind(&v.description)
    .bind(input.base_mode.as_str())
    .bind(input.sip_addresses)
    .execute(&mut *tx)
    .await
    .map_err(|e| name_conflict(e.into()))?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    replace_rules(&mut tx, actor.tenant, id, &v.rules).await?;
    crate::audit::record(
        &mut *tx,
        actor,
        "update",
        "dialing_profile",
        Some(id.to_string()),
        audit_details(&v, input),
    )
    .await?;
    tx.commit().await?;
    get(db, actor.tenant, id).await
}

/// Deletes a profile that is not the default and that no extension uses.
pub async fn delete(db: &PgPool, actor: &Actor, id: Uuid) -> CoreResult<()> {
    let mut tx = db.begin().await?;
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT name, is_default FROM dialing_profiles WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(actor.tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (name, is_default) = row.ok_or(CoreError::NotFound)?;
    if is_default {
        return Err(CoreError::Conflict(
            "the default profile cannot be deleted".into(),
        ));
    }
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM extensions WHERE tenant_id = $1 AND dialing_profile_id = $2",
    )
    .bind(actor.tenant)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if used > 0 {
        return Err(CoreError::Conflict(format!(
            "{used} extension(s) use this profile"
        )));
    }
    sqlx::query("DELETE FROM dialing_profiles WHERE tenant_id = $1 AND id = $2")
        .bind(actor.tenant)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    crate::audit::record(
        &mut *tx,
        actor,
        "delete",
        "dialing_profile",
        Some(id.to_string()),
        serde_json::json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Makes `id` the default profile of the tenant.
pub async fn set_default(db: &PgPool, actor: &Actor, id: Uuid) -> CoreResult<DialingProfile> {
    let mut tx = db.begin().await?;
    let name: String = sqlx::query_scalar(
        "SELECT name FROM dialing_profiles WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(actor.tenant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    sqlx::query(
        "UPDATE dialing_profiles SET is_default = false, updated_at = now()
         WHERE tenant_id = $1 AND is_default AND id <> $2",
    )
    .bind(actor.tenant)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE dialing_profiles SET is_default = true, updated_at = now()
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(actor.tenant)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    crate::audit::record(
        &mut *tx,
        actor,
        "set_default",
        "dialing_profile",
        Some(id.to_string()),
        serde_json::json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;
    get(db, actor.tenant, id).await
}

/// Gives an extension its own profile, or the default profile (`None`).
pub async fn assign(
    db: &PgPool,
    actor: &Actor,
    extension: Uuid,
    profile: Option<Uuid>,
) -> CoreResult<()> {
    let mut tx = db.begin().await?;
    let number: String = sqlx::query_scalar(
        "UPDATE extensions SET dialing_profile_id = $3, updated_at = now()
         WHERE tenant_id = $1 AND id = $2 RETURNING number",
    )
    .bind(actor.tenant)
    .bind(extension)
    .bind(profile)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    crate::audit::record(
        &mut *tx,
        actor,
        "update",
        "extension_dialing_profile",
        Some(extension.to_string()),
        serde_json::json!({ "extension": number, "profile_id": profile }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Number of profiles; used by tests and the backup check.
pub async fn count<'e>(db: impl PgExecutor<'e>, tenant: TenantId) -> CoreResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM dialing_profiles WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(db)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: RuleKind, value: &str, action: RuleAction) -> DialingRule {
        DialingRule {
            id: Uuid::new_v4(),
            kind,
            value: value.into(),
            action,
            label: String::new(),
        }
    }

    fn profile(base_mode: RuleAction, rules: Vec<DialingRule>) -> DialingProfile {
        DialingProfile {
            id: Uuid::new_v4(),
            name: "Test".into(),
            description: String::new(),
            base_mode,
            sip_addresses: true,
            is_default: false,
            rules,
            extensions: 0,
            updated_at: Utc::now(),
        }
    }

    use RuleAction::{Allow, Deny};
    use RuleKind::{Country, Number, Prefix};

    #[test]
    fn base_mode_applies_without_a_matching_rule() {
        let open = profile(Allow, vec![]);
        assert!(evaluate(&open, "+4930123456", Some("DE")).allowed);
        let closed = profile(Deny, vec![]);
        let d = evaluate(&closed, "+4930123456", Some("DE"));
        assert!(!d.allowed);
        assert_eq!(d.rule, None);
        assert_eq!(d.reason(), "dialing:base");
    }

    #[test]
    fn number_beats_prefix_beats_country() {
        let p = profile(
            Deny,
            vec![
                rule(Country, "DE", Allow),
                rule(Prefix, "+49900", Deny),
                rule(Number, "+499001234567", Allow),
            ],
        );
        // Country rule.
        assert!(evaluate(&p, "+4930123456", Some("DE")).allowed);
        // Prefix wins over the country.
        let d = evaluate(&p, "+499005555555", Some("DE"));
        assert!(!d.allowed);
        assert_eq!(d.reason(), "dialing:prefix:+49900");
        // A single number wins over the prefix.
        let d = evaluate(&p, "+499001234567", Some("DE"));
        assert!(d.allowed);
        assert_eq!(d.rule.unwrap().kind, Number);
        // Other countries: base mode.
        assert!(!evaluate(&p, "+33123456789", Some("FR")).allowed);
        // Without geolocation data a country rule cannot match.
        assert!(!evaluate(&p, "+4930123456", None).allowed);
    }

    #[test]
    fn longest_prefix_wins() {
        let p = profile(
            Allow,
            vec![
                rule(Prefix, "+49", Deny),
                rule(Prefix, "+4989", Allow),
                rule(Prefix, "+49891", Deny),
            ],
        );
        assert!(!evaluate(&p, "+4930123456", None).allowed);
        assert!(evaluate(&p, "+4989555555", None).allowed);
        let d = evaluate(&p, "+49891234567", None);
        assert!(!d.allowed);
        assert_eq!(d.rule.unwrap().value, "+49891");
        // A prefix rule does not match a different number.
        assert!(evaluate(&p, "+4312345678", None).allowed);
    }

    #[test]
    fn deny_wins_a_tie() {
        for rules in [
            vec![rule(Prefix, "+49", Allow), rule(Prefix, "+49", Deny)],
            vec![rule(Prefix, "+49", Deny), rule(Prefix, "+49", Allow)],
            vec![rule(Country, "DE", Allow), rule(Country, "DE", Deny)],
            vec![
                rule(Number, "+4930123456", Allow),
                rule(Number, "+4930123456", Deny),
            ],
        ] {
            let p = profile(Allow, rules);
            assert!(!evaluate(&p, "+4930123456", Some("DE")).allowed);
        }
    }

    #[test]
    fn country_rules_ignore_case_of_the_lookup() {
        let p = profile(Deny, vec![rule(Country, "AT", Allow)]);
        assert!(evaluate(&p, "+43123456789", Some("at")).allowed);
    }

    #[test]
    fn dialed_digits_become_the_checked_number() {
        let dp = DialPlanSettings {
            area_code: "89".into(),
            ..Default::default()
        };
        let check = |digits: &str| number_to_check(&dp, &dp.classify(digits));
        // National dialing with the national prefix.
        assert_eq!(check("0900 1234567").as_deref(), Some("+499001234567"));
        // International dialing with 00 and +.
        assert_eq!(check("0043 1 234567").as_deref(), Some("+431234567"));
        assert_eq!(check("+41 44 1234567").as_deref(), Some("+41441234567"));
        // Local number with the area code.
        assert_eq!(check("1234567").as_deref(), Some("+49891234567"));
        // Service numbers in national form.
        assert_eq!(check("11833").as_deref(), Some("+4911833"));
        // Emergency numbers are never checked.
        assert_eq!(check("112"), None);
        assert_eq!(check("110"), None);
        assert_eq!(check("abc"), None);
    }

    #[test]
    fn premium_rules_block_premium_numbers() {
        let p = profile(
            Allow,
            PREMIUM_RULES
                .iter()
                .map(|r| rule(Prefix, r.prefix, Deny))
                .collect(),
        );
        let dp = DialPlanSettings::default();
        for (digits, allowed) in [
            ("09001234567", false),
            ("0137123456", false),
            ("11833", false),
            ("00870123456789", false),
            ("0043900123456", false),
            ("0041901123456", false),
            ("00881612345678", false),
            ("030123456", true),
            ("0043123456789", true),
            ("0800123456", true),
        ] {
            let n = number_to_check(&dp, &dp.classify(digits)).unwrap();
            assert_eq!(evaluate(&p, &n, None).allowed, allowed, "{digits}");
        }
        for r in PREMIUM_RULES {
            assert_eq!(normalize_value(Prefix, r.prefix).unwrap(), r.prefix);
            assert!(!r.source.is_empty() && !r.label.is_empty());
        }
    }

    #[test]
    fn validates_rule_values() {
        assert_eq!(normalize_value(Country, " de ").unwrap(), "DE");
        assert!(normalize_value(Country, "DEU").is_err());
        assert!(normalize_value(Country, "D1").is_err());
        assert_eq!(normalize_value(Prefix, "+49 900*").unwrap(), "+49900");
        assert_eq!(normalize_value(Prefix, "+4").unwrap(), "+4");
        assert!(normalize_value(Prefix, "0900").is_err());
        assert!(normalize_value(Prefix, "+").is_err());
        assert!(normalize_value(Prefix, "+49${x}").is_err());
        assert_eq!(
            normalize_value(Number, "+49 30 123-456").unwrap(),
            "+4930123456"
        );
        assert!(normalize_value(Number, "+0301234").is_err());
        assert!(normalize_value(Number, "+49900*").is_err());
    }

    #[test]
    fn recognizes_only_written_reasons() {
        for ok in [
            "dialing:base",
            "dialing:sip",
            "dialing:country:DE",
            "dialing:prefix:+49900",
            "dialing:number:+4930123456",
        ] {
            assert!(valid_block_reason(ok), "{ok}");
        }
        for bad in [
            "dialing:",
            "dialing:country:de",
            "dialing:prefix:+49900*",
            "dialing:other:x",
            "list:+49900*",
            "base",
        ] {
            assert!(!valid_block_reason(bad), "{bad}");
        }
    }

    #[test]
    fn no_profile_allows_everything() {
        let geo = GeoIndex::default();
        assert!(check(None, "+499001234567", &geo).allowed);
        assert!(sip_allowed(None));
        let mut p = profile(Allow, vec![]);
        p.sip_addresses = false;
        assert!(!sip_allowed(Some(&p)));
    }
}
