//! Outbound dialing permissions (ADR 0045): dialing profiles, the default
//! profile, the profile of an extension and a number test.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use talkops_core::dial_permissions::{
    self, Decision, DialingProfile, DialingProfileInput, PREMIUM_RULES, PremiumRule,
};
use talkops_core::dialing::Dialed;
use talkops_core::roles::Permission;
use talkops_core::{extensions, settings};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_profiles, create_profile))
        .routes(routes!(update_profile, delete_profile))
        .routes(routes!(set_default))
        .routes(routes!(premium_rules))
        .routes(routes!(test_number))
        .routes(routes!(assign_extension))
}

/// The profiles can be read with `extensions.view` too (names in the
/// extension list); changes need `dialing.manage`.
fn require_read(auth: &AuthUser) -> ApiResult<()> {
    if auth.can(Permission::DialingManage) || auth.can(Permission::ExtensionsView) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

/// All dialing profiles with their rules.
#[utoipa::path(get, path = "/api/v1/dialing-profiles", tag = "routing", responses((status = 200, body = Vec<DialingProfile>)))]
pub async fn list_profiles(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<DialingProfile>>> {
    require_read(&auth)?;
    Ok(Json(dial_permissions::list(&state.db, auth.tenant).await?))
}

#[utoipa::path(post, path = "/api/v1/dialing-profiles", tag = "routing", request_body = DialingProfileInput,
    responses((status = 201, body = DialingProfile), (status = 409, description = "Name in use"), (status = 422, description = "Invalid rule")))]
pub async fn create_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<DialingProfileInput>,
) -> ApiResult<(StatusCode, Json<DialingProfile>)> {
    auth.require_perm(Permission::DialingManage)?;
    let profile = dial_permissions::create(&state.db, &auth.actor(), &input).await?;
    Ok((StatusCode::CREATED, Json(profile)))
}

/// Changes a profile; the rules in the request replace all rules.
#[utoipa::path(put, path = "/api/v1/dialing-profiles/{id}", tag = "routing", params(("id" = Uuid, Path)), request_body = DialingProfileInput,
    responses((status = 200, body = DialingProfile), (status = 409, description = "Name in use"), (status = 422, description = "Invalid rule")))]
pub async fn update_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DialingProfileInput>,
) -> ApiResult<Json<DialingProfile>> {
    auth.require_perm(Permission::DialingManage)?;
    Ok(Json(
        dial_permissions::update(&state.db, &auth.actor(), id, &input).await?,
    ))
}

/// Deletes a profile that is not the default and that no extension uses.
#[utoipa::path(delete, path = "/api/v1/dialing-profiles/{id}", tag = "routing", params(("id" = Uuid, Path)),
    responses((status = 204), (status = 409, description = "Default profile or in use")))]
pub async fn delete_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require_perm(Permission::DialingManage)?;
    dial_permissions::delete(&state.db, &auth.actor(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Makes the profile the default for extensions without their own profile.
#[utoipa::path(put, path = "/api/v1/dialing-profiles/{id}/default", tag = "routing", params(("id" = Uuid, Path)), responses((status = 200, body = DialingProfile)))]
pub async fn set_default(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DialingProfile>> {
    auth.require_perm(Permission::DialingManage)?;
    Ok(Json(
        dial_permissions::set_default(&state.db, &auth.actor(), id).await?,
    ))
}

/// The built-in premium-rate block list with its sources (to add to a
/// profile).
#[utoipa::path(get, path = "/api/v1/dialing-profiles/premium-rules", tag = "routing", responses((status = 200, body = Vec<PremiumRule>)))]
pub async fn premium_rules(auth: AuthUser) -> ApiResult<Json<&'static [PremiumRule]>> {
    auth.require_perm(Permission::DialingManage)?;
    Ok(Json(PREMIUM_RULES))
}

#[derive(Deserialize, ToSchema)]
pub struct DialTestRequest {
    /// Digits as a phone dials them (`0900…`, `0043…`, `+41…`, `112`).
    pub number: String,
    /// Test with the profile of this extension.
    #[serde(default)]
    pub extension_id: Option<Uuid>,
    /// Test with this profile (wins over `extension_id`).
    #[serde(default)]
    pub profile_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct DialTestProfile {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct DialTestResult {
    /// `emergency`, `service`, `external` or `invalid`.
    pub class: String,
    /// The number the profile evaluates (E.164).
    pub number: Option<String>,
    /// ISO country of the number from the geolocation data.
    pub country: Option<String>,
    /// The profile used; none if the tenant has no default profile.
    pub profile: Option<DialTestProfile>,
    pub allowed: bool,
    #[serde(flatten)]
    pub decision: Option<Decision>,
    /// Reason a call would be blocked with (`dialing:…`).
    pub reason: Option<String>,
}

/// Would a call to this number be allowed? Uses the profile, else the
/// extension's profile, else the default profile.
#[utoipa::path(post, path = "/api/v1/dialing-profiles/test", tag = "routing", request_body = DialTestRequest, responses((status = 200, body = DialTestResult)))]
pub async fn test_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<DialTestRequest>,
) -> ApiResult<Json<DialTestResult>> {
    auth.require_perm(Permission::DialingManage)?;
    let own = match (req.profile_id, req.extension_id) {
        (Some(id), _) => Some(id),
        (None, Some(ext)) => {
            extensions::get(&state.db, auth.tenant, ext)
                .await?
                .dialing_profile_id
        }
        (None, None) => None,
    };
    let profile = dial_permissions::effective(&state.db, auth.tenant, own).await?;
    let dial_plan = settings::get(&state.db, auth.tenant).await?.dial_plan();
    let dialed = dial_plan.classify(&req.number);
    let class = match dialed {
        Dialed::Emergency(_) => "emergency",
        Dialed::Service(_) => "service",
        Dialed::External(_) => "external",
        Dialed::Invalid => "invalid",
    };
    let number = dial_permissions::number_to_check(&dial_plan, &dialed);
    let decision = number
        .as_deref()
        .map(|n| dial_permissions::check(profile.as_ref(), n, &state.geo));
    Ok(Json(DialTestResult {
        class: class.to_owned(),
        country: number
            .as_deref()
            .and_then(|n| state.geo.lookup(n))
            .map(|a| a.country.clone()),
        number,
        profile: profile.map(|p| DialTestProfile {
            id: p.id,
            name: p.name,
        }),
        // Emergency calls are always allowed; invalid input never goes out.
        allowed: match &decision {
            Some(d) => d.allowed,
            None => matches!(dialed, Dialed::Emergency(_)),
        },
        reason: decision
            .as_ref()
            .filter(|d| !d.allowed)
            .map(Decision::reason),
        decision,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct AssignProfile {
    /// The extension's own profile; null: the default profile.
    pub profile_id: Option<Uuid>,
}

/// Gives an extension its own dialing profile or the default profile.
#[utoipa::path(put, path = "/api/v1/extensions/{id}/dialing-profile", tag = "extensions", params(("id" = Uuid, Path)), request_body = AssignProfile,
    responses((status = 200, body = extensions::Extension), (status = 422, description = "Unknown profile")))]
pub async fn assign_extension(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<AssignProfile>,
) -> ApiResult<Json<extensions::Extension>> {
    auth.require_perm(Permission::DialingManage)?;
    if let Some(profile) = input.profile_id {
        // Another tenant's profile does not exist for this tenant.
        dial_permissions::get(&state.db, auth.tenant, profile)
            .await
            .map_err(|_| ApiError::BadRequest("unknown dialing profile".into()))?;
    }
    dial_permissions::assign(&state.db, &auth.actor(), id, input.profile_id).await?;
    Ok(Json(extensions::get(&state.db, auth.tenant, id).await?))
}
