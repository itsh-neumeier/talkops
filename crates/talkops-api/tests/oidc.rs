//! Single sign-on against a fake OpenID provider (ES256 keys, discovery,
//! token endpoint with PKCE check).

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use base64::Engine;
use common::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

const KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgb+ufEs+ZwDng1WPV
NPCdxk7sAbmUVWWYCcTjZ82v0YKhRANCAATT6ZvSZGf9bejDK+AUaFWRCd45/eok
d2ruhxwHb0NrCH4lSVeGueVfh7gJIwuYHyiKgNa9lrxIbwblJzKUO2Uc
-----END PRIVATE KEY-----
";
const KEY_X: &str = "0-mb0mRn_W3owyvgFGhVkQneOf3qJHdq7occB29Dawg";
const KEY_Y: &str = "fiVJV4a55V-HuAkjC5gfKIqA1r2WvEhvBuUnMpQ7ZRw";

/// PKCS#8 DER of the PEM key.
fn key_der() -> Vec<u8> {
    let b64: String = KEY_PEM
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap()
}

#[derive(Default)]
struct Idp {
    issuer: String,
    /// Claims of the next ID token (nonce is filled in from the login).
    claims: Mutex<Value>,
    nonce: Mutex<String>,
    challenge: Mutex<String>,
}

async fn token(
    State(idp): State<Arc<Idp>>,
    Form(f): Form<HashMap<String, String>>,
) -> axum::response::Response {
    let verifier = f.get("code_verifier").cloned().unwrap_or_default();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));
    if f.get("code").map(String::as_str) != Some("good-code")
        || challenge != *idp.challenge.lock().unwrap()
        || f.get("client_secret").map(String::as_str) != Some("client-secret")
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    }
    let mut claims = idp.claims.lock().unwrap().clone();
    let now = chrono::Utc::now().timestamp();
    claims["iss"] = json!(idp.issuer);
    claims["aud"] = json!("talkops");
    claims["iat"] = json!(now);
    claims["exp"] = json!(now + 300);
    if claims.get("nonce").is_none() {
        claims["nonce"] = json!(*idp.nonce.lock().unwrap());
    }
    let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    h.kid = Some("k1".into());
    let jwt = jsonwebtoken::encode(
        &h,
        &claims,
        &jsonwebtoken::EncodingKey::from_ec_der(&key_der()),
    )
    .unwrap();
    axum::Json(json!({"access_token": "x", "token_type": "Bearer", "id_token": jwt}))
        .into_response()
}

async fn fake_idp() -> Arc<Idp> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let idp = Arc::new(Idp {
        issuer: issuer.clone(),
        ..Default::default()
    });
    let app = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(move || {
                let issuer = issuer.clone();
                async move {
                    axum::Json(json!({
                        "issuer": issuer,
                        "authorization_endpoint": format!("{issuer}/authorize"),
                        "token_endpoint": format!("{issuer}/token"),
                        "jwks_uri": format!("{issuer}/jwks"),
                    }))
                }
            }),
        )
        .route(
            "/jwks",
            get(|| async {
                axum::Json(json!({"keys": [{"kty": "EC", "crv": "P-256", "kid": "k1",
                    "use": "sig", "alg": "ES256", "x": KEY_X, "y": KEY_Y}]}))
            }),
        )
        .route("/token", post(token))
        .with_state(idp.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    idp
}

fn query(url: &str) -> HashMap<String, String> {
    url.split_once('?')
        .map(|(_, q)| q)
        .unwrap_or_default()
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| {
            let v = v
                .replace("%3A", ":")
                .replace("%2F", "/")
                .replace("%20", " ");
            (k.to_owned(), v)
        })
        .collect()
}

/// Runs a whole SSO login; returns the final redirect and the cookie.
async fn sso(router: &Router, idp: &Idp, claims: Value) -> (String, Option<String>) {
    *idp.claims.lock().unwrap() = claims;
    let res = raw(
        router,
        Request::get("/api/v1/auth/oidc/start")
            .header(header::HOST, "pbx.test")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    assert!(
        loc.starts_with(&format!("{}/authorize?", idp.issuer)),
        "{loc}"
    );
    let q = query(&loc);
    assert_eq!(
        q["redirect_uri"],
        "http://pbx.test/api/v1/auth/oidc/callback"
    );
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["client_id"], "talkops");
    *idp.nonce.lock().unwrap() = q["nonce"].clone();
    *idp.challenge.lock().unwrap() = q["code_challenge"].clone();
    let res = raw(
        router,
        Request::get(format!(
            "/api/v1/auth/oidc/callback?code=good-code&state={}",
            q["state"]
        ))
        .header(header::HOST, "pbx.test")
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .map(|c| c.to_str().unwrap().split(';').next().unwrap().to_owned());
    (loc, cookie)
}

async fn me(router: &Router, cookie: &str) -> Value {
    body_json(
        raw(
            router,
            Request::get("/api/v1/auth/me")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn oidc_single_sign_on(db: PgPool) {
    let router = router(db);
    let admin = setup_admin(&router).await;
    let idp = fake_idp().await;

    let (_, info) = admin.get("/api/v1/auth/oidc").await;
    assert_eq!(info["enabled"], false);
    let (status, s) = admin
        .put(
            "/api/v1/settings/identity",
            json!({"oidc_enabled": true, "oidc_issuer": "not a url", "oidc_client_id": "talkops"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{s}");
    let (status, s) = admin
        .put(
            "/api/v1/settings/identity",
            json!({"oidc_enabled": true, "oidc_issuer": idp.issuer, "oidc_client_id": "talkops",
                   "oidc_client_secret": "client-secret", "oidc_button_label": "Firmen-Login",
                   "admin_group": "pbx-admins", "user_group": "staff"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["oidc_has_secret"], true);
    assert!(s.get("oidc_client_secret").is_none());
    let (_, info) = admin.get("/api/v1/auth/oidc").await;
    assert_eq!(info["label"], "Firmen-Login");

    // First login creates the user; the role comes from the groups.
    let (loc, cookie) = sso(
        &router,
        &idp,
        json!({"sub": "u-1", "preferred_username": "anna", "name": "Anna Beispiel",
               "email": "anna@example.com", "groups": ["staff", "pbx-admins"]}),
    )
    .await;
    assert_eq!(loc, "/");
    let m = me(&router, &cookie.unwrap()).await;
    assert_eq!(m["user"]["username"], "anna");
    assert_eq!(m["user"]["role"], "admin");
    assert_eq!(m["user"]["auth_source"], "oidc");
    // Next login: group removed → role follows the directory.
    let (_, cookie) = sso(
        &router,
        &idp,
        json!({"sub": "u-1", "preferred_username": "anna", "name": "Anna B.", "groups": ["staff"]}),
    )
    .await;
    let m = me(&router, &cookie.unwrap()).await;
    assert_eq!(m["user"]["role"], "user");
    assert_eq!(m["user"]["display_name"], "Anna B.");
    let (_, users) = admin.get("/api/v1/users").await;
    assert_eq!(
        users.as_array().unwrap().len(),
        2,
        "matched by sub, not duplicated"
    );

    // Not in an allowed group.
    let (loc, cookie) = sso(
        &router,
        &idp,
        json!({"sub": "u-2", "preferred_username": "eve", "groups": ["guests"]}),
    )
    .await;
    assert_eq!(loc, "/login?sso_error=forbidden");
    assert!(cookie.is_none());
    // A wrong nonce is rejected.
    let (loc, _) = sso(
        &router,
        &idp,
        json!({"sub": "u-1", "nonce": "other", "groups": ["staff"]}),
    )
    .await;
    assert_eq!(loc, "/login?sso_error=provider");
    // A local account is never taken over by an SSO login with its name.
    let (loc, _) = sso(
        &router,
        &idp,
        json!({"sub": "u-3", "preferred_username": "admin", "groups": ["staff"]}),
    )
    .await;
    assert_eq!(loc, "/login?sso_error=conflict");
    // Unknown or reused state.
    let res = raw(
        &router,
        Request::get("/api/v1/auth/oidc/callback?code=good-code&state=nope")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.headers()[header::LOCATION], "/login?sso_error=expired");
    // SSO accounts cannot log in with a password or set up TOTP.
    let anna = login(&router, "anna", "").await;
    assert!(anna.is_none());
    let anna_id = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "anna")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = admin
        .post(
            &format!("/api/v1/users/{anna_id}/password"),
            json!({"password": "local-password-1"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
