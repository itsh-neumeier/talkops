mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::PgPool;
use talkops_core::{fax, tenant::TenantId};
use uuid::Uuid;
const T: TenantId = TenantId::DEFAULT;

async fn fixture(pool: &PgPool) -> (talkops_api::AppState, Uuid, Uuid) {
    let s = state(pool.clone());
    let user:Uuid=sqlx::query_scalar("INSERT INTO users(tenant_id,username,display_name,role) VALUES($1,'faxadmin','Fax Admin','admin') RETURNING id").bind(T).fetch_one(pool).await.unwrap();
    let trunk: Uuid = sqlx::query_scalar(
        "INSERT INTO trunks(tenant_id,name,preset) VALUES($1,'fax trunk','generic') RETURNING id",
    )
    .bind(T)
    .fetch_one(pool)
    .await
    .unwrap();
    let account:Uuid=sqlx::query_scalar("INSERT INTO trunk_accounts(tenant_id,trunk_id,username,password_enc) VALUES($1,$2,'fax','encrypted-fixture') RETURNING id").bind(T).bind(trunk).fetch_one(pool).await.unwrap();
    let number:Uuid=sqlx::query_scalar("INSERT INTO numbers(tenant_id,trunk_id,account_id,e164) VALUES($1,$2,$3,'+49123456') RETURNING id").bind(T).bind(trunk).bind(account).fetch_one(pool).await.unwrap();
    let cfg = fax::Settings {
        enabled: true,
        global_number_id: Some(number),
        ..Default::default()
    };
    fax::save_settings(pool, T, &cfg, Some("mail-secret"), &s.secrets)
        .await
        .unwrap();
    (s, user, number)
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn mail_admission_is_atomic_whitelisted_and_body_free(pool: PgPool) {
    let (s, user, _) = fixture(&pool).await;
    fax::save_whitelist(
        &pool,
        T,
        &[fax::Whitelist {
            email: "Allowed@Example.com".into(),
            user_id: user,
            enabled: true,
        }],
    )
    .await
    .unwrap();
    let input = |uid: &str, sender: &str, subject: &str| {
        serde_json::from_value(json!({"uid":uid,"validity":"1","sender":sender,"subject":subject,"automatic":false,"files":[{"name":"one.pdf","data":"JVBERi0xLjQ="}],"error":"","fingerprint":format!("fingerprint-{uid}")})).unwrap()
    };
    talkops_api::fax::admit_mail(
        &s,
        T,
        input("1", "unknown@example.com", "+49123456"),
        "mailbox",
    )
    .await
    .unwrap();
    talkops_api::fax::admit_mail(
        &s,
        T,
        input("2", "allowed@example.com", "Re: +49123456"),
        "mailbox",
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM faxes")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let notices: Vec<String> =
        sqlx::query_scalar("SELECT recipient FROM fax_mail_outbox ORDER BY recipient")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(notices, ["allowed@example.com", "unknown@example.com"]);
    talkops_api::fax::admit_mail(
        &s,
        T,
        input("3", "Allowed@Example.com", "+49123456"),
        "mailbox",
    )
    .await
    .unwrap();
    talkops_api::fax::admit_mail(
        &s,
        T,
        input("3", "Allowed@Example.com", "+49123456"),
        "mailbox",
    )
    .await
    .unwrap();
    let faxes = fax::list(&pool, T, user, false).await.unwrap();
    assert_eq!(faxes.len(), 1);
    assert_eq!(faxes[0].header, "allowed@example.com");
    assert_eq!(faxes[0].sender_email, "allowed@example.com");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM jobs WHERE kind='fax.prepare'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(
        fax::mail_user(&pool, T, "otherallowed@example.com")
            .await
            .is_err()
    );
    fax::save_whitelist(&pool, T, &[]).await.unwrap();
    assert!(
        fax::mail_user(&pool, T, "allowed@example.com")
            .await
            .is_err()
    );
    assert_eq!(
        fax::password(&pool, T, &s.secrets).await.unwrap().unwrap(),
        "mail-secret"
    );
    let enc: String =
        sqlx::query_scalar("SELECT imap_password_enc FROM fax_settings WHERE tenant_id=$1")
            .bind(T)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!enc.contains("mail-secret"));
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn fax_routes_and_document_access_are_scoped(pool: PgPool) {
    let (s, owner, _) = fixture(&pool).await;
    let other:Uuid=sqlx::query_scalar("INSERT INTO users(tenant_id,username,display_name) VALUES($1,'reader','Reader') RETURNING id").bind(T).fetch_one(&pool).await.unwrap();
    let e = fax::Endpoint {
        id: Uuid::new_v4(),
        name: "Incoming".into(),
        owner_id: owner,
        send_users: vec![],
        receive_users: vec![other],
        reception_email: "inbox@example.com".into(),
        enabled: true,
    };
    fax::save_endpoint(&pool, T, &e).await.unwrap();
    talkops_core::numbering::check_destination(
        &pool,
        T,
        talkops_core::trunks::NumberDestination::Fax,
        Some(e.id),
    )
    .await
    .unwrap();
    let id:Uuid=sqlx::query_scalar("INSERT INTO faxes(tenant_id,owner_id,endpoint_id,direction,state) VALUES($1,$2,$3,'inbound','succeeded') RETURNING id").bind(T).bind(owner).bind(e.id).fetch_one(&pool).await.unwrap();
    assert_eq!(fax::list(&pool, T, other, false).await.unwrap().len(), 1);
    fax::save_endpoint(
        &pool,
        T,
        &fax::Endpoint {
            receive_users: vec![],
            ..e
        },
    )
    .await
    .unwrap();
    assert!(fax::list(&pool, T, other, false).await.unwrap().is_empty());
    assert!(fax::get(&pool, TenantId(Uuid::new_v4()), id).await.is_err());
    let text = fax::Template {
        header: "Saved {page}/{pages}".into(),
        footer: "Footer".into(),
    };
    fax::save_template(&pool, T, owner, &text).await.unwrap();
    assert_eq!(
        fax::template_for(&pool, T, owner).await.unwrap().footer,
        "Footer"
    );
    let disabled = fax::Settings {
        enabled: false,
        ..fax::settings(&pool, T).await.unwrap()
    };
    fax::save_settings(&pool, T, &disabled, None, &s.secrets)
        .await
        .unwrap();
    assert!(fax::resolve_number(&pool, T, owner, None).await.is_err());
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn api_settings_permissions_and_prepared_submit(pool: PgPool) {
    let router = router(pool.clone());
    let admin = setup_admin(&router).await;
    let (status, _) = admin
        .post(
            "/api/v1/users",
            json!({"username":"bob","display_name":"Bob","role":"user","password":"correct-horse"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let bob = login(&router, "bob", "correct-horse").await.unwrap();
    assert_eq!(
        bob.get("/api/v1/settings/fax").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bob.post(
            "/api/v1/faxes",
            json!({"destination":"+49123456","files":[]})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, view) = admin.get("/api/v1/settings/fax").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["config"]["enabled"], false);
    assert!(view.get("password").is_none());
    let (status, _) = admin
        .put("/api/v1/settings/fax", json!({"config":view["config"]}))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        admin
            .put("/api/v1/settings/fax/whitelist", json!([]))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(admin.get("/api/v1/faxes/options").await.0, StatusCode::OK);
    assert_eq!(admin.get("/api/v1/faxes/template").await.0, StatusCode::OK);
    assert_eq!(admin.get("/api/v1/fax-endpoints").await.0, StatusCode::OK);
}
#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn cdr_results_are_idempotent_and_partial_is_not_success(pool: PgPool) {
    let (_, user, number) = fixture(&pool).await;
    let id = Uuid::new_v4();
    let call = Uuid::new_v4();
    sqlx::query("INSERT INTO faxes(id,tenant_id,owner_id,direction,state,number_id,pages,call_uuid,sender_email) VALUES($1,$2,$3,'outbound','sending',$4,2,$5,'allowed@example.com')").bind(id).bind(T).bind(user).bind(number).bind(call).execute(&pool).await.unwrap();
    let xml = format!(
        "<cdr><variables><uuid>{call}</uuid><fax_success>1</fax_success><fax_result_code>0</fax_result_code><fax_document_transferred_pages>2</fax_document_transferred_pages><hangup_cause>NORMAL_CLEARING</hangup_cause></variables></cdr>"
    );
    talkops_api::fax::ingest_cdr(&pool, &xml).await.unwrap();
    talkops_api::fax::ingest_cdr(&pool, &xml).await.unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().state, "succeeded");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM fax_mail_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE faxes SET state='sending' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let partial = xml.replace(
        "<fax_document_transferred_pages>2",
        "<fax_document_transferred_pages>1",
    );
    talkops_api::fax::ingest_cdr(&pool, &partial).await.unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().state, "failed");
    let busy = format!(
        "<cdr><variables><uuid>{call}</uuid><hangup_cause>USER_BUSY</hangup_cause></variables></cdr>"
    );
    sqlx::query("UPDATE faxes SET state='dialing',attempts=1 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    talkops_api::fax::ingest_cdr(&pool, &busy).await.unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().state, "queued");
    sqlx::query("UPDATE faxes SET state='dialing',attempts=3 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    talkops_api::fax::ingest_cdr(&pool, &busy).await.unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().state, "failed");
    // A durable CDR can settle a transmission after control-plane loss.
    sqlx::query("UPDATE faxes SET state='unknown' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    talkops_api::fax::ingest_cdr(&pool, &xml).await.unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().state, "succeeded");
    let incoming = Uuid::new_v4();
    sqlx::query("INSERT INTO faxes(id,tenant_id,owner_id,direction,state,call_uuid) VALUES($1,$2,$3,'inbound','sending',$1)")
        .bind(incoming).bind(T).bind(user).execute(&pool).await.unwrap();
    talkops_api::fax::ingest_cdr(
        &pool,
        &partial.replace(&call.to_string(), &incoming.to_string()),
    )
    .await
    .unwrap();
    talkops_api::fax::ingest_cdr(
        &pool,
        &partial.replace(&call.to_string(), &incoming.to_string()),
    )
    .await
    .unwrap();
    assert_eq!(
        fax::get(&pool, T, incoming).await.unwrap().state,
        "received"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM jobs WHERE kind='fax.receive'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn prepared_documents_need_submit_and_obey_access_checks(pool: PgPool) {
    use axum::{
        body::Body,
        http::{Request, header},
    };
    let initial = router(pool.clone());
    let mut admin = setup_admin(&initial).await;
    let (s, _, number) = fixture(&pool).await;
    let router = talkops_api::app(s.clone(), None);
    admin.router = router.clone();
    let owner: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username='admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let payload = json!({"destination":"+49654321","number_id":number,"files":[{"name":"a.pdf","data":"JVBERi0xLjQ="}]});
    let (status, f) = admin.post("/api/v1/faxes", payload).await;
    assert_eq!(status, StatusCode::OK, "{f}");
    assert_eq!(f["state"], "preparing");
    let id: Uuid = f["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(fax::get(&pool, T, id).await.unwrap().owner_id, owner);
    assert_eq!(
        admin
            .post(&format!("/api/v1/faxes/{id}/submit"), json!({}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE faxes SET state='draft',pages=1 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    tokio::fs::write(
        talkops_api::fax::directory(&s, T, id).join("document.pdf"),
        b"%PDF-fixture",
    )
    .await
    .unwrap();
    let response = raw(
        &router,
        Request::get(format!("/api/v1/faxes/{id}/document"))
            .header(header::COOKIE, &admin.cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::X_FRAME_OPTIONS], "SAMEORIGIN");
    let (_, bob) = admin
        .post(
            "/api/v1/users",
            json!({"username":"bob","display_name":"Bob","role":"user","password":"correct-horse"}),
        )
        .await;
    assert!(bob["id"].is_string());
    let bob = login(&router, "bob", "correct-horse").await.unwrap();
    assert_eq!(
        bob.get(&format!("/api/v1/faxes/{id}/document")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        admin
            .post(&format!("/api/v1/faxes/{id}/submit"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        admin
            .post(&format!("/api/v1/faxes/{id}/submit"), json!({}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        admin
            .post(&format!("/api/v1/faxes/{id}/cancel"), json!({}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, new) = admin
        .post(&format!("/api/v1/faxes/{id}/resend"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{new}");
    assert_ne!(new["id"], f["id"]);
    let new_id: Uuid = new["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("UPDATE faxes SET state='sending',call_uuid=$2 WHERE id=$1")
        .bind(new_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        admin
            .post(&format!("/api/v1/faxes/{new_id}/cancel"), json!({}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(fax::get(&pool, T, new_id).await.unwrap().state, "unknown");
    assert_eq!(
        admin
            .call("DELETE", &format!("/api/v1/faxes/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        admin.get("/api/v1/settings/fax/input-log").await.0,
        StatusCode::OK
    );
    assert_eq!(
        admin
            .put(
                "/api/v1/faxes/template",
                json!({"header":"Custom","footer":"Footer"})
            )
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        admin.call("DELETE", "/api/v1/faxes/template", None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_ne!(
        admin.get("/api/v1/faxes/template").await.1["header"],
        "Custom"
    );
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn rejection_outbox_delivers_once_without_original_content(pool: PgPool) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::time::{Duration, timeout};
    let (s, _, _) = fixture(&pool).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    talkops_core::mail::update(
        &pool,
        T,
        &s.secrets,
        &talkops_core::mail::SmtpInput {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port().into(),
            security: "none".into(),
            username: String::new(),
            password: None,
            from: "pbx@example.com".into(),
        },
    )
    .await
    .unwrap();
    let smtp = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut read = BufReader::new(read);
        write.write_all(b"220 fixture ESMTP\r\n").await.unwrap();
        let mut message = String::new();
        let mut data = false;
        loop {
            let mut line = String::new();
            if read.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            if data {
                if line == ".\r\n" {
                    write.write_all(b"250 queued\r\n").await.unwrap();
                    break;
                }
                message.push_str(&line);
            } else if line.starts_with("DATA") {
                write.write_all(b"354 send data\r\n").await.unwrap();
                data = true;
            } else if line.starts_with("EHLO") {
                write
                    .write_all(b"250-fixture\r\n250 8BITMIME\r\n")
                    .await
                    .unwrap();
            } else {
                write.write_all(b"250 OK\r\n").await.unwrap();
            }
        }
        message
    });
    let input = serde_json::from_value(json!({"uid":"smtp-1","validity":"1","sender":"denied@example.com","subject":"+49123456","automatic":false,"files":[],"error":"","fingerprint":"smtp-1"})).unwrap();
    talkops_api::fax::admit_mail(&s, T, input, "smtp-fixture")
        .await
        .unwrap();
    talkops_api::fax::spawn(s);
    let message = timeout(Duration::from_secs(15), smtp)
        .await
        .unwrap()
        .unwrap();
    assert!(message.contains("denied@example.com"));
    assert!(message.contains("Auto-Submitted: auto-replied"));
    assert!(!message.contains("Content-Disposition: attachment"));
    timeout(Duration::from_secs(5), async {
        loop {
            let sent: bool = sqlx::query_scalar(
                "SELECT sent FROM fax_mail_outbox WHERE recipient='denied@example.com'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if sent {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let attempts: i32 = sqlx::query_scalar(
        "SELECT attempts FROM fax_mail_outbox WHERE recipient='denied@example.com'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM faxes")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
