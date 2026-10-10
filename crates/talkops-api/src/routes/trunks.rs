//! Trunks, accounts, phone numbers and provider presets.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use talkops_core::presets::{AccountMode, TrunkPreset, render_template};
use talkops_core::trunks::{
    self, AccountInput, NumberDestination, NumberInput, PhoneNumber, Trunk, TrunkAccount,
    TrunkInput,
};
use talkops_core::users::Role;
use talkops_core::{audit, numbering};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::telephony::GatewayState;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_presets))
        .routes(routes!(list_trunks, create_trunk))
        .routes(routes!(get_trunk, update_trunk, delete_trunk))
        .routes(routes!(create_account))
        .routes(routes!(update_account, delete_account))
        .routes(routes!(add_line))
        .routes(routes!(list_numbers, create_number))
        .routes(routes!(update_number, delete_number))
}

#[derive(Serialize, ToSchema)]
pub struct AccountWithState {
    #[serde(flatten)]
    pub account: TrunkAccount,
    pub gateway: String,
    /// Live registration state from FreeSWITCH, if known.
    pub state: Option<GatewayState>,
}

#[derive(Serialize, ToSchema)]
pub struct TrunkDetail {
    #[serde(flatten)]
    pub trunk: Trunk,
    pub accounts: Vec<AccountWithState>,
    pub numbers: Vec<PhoneNumber>,
}

/// One phone number with its own credentials (per-number providers like LEONET).
#[derive(Deserialize, ToSchema)]
pub struct LineInput {
    /// E.164, e.g. +49891234567.
    pub e164: String,
    pub password: String,
    /// Overrides the username derived from the preset template.
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_username: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub destination_extension_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct Line {
    pub account: TrunkAccount,
    pub number: PhoneNumber,
}

/// Collects gateways to reload after a change and triggers the reload in the background.
fn reload(state: &AppState, gateways: Vec<String>) {
    let telephony = state.telephony.clone();
    tokio::spawn(async move { telephony.reload_gateways(&gateways).await });
}

async fn trunk_gateways(state: &AppState, auth: &AuthUser, trunk: Uuid) -> ApiResult<Vec<String>> {
    Ok(trunks::list_accounts(&state.db, auth.tenant, trunk)
        .await?
        .iter()
        .map(TrunkAccount::gateway_name)
        .collect())
}

/// All provider presets (any logged-in user).
#[utoipa::path(get, path = "/api/v1/presets", tag = "trunks", responses((status = 200, body = [Object])))]
pub async fn list_presets(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<Vec<TrunkPreset>> {
    Json(state.catalog.all().cloned().collect())
}

/// Lists trunks (operator or admin).
#[utoipa::path(get, path = "/api/v1/trunks", tag = "trunks", responses((status = 200, body = [TrunkDetail])))]
pub async fn list_trunks(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<TrunkDetail>>> {
    auth.require(Role::Operator)?;
    let mut out = Vec::new();
    for trunk in trunks::list(&state.db, auth.tenant).await? {
        out.push(detail(&state, &auth, trunk).await?);
    }
    Ok(Json(out))
}

async fn detail(state: &AppState, auth: &AuthUser, trunk: Trunk) -> ApiResult<TrunkDetail> {
    let live = state.telephony.snapshot().await;
    let accounts = trunks::list_accounts(&state.db, auth.tenant, trunk.id)
        .await?
        .into_iter()
        .map(|account| {
            let gateway = account.gateway_name();
            let state = live.gateways.get(&gateway).cloned();
            AccountWithState {
                account,
                gateway,
                state,
            }
        })
        .collect();
    let numbers = trunks::list_numbers(&state.db, auth.tenant)
        .await?
        .into_iter()
        .filter(|n| n.trunk_id == trunk.id)
        .collect();
    Ok(TrunkDetail {
        trunk,
        accounts,
        numbers,
    })
}

/// Creates a trunk from a preset (admin).
#[utoipa::path(post, path = "/api/v1/trunks", tag = "trunks", request_body = TrunkInput, responses((status = 200, body = Trunk)))]
pub async fn create_trunk(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TrunkInput>,
) -> ApiResult<Json<Trunk>> {
    auth.require(Role::Admin)?;
    let trunk = trunks::create(&state.db, auth.tenant, &state.catalog, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "trunk",
        Some(trunk.id.to_string()),
        json!({"name": trunk.name, "preset": trunk.preset}),
    )
    .await?;
    Ok(Json(trunk))
}

/// Returns a trunk with accounts (incl. live registration state) and numbers.
#[utoipa::path(get, path = "/api/v1/trunks/{id}", tag = "trunks", params(("id" = Uuid, Path)), responses((status = 200, body = TrunkDetail)))]
pub async fn get_trunk(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TrunkDetail>> {
    auth.require(Role::Operator)?;
    let trunk = trunks::get(&state.db, auth.tenant, id).await?;
    Ok(Json(detail(&state, &auth, trunk).await?))
}

/// Updates a trunk (admin). Its gateways are re-registered.
#[utoipa::path(put, path = "/api/v1/trunks/{id}", tag = "trunks", params(("id" = Uuid, Path)), request_body = TrunkInput, responses((status = 200, body = Trunk)))]
pub async fn update_trunk(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<TrunkInput>,
) -> ApiResult<Json<Trunk>> {
    auth.require(Role::Admin)?;
    let trunk = trunks::update(&state.db, auth.tenant, &state.catalog, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "trunk",
        Some(id.to_string()),
        json!({"name": trunk.name, "enabled": trunk.enabled, "overrides": trunk.overrides}),
    )
    .await?;
    reload(&state, trunk_gateways(&state, &auth, id).await?);
    Ok(Json(trunk))
}

/// Deletes a trunk with its accounts and numbers (admin).
#[utoipa::path(delete, path = "/api/v1/trunks/{id}", tag = "trunks", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_trunk(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let trunk = trunks::get(&state.db, auth.tenant, id).await?;
    let gateways = trunk_gateways(&state, &auth, id).await?;
    trunks::delete(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "trunk",
        Some(id.to_string()),
        json!({"name": trunk.name}),
    )
    .await?;
    reload(&state, gateways);
    Ok(StatusCode::NO_CONTENT)
}

/// Adds an account (registration) to a trunk (admin).
#[utoipa::path(post, path = "/api/v1/trunks/{id}/accounts", tag = "trunks", params(("id" = Uuid, Path)), request_body = AccountInput, responses((status = 200, body = TrunkAccount)))]
pub async fn create_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<AccountInput>,
) -> ApiResult<Json<TrunkAccount>> {
    auth.require(Role::Admin)?;
    numbering::check_destination(
        &state.db,
        auth.tenant,
        input.destination_type,
        input.destination_id,
    )
    .await?;
    let account =
        trunks::create_account(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "trunk_account",
        Some(account.id.to_string()),
        json!({"trunk": id, "username": account.username}),
    )
    .await?;
    reload(&state, vec![]);
    Ok(Json(account))
}

/// Updates an account; an empty password keeps the stored one (admin).
#[utoipa::path(put, path = "/api/v1/trunk-accounts/{id}", tag = "trunks", params(("id" = Uuid, Path)), request_body = AccountInput, responses((status = 200, body = TrunkAccount)))]
pub async fn update_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<AccountInput>,
) -> ApiResult<Json<TrunkAccount>> {
    auth.require(Role::Admin)?;
    numbering::check_destination(
        &state.db,
        auth.tenant,
        input.destination_type,
        input.destination_id,
    )
    .await?;
    let account =
        trunks::update_account(&state.db, auth.tenant, &state.secrets, id, &input).await?;
    audit::record(&state.db, &auth.actor(), "update", "trunk_account", Some(id.to_string()), json!({"username": account.username, "enabled": account.enabled, "password_changed": input.password.as_deref().is_some_and(|p| !p.is_empty())})).await?;
    reload(&state, vec![account.gateway_name()]);
    Ok(Json(account))
}

/// Deletes an account (admin).
#[utoipa::path(delete, path = "/api/v1/trunk-accounts/{id}", tag = "trunks", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let account = trunks::get_account(&state.db, auth.tenant, id).await?;
    trunks::delete_account(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "trunk_account",
        Some(id.to_string()),
        json!({"username": account.username}),
    )
    .await?;
    reload(&state, vec![account.gateway_name()]);
    Ok(StatusCode::NO_CONTENT)
}

/// Adds a number with its own credentials in one step: creates the account
/// (username from the preset template) and the number (admin).
#[utoipa::path(post, path = "/api/v1/trunks/{id}/lines", tag = "trunks", params(("id" = Uuid, Path)), request_body = LineInput, responses((status = 200, body = Line)))]
pub async fn add_line(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<LineInput>,
) -> ApiResult<Json<Line>> {
    auth.require(Role::Admin)?;
    let trunk = trunks::get(&state.db, auth.tenant, id).await?;
    let preset = state
        .catalog
        .get(&trunk.preset)
        .ok_or_else(|| ApiError::BadRequest("unknown preset".into()))?;
    if preset.credentials.mode != AccountMode::PerNumber {
        return Err(ApiError::BadRequest(
            "this provider uses one account for all numbers; add an account and numbers instead"
                .into(),
        ));
    }
    let username = input
        .username
        .clone()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| {
            render_template(&preset.credentials.username_template, Some(&input.e164), "")
        });
    let auth_username = input.auth_username.clone().unwrap_or_default();

    let mut tx = state.db.begin().await?;
    let account = trunks::create_account(
        &mut *tx,
        auth.tenant,
        &state.secrets,
        id,
        &AccountInput {
            username,
            auth_username,
            password: Some(input.password.clone()),
            enabled: true,
            destination_type: NumberDestination::None,
            destination_id: None,
        },
    )
    .await?;
    tx.commit().await?;
    let number_input = NumberInput {
        trunk_id: id,
        account_id: Some(account.id),
        e164: input.e164.clone(),
        label: input.label.clone(),
        destination_type: if input.destination_extension_id.is_some() {
            NumberDestination::Extension
        } else {
            NumberDestination::None
        },
        destination_id: input.destination_extension_id,
        extra_extensions: vec![],
        enabled: true,
    };
    let number = match trunks::create_number(&state.db, auth.tenant, &number_input).await {
        Ok(n) => n,
        Err(err) => {
            // Keep things consistent: no orphan account without its number.
            let _ = trunks::delete_account(&state.db, auth.tenant, account.id).await;
            return Err(err.into());
        }
    };
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "trunk_line",
        Some(number.id.to_string()),
        json!({"trunk": id, "number": number.e164, "username": account.username}),
    )
    .await?;
    reload(&state, vec![]);
    Ok(Json(Line { account, number }))
}

/// Lists all phone numbers (operator or admin).
#[utoipa::path(get, path = "/api/v1/numbers", tag = "trunks", responses((status = 200, body = [PhoneNumber])))]
pub async fn list_numbers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<PhoneNumber>>> {
    auth.require(Role::Operator)?;
    Ok(Json(trunks::list_numbers(&state.db, auth.tenant).await?))
}

/// Adds a phone number to a trunk (admin).
#[utoipa::path(post, path = "/api/v1/numbers", tag = "trunks", request_body = NumberInput, responses((status = 200, body = PhoneNumber)))]
pub async fn create_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NumberInput>,
) -> ApiResult<Json<PhoneNumber>> {
    auth.require(Role::Admin)?;
    let number = trunks::create_number(&state.db, auth.tenant, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "create",
        "number",
        Some(number.id.to_string()),
        json!({"e164": number.e164}),
    )
    .await?;
    if let Some(account) = number.account_id {
        reload(&state, vec![trunks::gateway_name(account)]);
    }
    Ok(Json(number))
}

/// Updates a phone number, e.g. its destination (admin).
#[utoipa::path(put, path = "/api/v1/numbers/{id}", tag = "trunks", params(("id" = Uuid, Path)), request_body = NumberInput, responses((status = 200, body = PhoneNumber)))]
pub async fn update_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<NumberInput>,
) -> ApiResult<Json<PhoneNumber>> {
    auth.require(Role::Admin)?;
    let before = trunks::get_number(&state.db, auth.tenant, id).await?;
    let number = trunks::update_number(&state.db, auth.tenant, id, &input).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "number",
        Some(id.to_string()),
        json!({"e164": number.e164, "destination_type": number.destination_type}),
    )
    .await?;
    if before.e164 != number.e164 || before.account_id != number.account_id {
        reload(
            &state,
            [before.account_id, number.account_id]
                .into_iter()
                .flatten()
                .map(trunks::gateway_name)
                .collect(),
        );
    }
    Ok(Json(number))
}

/// Deletes a phone number (admin).
#[utoipa::path(delete, path = "/api/v1/numbers/{id}", tag = "trunks", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    auth.require(Role::Admin)?;
    let number = trunks::get_number(&state.db, auth.tenant, id).await?;
    trunks::delete_number(&state.db, auth.tenant, id).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "delete",
        "number",
        Some(id.to_string()),
        json!({"e164": number.e164}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
