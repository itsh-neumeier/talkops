//! Wi-Fi voice survey (ADR 0041): start, self-dial, live data, markers,
//! call record, analysis, export, audio ranges and permissions.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::tenant::TenantId;
use talkops_core::wifi_survey::{self as ws, MediaSample, RadioSample};
use uuid::Uuid;

const T: TenantId = TenantId::DEFAULT;

fn actions(xml: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("action"))
        .map(|n| {
            (
                n.attribute("application").unwrap().to_owned(),
                n.attribute("data").unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

async fn self_dial(
    router: &axum::Router,
    ext: &str,
    device: &str,
    call: Uuid,
) -> Vec<(String, String)> {
    let call = call.to_string();
    let (status, xml) = fs_post(
        router,
        "/fs/xml",
        &[
            ("section", "dialplan"),
            ("Caller-Context", "internal"),
            ("Caller-Destination-Number", "*39"),
            ("variable_talkops_tenant_id", &T.to_string()),
            ("variable_talkops_extension_id", ext),
            ("variable_talkops_device_id", device),
            ("variable_uuid", &call),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    actions(&xml)
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn survey_flow(db: PgPool) {
    let state = state(db.clone());
    let recordings = state.media.recordings.clone();
    let router = talkops_api::app(state, None);
    let admin = setup_admin(&router).await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number": "300", "display_name": "Walker"}),
        )
        .await;
    let ext_id = ext["id"].as_str().unwrap().to_owned();
    let (status, created) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "AX83H", "kind": "wifi"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let device = created["device_id"].as_str().unwrap().to_owned();
    let (_, desk) = admin
        .post(
            &format!("/api/v1/extensions/{ext_id}/devices"),
            json!({"name": "Desk", "kind": "desk"}),
        )
        .await;
    let desk = desk["device_id"].as_str().unwrap().to_owned();

    let (status, devices) = admin.get("/api/v1/wifi-surveys/devices").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(devices.as_array().unwrap().len(), 1);
    assert_eq!(devices[0]["extension_number"], "300");
    let (_, unifi) = admin
        .get(&format!("/api/v1/wifi-surveys/devices/{device}/unifi"))
        .await;
    assert_eq!(unifi["status"], "not_configured");

    // Only Wi-Fi devices.
    let (status, _) = admin
        .post(
            "/api/v1/wifi-surveys",
            json!({"device_id": desk, "mode": "originate"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Without FreeSWITCH the call cannot start; the survey fails.
    let (status, _) = admin
        .post(
            "/api/v1/wifi-surveys",
            json!({"device_id": device, "mode": "originate"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, list) = admin.get("/api/v1/wifi-surveys").await;
    assert_eq!(list[0]["state"], "failed");

    // An unarmed *39 starts nothing.
    let a = self_dial(&router, &ext_id, &device, Uuid::new_v4()).await;
    assert!(a.iter().any(|(app, _)| app == "hangup"), "{a:?}");
    assert!(!a.iter().any(|(app, _)| app == "socket"));

    // Self-dial: armed, then claimed by the device's call.
    let (status, armed) = admin
        .post(
            "/api/v1/wifi-surveys",
            json!({"device_id": device, "mode": "self_dial", "label": "Before"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{armed}");
    assert_eq!(armed["state"], "armed");
    let id: Uuid = armed["id"].as_str().unwrap().parse().unwrap();
    let call = Uuid::new_v4();
    let a = self_dial(&router, &ext_id, &device, call).await;
    assert!(
        a.iter()
            .any(|(app, d)| app == "set" && d == "talkops_app=wifi_survey"),
        "{a:?}"
    );
    assert!(
        a.iter()
            .any(|(app, d)| app == "set" && *d == format!("talkops_survey_id={id}"))
    );
    assert!(a.iter().any(|(app, _)| app == "socket"));

    // The socket handler marks the answer; the collector writes samples.
    let answered = chrono::Utc::now() - chrono::Duration::seconds(20);
    let file = ws::recording_file(T, id);
    assert!(
        ws::mark_answered(&db, T, id, answered, Some(&file))
            .await
            .unwrap()
    );
    for s in 1..=20 {
        let lost = if s == 10 { 8 } else { 0 };
        ws::add_media(
            &db,
            T,
            id,
            chrono::Utc::now(),
            &MediaSample {
                offset_ms: s * 1000,
                interval_ms: 1000,
                received: 50 - lost,
                expected: 50,
                flaws: lost,
                mos: Some(4.3),
                jitter_max_ms: None,
                rtcp_fraction_lost: None,
                rtcp_jitter_ms: Some(4.0),
            },
        )
        .await
        .unwrap();
        ws::add_radio(
            &db,
            T,
            id,
            chrono::Utc::now(),
            &RadioSample {
                offset_ms: s * 1000,
                ap_mac: Some(
                    if s < 10 {
                        "f4:92:bf:00:00:01"
                    } else {
                        "f4:92:bf:00:00:02"
                    }
                    .into(),
                ),
                ap_name: Some(if s < 10 { "Hall" } else { "Office" }.into()),
                bssid: None,
                radio: Some("na".into()),
                channel: Some(36),
                rssi: Some(-62),
                tx_rate_kbps: Some(144_000),
                rx_rate_kbps: Some(144_000),
                error: None,
            },
        )
        .await
        .unwrap();
    }

    let (status, live) = admin
        .get(&format!("/api/v1/wifi-surveys/{id}/live?since=15000"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(live["timeline"]["media"].as_array().unwrap().len(), 5);
    assert_eq!(live["now"]["radio"]["ap_name"], "Office");
    assert_eq!(live["now"]["rating_5s"], "clean");
    assert_eq!(live["analysis"]["roams"][0]["rating"], "bad");

    let (status, marker) = admin
        .post(
            &format!("/api/v1/wifi-surveys/{id}/markers"),
            json!({"marker": "location", "note": "Stairs"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{marker}");
    let (status, _) = admin
        .put(
            &format!(
                "/api/v1/wifi-surveys/{id}/markers/{}",
                marker["id"].as_str().unwrap()
            ),
            json!({"note": "Stairs 2"}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // A running survey cannot be deleted.
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/wifi-surveys/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // The call record ends the survey and queues the analysis.
    let wav = recordings.join(&file);
    std::fs::create_dir_all(wav.parent().unwrap()).unwrap();
    let mut data = b"RIFF\0\0\0\0WAVE".to_vec();
    data.extend(std::iter::repeat_n(7u8, 1000));
    std::fs::write(&wav, &data).unwrap();
    let start = answered.timestamp_micros() + 9_500_000;
    let xml = format!(
        r#"<?xml version="1.0"?><cdr><call-stats><audio><inbound><mos>4.2</mos></inbound><error-log><error-period><start>{start}</start><stop>{}</stop><flaws>8</flaws></error-period></error-log></audio></call-stats><variables><uuid>{call}</uuid><talkops_survey_id>{id}</talkops_survey_id><talkops_tenant_id>{T}</talkops_tenant_id><hangup_cause>NORMAL_CLEARING</hangup_cause></variables></cdr>"#,
        start + 3_000_000
    );
    let (status, _) = fs_post(&router, "/fs/cdr", &[("cdr", &xml)]).await;
    assert!(status.is_success());
    let s = ws::get(&db, T, id).await.unwrap();
    assert_eq!(s.state, "analysing");
    let (_, tl) = admin
        .get(&format!("/api/v1/wifi-surveys/{id}/timeline"))
        .await;
    let kinds: Vec<&str> = tl["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"error_period") && kinds.contains(&"marker"),
        "{kinds:?}"
    );
    // The worker job.
    ws::analyse_and_store(&db, T, id).await.unwrap();
    let (_, details) = admin.get(&format!("/api/v1/wifi-surveys/{id}")).await;
    assert_eq!(details["survey"]["state"], "analysed");
    assert_eq!(details["roams"][0]["from_ap_name"], "Hall");
    assert_eq!(details["roams"][0]["gap_ms"], 160);
    assert_eq!(details["survey"]["summary"]["bad"], 1);

    // Export and comparison.
    let res = raw(
        &router,
        Request::get(format!("/api/v1/wifi-surveys/{id}/export?format=csv"))
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let csv = body_string(res).await;
    assert!(csv.starts_with("offset_s,received"));
    assert!(csv.contains("Hall > Office,bad,160"), "{csv}");
    let (status, cmp) = admin
        .get(&format!("/api/v1/wifi-surveys/compare?a={id}&b={id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cmp["pairs"][0]["a_count"], 1);

    // Playback from a position: a byte range of the recording.
    let res = raw(
        &router,
        Request::get(format!("/api/v1/wifi-surveys/{id}/audio"))
            .header(header::COOKIE, &admin.cookie)
            .header(header::RANGE, "bytes=100-199")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        res.headers()[header::CONTENT_RANGE],
        format!("bytes 100-199/{}", data.len()).as_str()
    );
    assert_eq!(body_string(res).await.len(), 100);

    // Settings: validation and re-analysis.
    let (_, mut settings) = admin.get("/api/v1/wifi-surveys/settings").await;
    assert_eq!(settings["clean_ms"], 50);
    settings["clean_ms"] = json!(500);
    let (status, _) = admin
        .put("/api/v1/wifi-surveys/settings", settings.clone())
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    settings["clean_ms"] = json!(100);
    settings["bad_ms"] = json!(200);
    let (status, _) = admin.put("/api/v1/wifi-surveys/settings", settings).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin
        .post(&format!("/api/v1/wifi-surveys/{id}/analyse"), json!({}))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    ws::analyse_and_store(&db, T, id).await.unwrap();
    let (_, details) = admin.get(&format!("/api/v1/wifi-surveys/{id}")).await;
    assert_eq!(details["roams"][0]["rating"], "audible");

    // Audit entries without secrets or notes.
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE entity_type = 'wifi_survey'")
            .fetch_all(&db)
            .await
            .unwrap();
    for a in [
        "wifi_survey.start",
        "wifi_survey.marker",
        "wifi_survey.export",
        "wifi_survey.settings",
    ] {
        assert!(actions.iter().any(|x| x == a), "{a}: {actions:?}");
    }
    let details: Vec<Value> =
        sqlx::query_scalar("SELECT details FROM audit_log WHERE action = 'wifi_survey.marker'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert!(!details[0].to_string().contains("Stairs"));

    // Delete removes the recording.
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/wifi-surveys/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!wav.exists());
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn permissions_and_unifi_connection(db: PgPool) {
    let router = router(db.clone());
    let admin = setup_admin(&router).await;
    let (status, _) = admin
        .post(
            "/api/v1/users",
            json!({"username": "bob", "display_name": "Bob", "password": "correct-horse-2", "role": "user"}),
        )
        .await;
    assert!(status.is_success());
    let bob = login(&router, "bob", "correct-horse-2").await.unwrap();
    for path in [
        "/api/v1/wifi-surveys",
        "/api/v1/wifi-surveys/devices",
        "/api/v1/unifi/connection",
    ] {
        let (status, _) = bob.get(path).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }

    // The API key is required, stored encrypted and never returned.
    let (status, _) = admin
        .put(
            "/api/v1/unifi/connection",
            json!({"url": "https://192.168.1.1"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = admin
        .put(
            "/api/v1/unifi/connection",
            json!({"url": "http://192.168.1.1", "api_key": "k-secret-1"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, view) = admin
        .put(
            "/api/v1/unifi/connection",
            json!({"url": "https://192.168.1.1/", "api_key": "k-secret-1"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["has_api_key"], true);
    let (_, view) = admin.get("/api/v1/unifi/connection").await;
    assert_eq!(view["connection"]["url"], "https://192.168.1.1");
    assert!(!view.to_string().contains("k-secret-1"));
    let audit: Vec<String> = sqlx::query_scalar("SELECT details::text FROM audit_log")
        .fetch_all(&db)
        .await
        .unwrap();
    assert!(audit.iter().all(|d| !d.contains("k-secret-1")));
    let (status, _) = admin.call("DELETE", "/api/v1/unifi/connection", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
