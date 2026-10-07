//! LDAP logins against a real OpenLDAP server (tests/ldap/start.sh). Skipped
//! unless TALKOPS_TEST_LDAP_URL is set.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use serde_json::json;
use sqlx::PgPool;

fn ldap_url() -> Option<String> {
    std::env::var("TALKOPS_TEST_LDAP_URL")
        .ok()
        .filter(|u| !u.is_empty())
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn ldap_logins(db: PgPool) {
    let pool = db.clone();
    let Some(url) = ldap_url() else {
        eprintln!("TALKOPS_TEST_LDAP_URL not set, skipping");
        return;
    };
    let router = router(pool);
    let admin = setup_admin(&router).await;
    let (status, s) = admin
        .put(
            "/api/v1/settings/identity",
            json!({"ldap_enabled": true, "ldap_url": url,
                   "ldap_bind_dn": "cn=svc,ou=people,dc=example,dc=org",
                   "ldap_bind_password": "svcpw", "ldap_base_dn": "dc=example,dc=org",
                   "ldap_user_filter": "(&(objectClass=inetOrgPerson)(uid={username}))",
                   "admin_group": "pbx-admins", "user_group": "cn=staff,ou=groups,dc=example,dc=org"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["ldap_has_password"], true);

    // Connection test with a user lookup.
    let (status, t) = admin
        .post(
            "/api/v1/settings/identity/ldap-test",
            json!({"username": "anna"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    assert_eq!(t["dn"], "uid=anna,ou=people,dc=example,dc=org");
    assert_eq!(t["role"], "admin");
    let (status, _) = admin
        .post(
            "/api/v1/settings/identity/ldap-test",
            json!({"username": "nobody"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Directory login creates the user with the mapped role.
    let anna = login(&router, "anna", "anna-ldap-pw")
        .await
        .expect("ldap login");
    let (_, me) = anna.get("/api/v1/auth/me").await;
    assert_eq!(me["user"]["auth_source"], "ldap");
    assert_eq!(me["user"]["role"], "admin");
    assert_eq!(me["user"]["display_name"], "Anna Beispiel");
    assert_eq!(me["user"]["email"], "anna@example.org");
    assert!(login(&router, "anna", "wrong").await.is_none());
    assert!(
        login(&router, "anna", "").await.is_none(),
        "no unauthenticated bind"
    );
    // Filter injection does not match other users.
    assert!(login(&router, "*", "anna-ldap-pw").await.is_none());
    // Ben exists but is not in the staff group.
    assert!(login(&router, "ben", "ben-ldap-pw").await.is_none());
    // The local admin still logs in locally.
    assert!(login(&router, "admin", "correct-horse").await.is_some());
    // LDAP accounts can use TOTP as a second factor.
    let (_, st) = anna.get("/api/v1/auth/totp").await;
    assert_eq!(st["available"], true);

    // The hourly sync disables accounts that lost their group.
    admin
        .put(
            "/api/v1/settings/identity",
            json!({"ldap_enabled": true, "ldap_url": url,
                   "ldap_bind_dn": "cn=svc,ou=people,dc=example,dc=org",
                   "ldap_base_dn": "dc=example,dc=org",
                   "ldap_user_filter": "(&(objectClass=inetOrgPerson)(uid={username}))",
                   "user_group": "nobody-is-here"}),
        )
        .await;
    let secrets = talkops_core::crypto::SecretBox::from_hex(KEY).unwrap();
    let (disabled, _) = talkops_api::ldap::sync_users(&db, &secrets).await.unwrap();
    assert_eq!(disabled, 1);
    assert_eq!(
        anna.get("/api/v1/auth/me").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(login(&router, "anna", "anna-ldap-pw").await.is_none());

    // A wrong service password shows up in the connection test.
    admin
        .put(
            "/api/v1/settings/identity",
            json!({"ldap_enabled": true, "ldap_url": url,
                   "ldap_bind_dn": "cn=svc,ou=people,dc=example,dc=org",
                   "ldap_bind_password": "nope", "ldap_base_dn": "dc=example,dc=org"}),
        )
        .await;
    let res = raw(
        &router,
        Request::post("/api/v1/settings/identity/ldap-test")
            .header(header::COOKIE, &admin.cookie)
            .header("x-requested-with", "TalkOps")
            .header("x-csrf-token", &admin.csrf)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}
