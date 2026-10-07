//! Prometheus metrics (`GET /metrics`, text format 0.0.4). Disabled unless
//! `TALKOPS_METRICS_TOKEN` is set; scrapers send it as bearer token.

use std::fmt::Write;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use crate::AppState;

/// Escapes a label value.
fn label(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Accumulates metric families in exposition format.
#[derive(Default)]
pub struct Metrics(String);

impl Metrics {
    fn family(&mut self, name: &str, kind: &str, help: &str) {
        let _ = writeln!(self.0, "# HELP {name} {help}");
        let _ = writeln!(self.0, "# TYPE {name} {kind}");
    }

    fn sample(&mut self, name: &str, labels: &[(&str, &str)], value: f64) {
        self.0.push_str(name);
        if !labels.is_empty() {
            let l: Vec<String> = labels
                .iter()
                .map(|(k, v)| format!("{k}=\"{}\"", label(v)))
                .collect();
            let _ = write!(self.0, "{{{}}}", l.join(","));
        }
        let _ = writeln!(self.0, " {value}");
    }

    pub fn gauge(&mut self, name: &str, help: &str, value: f64) {
        self.family(name, "gauge", help);
        self.sample(name, &[], value);
    }

    pub fn into_text(self) -> String {
        self.0
    }
}

fn authorized(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.metrics_token.as_deref() else {
        return false;
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    // Constant time: compare digests.
    talkops_core::crypto::token_digest(given) == talkops_core::crypto::token_digest(expected)
}

async fn database_metrics(db: &PgPool, m: &mut Metrics) -> Result<(), sqlx::Error> {
    let jobs: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT kind, status::text, count(*) FROM jobs
         WHERE status IN ('queued', 'running', 'failed') GROUP BY 1, 2 ORDER BY 1, 2",
    )
    .fetch_all(db)
    .await?;
    m.family(
        "talkops_jobs",
        "gauge",
        "Background jobs by kind and status.",
    );
    for (kind, status, n) in jobs {
        m.sample(
            "talkops_jobs",
            &[("kind", &kind), ("status", &status)],
            n as f64,
        );
    }

    let calls: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT direction::text, count(*), count(*) FILTER (WHERE answered_at IS NOT NULL)
         FROM cdr GROUP BY 1 ORDER BY 1",
    )
    .fetch_all(db)
    .await?;
    m.family(
        "talkops_calls_total",
        "counter",
        "Finished calls by direction.",
    );
    for (dir, n, _) in &calls {
        m.sample("talkops_calls_total", &[("direction", dir)], *n as f64);
    }
    m.family(
        "talkops_calls_answered_total",
        "counter",
        "Answered calls by direction.",
    );
    for (dir, _, n) in &calls {
        m.sample(
            "talkops_calls_answered_total",
            &[("direction", dir)],
            *n as f64,
        );
    }

    let (recordings, bytes): (i64, Option<i64>) =
        sqlx::query_as("SELECT count(*), sum(size_bytes)::bigint FROM recordings")
            .fetch_one(db)
            .await?;
    m.gauge(
        "talkops_recordings",
        "Stored call recordings.",
        recordings as f64,
    );
    m.gauge(
        "talkops_recordings_bytes",
        "Disk space of stored call recordings.",
        bytes.unwrap_or(0) as f64,
    );
    let new_vm: i64 =
        sqlx::query_scalar("SELECT count(*) FROM voicemail_messages WHERE status = 'new'")
            .fetch_one(db)
            .await?;
    m.gauge(
        "talkops_voicemail_new",
        "Unheard voicemail messages.",
        new_vm as f64,
    );

    let doors: Vec<(String, bool)> = sqlx::query_as(
        "SELECT name, online FROM door_stations
         WHERE events_enabled AND host <> '' ORDER BY name",
    )
    .fetch_all(db)
    .await?;
    m.family(
        "talkops_door_station_online",
        "gauge",
        "Whether the event stream of a door station is connected.",
    );
    for (name, online) in doors {
        m.sample(
            "talkops_door_station_online",
            &[("name", &name)],
            f64::from(u8::from(online)),
        );
    }
    Ok(())
}

/// Prometheus scrape endpoint.
pub async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.metrics_token.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response();
    }
    let mut m = Metrics::default();
    m.family("talkops_build_info", "gauge", "TalkOps version.");
    m.sample(
        "talkops_build_info",
        &[("version", talkops_core::VERSION)],
        1.0,
    );

    let live = state.telephony.snapshot().await;
    m.gauge(
        "talkops_freeswitch_connected",
        "Whether TalkOps is connected to FreeSWITCH's event socket.",
        f64::from(u8::from(live.connected)),
    );
    m.gauge(
        "talkops_registrations",
        "Registered SIP devices.",
        live.registrations.len() as f64,
    );
    let mut gateways: Vec<_> = live.gateways.values().collect();
    gateways.sort_by(|a, b| a.name.cmp(&b.name));
    m.family(
        "talkops_trunk_registered",
        "gauge",
        "Whether a SIP trunk account is registered at its provider.",
    );
    for g in &gateways {
        m.sample(
            "talkops_trunk_registered",
            &[("gateway", &g.name)],
            f64::from(u8::from(g.state == "REGED")),
        );
    }
    m.family(
        "talkops_trunk_up",
        "gauge",
        "Whether a SIP trunk answers OPTIONS pings.",
    );
    for g in &gateways {
        m.sample(
            "talkops_trunk_up",
            &[("gateway", &g.name)],
            f64::from(u8::from(g.status == "UP")),
        );
    }

    // "N total."
    if let Some(esl) = state.telephony.esl.get().await
        && let Ok(out) = esl.api("show calls count").await
        && let Some(n) = out
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<f64>().ok())
    {
        m.gauge("talkops_active_calls", "Calls in progress.", n);
    }

    let db_ok = database_metrics(&state.db, &mut m).await.is_ok();
    m.gauge(
        "talkops_database_up",
        "Whether the database answered the metrics queries.",
        f64::from(u8::from(db_ok)),
    );
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        m.into_text(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposition_format() {
        let mut m = Metrics::default();
        m.gauge("a_total", "Help text.", 2.0);
        m.family("b", "gauge", "B.");
        m.sample("b", &[("name", "Front \"door\"\n")], 1.0);
        assert_eq!(
            m.into_text(),
            "# HELP a_total Help text.\n# TYPE a_total gauge\na_total 2\n\
             # HELP b B.\n# TYPE b gauge\nb{name=\"Front \\\"door\\\"\\n\"} 1\n"
        );
    }
}
