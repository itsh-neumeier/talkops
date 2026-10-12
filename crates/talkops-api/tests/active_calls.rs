//! Exercise real ESL replies, directory queries and live content authorization.
mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use uuid::Uuid;

async fn esl(
    state: &talkops_api::AppState,
    rows: Arc<RwLock<Value>>,
    dumps: HashMap<String, String>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (read, mut write) = socket.into_split();
        let mut read = BufReader::new(read);
        write
            .write_all(b"Content-Type: auth/request\n\n")
            .await
            .unwrap();
        loop {
            let mut command = String::new();
            if read.read_line(&mut command).await.unwrap() == 0 {
                break;
            }
            let mut line = String::new();
            loop {
                line.clear();
                if read.read_line(&mut line).await.unwrap() == 0 || line == "\n" {
                    break;
                }
            }
            let reply = if command.starts_with("auth ") {
                "Content-Type: command/reply\nReply-Text: +OK accepted\n\n".into()
            } else {
                let body = if command.trim() == "api show calls as json" {
                    rows.read().await.to_string()
                } else {
                    dumps
                        .get(
                            command
                                .trim()
                                .strip_prefix("api uuid_dump ")
                                .unwrap_or_default(),
                        )
                        .cloned()
                        .unwrap_or_else(|| "-ERR unknown\n".into())
                };
                format!(
                    "Content-Type: api/response\nContent-Length: {}\n\n{body}",
                    body.len()
                )
            };
            if write.write_all(reply.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    state
        .telephony
        .esl
        .spawn_supervisor(addr.to_string(), "test".into());
    for _ in 0..50 {
        if state.telephony.esl.get().await.is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("ESL fixture did not connect");
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn resolves_contacts_and_protects_live_content(db: PgPool) {
    let state = state(db.clone());
    let router = talkops_api::app(state.clone(), None);
    let admin = setup_admin(&router).await;
    let (_, ext) = admin
        .post(
            "/api/v1/extensions",
            json!({"number":"101","display_name":"Family Zweig"}),
        )
        .await;
    let (_, device) = admin
        .post(
            &format!("/api/v1/extensions/{}/devices", ext["id"].as_str().unwrap()),
            json!({"name":"Desk","kind":"desk"}),
        )
        .await;
    let (_, section) = admin
        .post("/api/v1/phonebook-sections", json!({"name":"Family"}))
        .await;
    admin.post("/api/v1/contacts", json!({"name":"Family Vollmeier (Maria)","phone_other":"095458589","section_id":section["id"]})).await;
    let tenant = talkops_core::tenant::TenantId::DEFAULT;
    let trunk: Uuid = sqlx::query_scalar(
        "INSERT INTO trunks (tenant_id, name, preset) VALUES ($1, 'Test', 'generic') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&db)
    .await
    .unwrap();
    let number: Uuid = sqlx::query_scalar("INSERT INTO numbers (tenant_id, trunk_id, e164) VALUES ($1, $2, '+4995431241') RETURNING id").bind(tenant).bind(trunk).fetch_one(&db).await.unwrap();
    sqlx::query("UPDATE tenant_settings SET area_code = '9545', transcription_enabled = true")
        .execute(&db)
        .await
        .unwrap();
    let outgoing = Uuid::new_v4();
    let incoming = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    let row = |uuid: Uuid| json!({"uuid":uuid,"created_epoch":"1791450000","cid_num":"095458589","cid_name":"","dest":"101","callee_num":device["sip_username"],"callee_name":"Outbound Call","b_cid_num":"095458589","b_uuid":Uuid::new_v4(),"callstate":"ACTIVE"});
    let rows = Arc::new(RwLock::new(
        json!({"rows":[row(outgoing),row(incoming),row(foreign)]}),
    ));
    let dump = |call: Uuid, direction: &str, caller: &str| {
        format!(
            "variable_talkops_tenant_id: {tenant}\nvariable_talkops_direction: {direction}\nvariable_talkops_extension_id: {}\nvariable_talkops_number_id: {number}\nvariable_talkops_caller_number: {caller}\nvariable_talkops_destination: %2B4995458589\nvariable_talkops_recording: {tenant}/2026-10/{call}.wav\nCaller-Channel-Answered-Time: 1791450000000000\n\n",
            ext["id"].as_str().unwrap()
        )
    };
    let mut dumps = HashMap::new();
    dumps.insert(outgoing.to_string(), dump(outgoing, "outbound", "101"));
    dumps.insert(incoming.to_string(), dump(incoming, "inbound", "095458589"));
    dumps.insert(
        foreign.to_string(),
        format!(
            "variable_talkops_tenant_id: {}\nvariable_talkops_direction: inbound\n\n",
            Uuid::new_v4()
        ),
    );
    esl(&state, rows.clone(), dumps).await;
    let (status, calls) = admin.get("/api/v1/telephony/calls").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.as_array().unwrap().len(), 2);
    assert_eq!(calls[0]["caller"]["name"], "Family Zweig");
    assert_eq!(calls[0]["caller"]["number"], "+4995431241");
    assert_eq!(calls[0]["called"]["name"], "Family Vollmeier (Maria)");
    assert_eq!(calls[0]["called"]["number"], "+4995458589");
    assert_eq!(calls[1]["caller"]["name"], "Family Vollmeier (Maria)");
    assert_eq!(calls[1]["called"]["name"], "Family Zweig");
    assert_eq!(calls[1]["called"]["number"], "+4995431241");
    assert_eq!(calls[0]["transcript_available"], true);
    assert!(calls[0].get("metadata").is_none());
    admin.post("/api/v1/users", json!({"username":"operator","display_name":"Operator","password":"operator-password","role":"operator"})).await;
    let operator = login(&router, "operator", "operator-password")
        .await
        .unwrap();
    assert_eq!(
        operator.get("/api/v1/telephony/calls").await.1[0]["transcript_available"],
        false
    );
    let path = format!("/api/v1/telephony/calls/{outgoing}/transcript");
    assert_eq!(
        operator.post(&path, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let operator_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = 'operator'")
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE extensions SET user_id = $1 WHERE id = $2")
        .bind(operator_id)
        .bind(Uuid::parse_str(ext["id"].as_str().unwrap()).unwrap())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        operator.get("/api/v1/telephony/calls").await.1[0]["transcript_available"],
        true
    );
    assert_eq!(operator.post(&path, json!({})).await.0, StatusCode::OK);
    assert_eq!(
        operator.call("DELETE", &path, None).await.0,
        StatusCode::NO_CONTENT
    );
    sqlx::query("UPDATE extensions SET user_id = NULL")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        operator.post(&path, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, preview) = admin.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["status"], "queued");
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'read_live_transcript'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(count, 2);
    admin.post(&path, json!({})).await;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'read_live_transcript'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(count, 2);
    *rows.write().await = json!({"rows":[]});
    assert_eq!(admin.post(&path, json!({})).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        admin.call("DELETE", &path, None).await.0,
        StatusCode::NO_CONTENT
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM live_transcript_previews")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
