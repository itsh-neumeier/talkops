//! Call origin map (ADR 0044): external calls of a rolling time window,
//! counted for each local network area or place.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use talkops_core::cdr::{self, Cdr, CdrQuery, CdrScope, Direction};
use talkops_core::geo::{GeoArea, GeoIndex, GeoKind, GeoSource};
use talkops_core::stats::Range;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::active_calls::Directory;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::settings::{CallEntry, own_extensions};
use talkops_core::audit;
use talkops_core::roles::Permission;
use talkops_core::settings::{self, MapTiles};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(call_map))
        .routes(routes!(get_map_tiles, update_map_tiles))
}

/// Calls in the details of one area.
const CALLS_PER_AREA: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Inbound, answered.
    Answered,
    /// Inbound, not answered (also busy or declined).
    Missed,
    /// Outbound; the destination gives the area.
    Outbound,
    /// Inbound, blocked by the block list, PhoneBlock or the anonymous rule.
    Blocked,
}

impl Category {
    const ALL: [Self; 4] = [Self::Answered, Self::Missed, Self::Outbound, Self::Blocked];

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "answered" => Self::Answered,
            "missed" => Self::Missed,
            "outbound" => Self::Outbound,
            "blocked" => Self::Blocked,
            _ => return None,
        })
    }

    /// Category of an external call; `None` for internal calls.
    pub fn of(c: &Cdr) -> Option<Self> {
        match c.direction {
            Direction::Internal => None,
            Direction::Outbound => Some(Self::Outbound),
            Direction::Inbound if c.blocked_reason.is_some() => Some(Self::Blocked),
            Direction::Inbound if c.answered_at.is_some() => Some(Self::Answered),
            Direction::Inbound => Some(Self::Missed),
        }
    }
}

/// Number of calls for each category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct Counts {
    pub answered: i64,
    pub missed: i64,
    pub outbound: i64,
    pub blocked: i64,
}

impl Counts {
    fn add(&mut self, c: Category) {
        match c {
            Category::Answered => self.answered += 1,
            Category::Missed => self.missed += 1,
            Category::Outbound => self.outbound += 1,
            Category::Blocked => self.blocked += 1,
        }
    }

    fn total(&self) -> i64 {
        self.answered + self.missed + self.outbound + self.blocked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AreaKind {
    /// A local network area or a place.
    Area,
    /// Mobile, service and other numbers without a location in a country.
    None,
}

/// A bubble of the map, or an entry without a location.
#[derive(Serialize, utoipa::ToSchema)]
pub struct MapArea {
    /// Geo area ID (`DE-951`), `<country>-none` or `none` (unknown country).
    pub id: String,
    pub kind: AreaKind,
    /// ISO country code; `None` when the country is unknown.
    pub country: Option<String>,
    pub name: String,
    /// Display prefix, e.g. `+49 951`; empty when the country is unknown.
    pub prefix: String,
    /// Position of the bubble; `None` when the country is unknown.
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub counts: Counts,
    pub total: i64,
    /// Talk time of all calls in seconds.
    pub billsec: i64,
    /// Entries without a location: mobile numbers.
    pub mobile: i64,
    /// Entries without a location: service and non-geographic numbers.
    pub service: i64,
    /// Entries without a location: numbers with only a known country, or
    /// no known country.
    pub unresolved: i64,
    /// Newest calls first, at most 20.
    pub calls: Vec<CallEntry>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CallMap {
    #[schema(value_type = String)]
    pub range: Range,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    /// More calls than `limit` in the range: older calls are not counted.
    pub truncated: bool,
    pub limit: i64,
    /// All counted external calls, anonymous calls included.
    pub counts: Counts,
    pub total: i64,
    /// Calls without a caller number (not on the map).
    pub anonymous: Counts,
    /// Areas with the most calls first.
    pub areas: Vec<MapArea>,
    /// Origin and license of the geo data (attribution).
    pub sources: Vec<GeoSource>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MapQuery {
    /// `1h`, `1d` (default), `1w` or `1m` (30 days), up to now.
    #[param(value_type = Option<String>)]
    pub range: Option<Range>,
    /// Comma-separated categories: `answered`, `missed`, `outbound`,
    /// `blocked`. Default: all.
    pub categories: Option<String>,
}

/// Length of a rolling range.
pub fn window(range: Range) -> Duration {
    match range {
        Range::Hour => Duration::hours(1),
        Range::Day => Duration::hours(24),
        Range::Week => Duration::days(7),
        Range::Month => Duration::days(30),
    }
}

/// Where a call appears on the map.
#[derive(Debug, PartialEq)]
pub enum Place<'g> {
    /// No caller number (only counted).
    Anonymous,
    /// A local network area or a place.
    Area(&'g GeoArea),
    /// No location below the country; `None`: unknown country.
    NoLocation(Option<&'g GeoArea>, GeoKind),
}

impl Place<'_> {
    /// The ID of the bubble or list entry (`DE-951`, `DE-none`, `none`).
    pub fn id(&self) -> Option<String> {
        match self {
            Place::Anonymous => None,
            Place::Area(a) => Some(a.id.clone()),
            Place::NoLocation(Some(a), _) => Some(format!("{}-none", a.country)),
            Place::NoLocation(None, _) => Some("none".into()),
        }
    }
}

/// The external number of a call: the caller of inbound calls, the
/// destination of outbound calls.
fn external_number(c: &Cdr) -> &str {
    match c.direction {
        Direction::Outbound => &c.destination,
        _ => &c.caller_number,
    }
}

/// Resolves the external number of a call to its place on the map.
pub fn place<'g>(dir: &Directory, geo: &'g GeoIndex, c: &Cdr) -> Place<'g> {
    let raw = external_number(c).trim();
    if c.blocked_reason.as_deref() == Some("anonymous") || !raw.bytes().any(|b| b.is_ascii_digit())
    {
        return Place::Anonymous;
    }
    match dir.locate(geo, raw) {
        Some(a) if a.kind.is_located() => Place::Area(a),
        Some(a) => Place::NoLocation(geo.country(&a.country), a.kind),
        None => Place::NoLocation(None, GeoKind::Country),
    }
}

fn parse_categories(text: Option<&str>) -> Result<Vec<Category>, ApiError> {
    let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(Category::ALL.to_vec());
    };
    let mut out = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let c = Category::parse(part)
            .ok_or_else(|| ApiError::BadRequest(format!("unknown category {part:?}")))?;
        if !out.contains(&c) {
            out.push(c);
        }
    }
    Ok(out)
}

/// The scope of the call log for this user (same rule as `GET /api/v1/calls`).
async fn scope(state: &AppState, auth: &AuthUser) -> ApiResult<CdrScope> {
    Ok(CdrScope {
        extensions: own_extensions(state, auth).await?,
        ..CdrScope::default()
    })
}

/// External calls of the window that the user may see, newest first, at
/// most `WINDOW_LIMIT + 1` (the extra row shows a truncated result).
async fn window_calls(
    state: &AppState,
    auth: &AuthUser,
    since: DateTime<Utc>,
    until: Option<DateTime<Utc>>,
) -> ApiResult<Vec<Cdr>> {
    let query = CdrQuery {
        since: Some(since),
        until,
        direction: Some("inbound,outbound".into()),
        ..CdrQuery::default()
    };
    Ok(cdr::list_window(
        &state.db,
        auth.tenant,
        &query,
        &scope(state, auth).await?,
        cdr::WINDOW_LIMIT + 1,
    )
    .await?)
}

/// IDs of the calls in a map area (filter of the call log). The window is
/// `since` to `until` of the list query, at most [`cdr::WINDOW_LIMIT`]
/// calls.
pub async fn area_call_ids(
    state: &AppState,
    auth: &AuthUser,
    directory: &Directory,
    area: &str,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
) -> ApiResult<Vec<uuid::Uuid>> {
    let since = since.unwrap_or_else(|| Utc::now() - window(Range::Month));
    let rows = window_calls(state, auth, since, until).await?;
    Ok(rows
        .iter()
        .take(cdr::WINDOW_LIMIT as usize)
        .filter(|c| place(directory, &state.geo, c).id().as_deref() == Some(area))
        .map(|c| c.id)
        .collect())
}

/// External calls of the last hour, day, week or month, counted for each
/// area of the caller (inbound) or destination (outbound) number. Users
/// without `calls.view_all` see only the calls of their extensions.
#[utoipa::path(get, path = "/api/v1/calls/map", tag = "settings", params(MapQuery), responses((status = 200, body = CallMap), (status = 422, description = "Unknown category")))]
pub async fn call_map(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<MapQuery>,
) -> ApiResult<Json<CallMap>> {
    let range = q.range.unwrap_or(Range::Day);
    let categories = parse_categories(q.categories.as_deref())?;
    let until = Utc::now();
    let since = until - window(range);
    let (rows, directory) = tokio::try_join!(
        window_calls(&state, &auth, since, None),
        Directory::load(&state, auth.tenant)
    )?;
    let truncated = rows.len() as i64 > cdr::WINDOW_LIMIT;
    let rows = &rows[..rows.len().min(cdr::WINDOW_LIMIT as usize)];

    let mut counts = Counts::default();
    let mut anonymous = Counts::default();
    let mut areas: Vec<MapArea> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    // Calls of each area for the details; parties are resolved only for them.
    let mut recent: Vec<Vec<&Cdr>> = Vec::new();
    for c in rows {
        let Some(category) = Category::of(c).filter(|k| categories.contains(k)) else {
            continue;
        };
        counts.add(category);
        let place = place(&directory, &state.geo, c);
        let Some(id) = place.id() else {
            anonymous.add(category);
            continue;
        };
        let i = *index.entry(id.clone()).or_insert_with(|| {
            areas.push(new_area(id, &place));
            recent.push(Vec::new());
            areas.len() - 1
        });
        let area = &mut areas[i];
        area.counts.add(category);
        area.total += 1;
        area.billsec += i64::from(c.billsec.max(0));
        match place {
            Place::NoLocation(_, GeoKind::Mobile) => area.mobile += 1,
            Place::NoLocation(_, GeoKind::Service) => area.service += 1,
            Place::NoLocation(..) => area.unresolved += 1,
            _ => {}
        }
        if recent[i].len() < CALLS_PER_AREA {
            recent[i].push(c);
        }
    }
    for (area, calls) in areas.iter_mut().zip(recent) {
        area.calls = calls
            .into_iter()
            .map(|c| {
                let (caller, called, via_number) = directory.record_parties(c);
                CallEntry {
                    cdr: c.clone(),
                    caller,
                    called,
                    via_number,
                    allowlisted: None,
                }
            })
            .collect();
    }
    areas.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.id.cmp(&b.id)));
    Ok(Json(CallMap {
        range,
        since,
        until,
        truncated,
        limit: cdr::WINDOW_LIMIT,
        total: counts.total(),
        counts,
        anonymous,
        areas,
        sources: state.geo.sources().to_vec(),
    }))
}

fn new_area(id: String, place: &Place<'_>) -> MapArea {
    let (kind, geo) = match place {
        Place::Area(a) => (AreaKind::Area, Some(*a)),
        Place::NoLocation(country, _) => (AreaKind::None, *country),
        Place::Anonymous => (AreaKind::None, None),
    };
    MapArea {
        id,
        kind,
        country: geo.map(|a| a.country.clone()),
        name: geo.map(|a| a.name.clone()).unwrap_or_default(),
        prefix: geo.map(|a| a.prefix.clone()).unwrap_or_default(),
        lat: geo.map(|a| a.lat),
        lon: geo.map(|a| a.lon),
        counts: Counts::default(),
        total: 0,
        billsec: 0,
        mobile: 0,
        service: 0,
        unresolved: 0,
        calls: Vec::new(),
    }
}

/// Placeholders that a tile URL may contain.
const TILE_PLACEHOLDERS: [&str; 5] = ["{z}", "{x}", "{y}", "{s}", "{r}"];

/// Checks a tile URL and returns the source for the Content Security Policy
/// (`https://host[:port]`, `https://*.host` for `{s}` subdomains).
pub fn tile_source(url: &str) -> Result<String, String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or("the tile URL must start with https://")?;
    if url.len() > 500
        || url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "'\"<>\\;,`^|".contains(c))
    {
        return Err("the tile URL contains invalid characters".into());
    }
    for p in ["{z}", "{x}", "{y}"] {
        if !url.contains(p) {
            return Err(format!(
                "the tile URL needs the placeholders {{z}}, {{x}} and {{y}} ({p} is missing)"
            ));
        }
    }
    let mut unknown = url.to_owned();
    for p in TILE_PLACEHOLDERS {
        unknown = unknown.replace(p, "");
    }
    if unknown.contains(['{', '}']) {
        return Err("the tile URL has an unknown placeholder".into());
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let (subdomains, plain) = match host.strip_prefix("{s}.") {
        Some(h) => (true, h),
        None => (false, host),
    };
    let valid_host = !plain.is_empty()
        && !plain.contains('@')
        && plain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
        && url::Url::parse(&format!("https://{plain}/")).is_ok_and(|u| u.host().is_some());
    if !valid_host {
        return Err("the tile URL has an invalid host".into());
    }
    let sample = url
        .replace("{s}", "a")
        .replace("{z}", "1")
        .replace("{x}", "1")
        .replace("{y}", "1")
        .replace("{r}", "");
    url::Url::parse(&sample).map_err(|_| "the tile URL is not a valid URL".to_owned())?;
    let host = plain.to_ascii_lowercase();
    Ok(if subdomains {
        format!("https://*.{host}")
    } else {
        format!("https://{host}")
    })
}

fn validate_tiles(input: MapTiles) -> Result<MapTiles, ApiError> {
    let tiles = MapTiles {
        tile_url: input.tile_url.trim().to_owned(),
        attribution: input.attribution.trim().to_owned(),
    };
    if !tiles.tile_url.is_empty() {
        tile_source(&tiles.tile_url).map_err(ApiError::BadRequest)?;
    }
    if tiles.attribution.chars().count() > 300 || tiles.attribution.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "attribution: at most 300 characters on one line".into(),
        ));
    }
    if !tiles.tile_url.is_empty() && tiles.attribution.is_empty() {
        return Err(ApiError::BadRequest(
            "a tile server needs an attribution text".into(),
        ));
    }
    Ok(tiles)
}

/// Tile server of the call map (any logged-in user; empty: offline map).
#[utoipa::path(get, path = "/api/v1/settings/call-map", tag = "settings", responses((status = 200, body = MapTiles)))]
pub async fn get_map_tiles(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<MapTiles>> {
    Ok(Json(settings::get_map_tiles(&state.db, auth.tenant).await?))
}

/// Sets or clears the tile server of the call map (settings managers).
/// The browser then loads map tiles from this server.
#[utoipa::path(put, path = "/api/v1/settings/call-map", tag = "settings", request_body = MapTiles, responses((status = 200, body = MapTiles), (status = 422, description = "Invalid tile URL or attribution")))]
pub async fn update_map_tiles(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<MapTiles>,
) -> ApiResult<Json<MapTiles>> {
    auth.require_perm(Permission::SettingsManage)?;
    let tiles = validate_tiles(input)?;
    let saved = settings::set_map_tiles(&state.db, auth.tenant, &tiles).await?;
    audit::record(
        &state.db,
        &auth.actor(),
        "update",
        "call_map_tiles",
        None,
        serde_json::to_value(&saved).unwrap_or_default(),
    )
    .await?;
    Ok(Json(saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_tile_urls_and_derives_the_csp_source() {
        assert_eq!(
            tile_source("https://tile.openstreetmap.org/{z}/{x}/{y}.png").unwrap(),
            "https://tile.openstreetmap.org"
        );
        assert_eq!(
            tile_source("https://{s}.tile.example.com:8443/osm/{z}/{x}/{y}{r}.png?key=1").unwrap(),
            "https://*.tile.example.com:8443"
        );
        for bad in [
            "http://tile.example.com/{z}/{x}/{y}.png",
            "https://tile.example.com/{z}/{x}.png",
            "https://tile.example.com/{z}/{x}/{y}.png'; script-src *",
            "https://tile example.com/{z}/{x}/{y}.png",
            "https://user@tile.example.com/{z}/{x}/{y}.png",
            "https://{z}/{x}/{y}",
            "https:///{z}/{x}/{y}",
            "https://tile.example.com/{z}/{x}/{y}/{token}.png",
            "javascript:alert(1)//{z}{x}{y}",
        ] {
            assert!(tile_source(bad).is_err(), "{bad}");
        }
        let ok = validate_tiles(MapTiles {
            tile_url: " https://tile.openstreetmap.org/{z}/{x}/{y}.png ".into(),
            attribution: "© OpenStreetMap contributors".into(),
        })
        .unwrap();
        assert_eq!(
            ok.tile_url,
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
        );
        assert!(
            validate_tiles(MapTiles {
                tile_url: ok.tile_url.clone(),
                attribution: String::new(),
            })
            .is_err()
        );
        assert!(
            validate_tiles(MapTiles::default()).is_ok(),
            "empty: offline map"
        );
    }

    #[test]
    fn parses_categories_and_windows() {
        assert_eq!(parse_categories(None).unwrap(), Category::ALL);
        assert_eq!(parse_categories(Some(" ")).unwrap(), Category::ALL);
        assert_eq!(
            parse_categories(Some("blocked, answered,blocked")).unwrap(),
            [Category::Blocked, Category::Answered]
        );
        assert!(parse_categories(Some("answered,spam")).is_err());
        assert_eq!(window(Range::Hour), Duration::minutes(60));
        assert_eq!(window(Range::Month), Duration::days(30));
    }
}
