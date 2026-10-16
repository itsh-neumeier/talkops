//! Roles and permissions: built-in defaults, own roles, the editor rules and
//! protection against more rights.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;
use talkops_core::roles::{OPERATOR_DEFAULTS, Permission};

use common::*;

async fn role_id(admin: &Client, builtin: &str) -> String {
    let (_, overview) = admin.get("/api/v1/roles").await;
    overview["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["builtin"] == builtin)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn create_user(admin: &Client, name: &str, role: Value) -> String {
    let mut body = json!({"username": name, "display_name": name, "role": "user", "password": "correct-horse"});
    match role.as_str() {
        Some(id) if id.contains('-') => body["role_id"] = json!(id),
        _ => body["role"] = role,
    }
    let (status, user) = admin.post("/api/v1/users", body).await;
    assert_eq!(status, StatusCode::OK, "{user}");
    user["id"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn migration_defaults_match_the_old_fixed_roles(pool: PgPool) {
    let router = router(pool.clone());
    let admin = setup_admin(&router).await;
    let (status, overview) = admin.get("/api/v1/roles").await;
    assert_eq!(status, StatusCode::OK);
    let roles = overview["roles"].as_array().unwrap();
    let builtins: Vec<&str> = roles
        .iter()
        .map(|r| r["builtin"].as_str().unwrap())
        .collect();
    assert_eq!(builtins, ["admin", "operator", "user"]);
    let mut operator: Vec<&str> = roles[1]["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    operator.sort();
    let mut expected: Vec<&str> = OPERATOR_DEFAULTS.iter().map(|p| p.key()).collect();
    expected.sort();
    assert_eq!(operator, expected, "the migration matches the code");
    assert!(roles[2]["permissions"].as_array().unwrap().is_empty());
    assert_eq!(roles[0]["users"], 1);
    assert_eq!(
        overview["permissions"].as_array().unwrap().len(),
        Permission::ALL.len()
    );
    // A new tenant gets the built-in roles from the trigger.
    let other: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO tenants (name) VALUES ('Other') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM roles WHERE tenant_id = $1")
        .bind(other)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn permissions_follow_the_role_and_apply_at_once(pool: PgPool) {
    let router = router(pool);
    let admin = setup_admin(&router).await;
    let user_role = role_id(&admin, "user").await;
    create_user(&admin, "bob", json!("user")).await;
    let bob = login(&router, "bob", "correct-horse").await.unwrap();
    let (_, me) = bob.get("/api/v1/auth/me").await;
    assert_eq!(me["permissions"], json!([]));
    assert_eq!(bob.get("/api/v1/extensions").await.0, StatusCode::FORBIDDEN);
    assert_eq!(bob.get("/api/v1/audit").await.0, StatusCode::FORBIDDEN);

    // The admin gives the default user role two permissions; the open
    // session gets them without a new login.
    let (status, role) = admin
        .put(
            &format!("/api/v1/roles/{user_role}"),
            json!({"name": "Default user", "permissions": ["extensions.view", "audit.view"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{role}");
    assert_eq!(bob.get("/api/v1/extensions").await.0, StatusCode::OK);
    assert_eq!(bob.get("/api/v1/audit").await.0, StatusCode::OK);
    assert_eq!(
        bob.call(
            "DELETE",
            &format!("/api/v1/extensions/{}", uuid::Uuid::new_v4()),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "view is not manage"
    );
    let (_, me) = bob.get("/api/v1/auth/me").await;
    assert_eq!(me["permissions"], json!(["extensions.view", "audit.view"]));

    // Admins have every key; the admin role cannot be changed.
    let (_, me) = admin.get("/api/v1/auth/me").await;
    assert_eq!(
        me["permissions"].as_array().unwrap().len(),
        Permission::ALL.len()
    );
    let admin_role = role_id(&admin, "admin").await;
    let (status, _) = admin
        .put(
            &format!("/api/v1/roles/{admin_role}"),
            json!({"name": "Admin", "permissions": []}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Only admins change roles.
    assert_eq!(
        bob.post("/api/v1/roles", json!({"name": "x", "permissions": []}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // Unknown permission keys are rejected.
    let (status, _) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Bad", "permissions": ["root.everything"]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn own_roles_are_assigned_and_cannot_be_deleted_in_use(pool: PgPool) {
    let router = router(pool);
    let admin = setup_admin(&router).await;
    let (status, reception) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Reception", "description": "Front desk",
                   "permissions": ["calls.active", "calls.view_all", "phonebook.edit"],
                   "directory_group": "pbx-reception"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reception}");
    let reception_id = reception["id"].as_str().unwrap();
    let anna = create_user(&admin, "anna", json!(reception_id)).await;
    let (_, user) = admin.get(&format!("/api/v1/users/{anna}")).await;
    assert_eq!(user["role"], "user");
    assert_eq!(user["role_id"], reception_id);
    let anna_client = login(&router, "anna", "correct-horse").await.unwrap();
    assert_eq!(
        anna_client.get("/api/v1/telephony/calls").await.0,
        StatusCode::OK
    );
    assert_eq!(
        anna_client.get("/api/v1/users").await.0,
        StatusCode::FORBIDDEN
    );

    // Names are unique.
    let (status, _) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "reception", "permissions": []}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // A role with users stays; without users it can be deleted.
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/roles/{reception_id}"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let operator = role_id(&admin, "operator").await;
    let (status, user) = admin
        .put(
            &format!("/api/v1/users/{anna}"),
            json!({"display_name": "anna", "role": "user", "role_id": operator, "enabled": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    assert_eq!(user["role"], "operator");
    assert_eq!(
        user["role_id"],
        Value::Null,
        "built-in roles live in `role`"
    );
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/roles/{reception_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Built-in roles cannot be deleted.
    let (status, _) = admin
        .call("DELETE", &format!("/api/v1/roles/{operator}"), None)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "talkops_core::db::MIGRATOR")]
async fn user_managers_cannot_give_or_take_more_rights(pool: PgPool) {
    let router = router(pool);
    let admin = setup_admin(&router).await;
    let (_, managers) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "User managers", "permissions": ["users.view", "users.manage", "calls.active"]}),
        )
        .await;
    let (_, small) = admin
        .post(
            "/api/v1/roles",
            json!({"name": "Small", "permissions": ["calls.active"]}),
        )
        .await;
    create_user(&admin, "mia", managers["id"].clone()).await;
    let olga = create_user(&admin, "olga", json!("operator")).await;
    let mia = login(&router, "mia", "correct-horse").await.unwrap();
    let new = |role: Value| {
        let mut body = json!({"username": "eve", "display_name": "Eve", "role": "user", "password": "correct-horse"});
        match role.as_str() {
            Some(id) if id.contains('-') => body["role_id"] = role,
            _ => body["role"] = role,
        }
        body
    };
    // Admin role and roles with other permissions: no.
    assert_eq!(
        mia.post("/api/v1/users", new(json!("admin"))).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        mia.post("/api/v1/users", new(json!("operator"))).await.0,
        StatusCode::FORBIDDEN
    );
    // A role inside the own permissions: yes.
    let (status, eve) = mia.post("/api/v1/users", new(small["id"].clone())).await;
    assert_eq!(status, StatusCode::OK, "{eve}");
    // Users with more rights stay out of reach.
    for (method, path, body) in [
        (
            "PUT",
            format!("/api/v1/users/{olga}"),
            Some(json!({"display_name": "x", "role": "user", "enabled": true})),
        ),
        (
            "POST",
            format!("/api/v1/users/{olga}/password"),
            Some(json!({"password": "another-horse"})),
        ),
        ("DELETE", format!("/api/v1/users/{olga}"), None),
    ] {
        assert_eq!(
            mia.call(method, &path, body).await.0,
            StatusCode::FORBIDDEN,
            "{method} {path}"
        );
    }
    let (_, me) = admin.get("/api/v1/auth/me").await;
    let admin_id = me["user"]["id"].as_str().unwrap();
    assert_eq!(
        mia.post(
            &format!("/api/v1/users/{admin_id}/password"),
            json!({"password": "another-horse"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}
