use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use serde_json::Value;
use tower::ServiceExt;

async fn login_as_admin(app: &axum::Router) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=root"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    res.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_rest_api_tokens_and_plugins_end_to_end() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("RestProject", 10, 10, "API Project", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_as_admin(&app).await;

    // 1. Unauthenticated REST request returns 401
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rest/users/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Create API token via Web UI (/api_token_create)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=CLI+Integration"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    let raw_token = loc.split("token=").nth(1).unwrap().to_string();
    assert_eq!(raw_token.len(), 64);

    // 3. Call GET /api/rest/users/me with Authorization header
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rest/users/me")
                .header("authorization", &raw_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json_me: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json_me["name"], "administrator");
    assert_eq!(json_me["access_level"]["id"], 90);

    // 4. Call GET /api/rest/projects with Bearer token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rest/projects")
                .header("authorization", format!("Bearer {}", raw_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json_projs: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json_projs["projects"][0]["name"], "RestProject");

    // 5. Call POST /api/rest/issues to create an issue via JSON
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rest/issues")
                .header("authorization", &raw_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "summary": "REST API created bug",
                        "description": "Created via JSON payload",
                        "project": { "id": proj_id },
                        "category": { "id": 1 },
                        "priority": { "id": 40 },
                        "severity": { "id": 60 },
                        "target_version": "v3.0"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let bug_id = created["issue"]["id"].as_i64().unwrap();
    assert_eq!(created["issue"]["summary"], "REST API created bug");
    assert_eq!(created["issue"]["priority"]["name"], "high");

    // 6. Call POST /api/rest/issues/:id/notes to add a note with time tracking
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/rest/issues/{}/notes", bug_id))
                .header("authorization", &raw_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "text": "Automated note from CI",
                        "view_state": { "id": 10 },
                        "time_tracking": 45
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 7. Call PATCH /api/rest/issues/:id to resolve the issue
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/rest/issues/{}", bug_id))
                .header("authorization", &raw_token)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "summary": "REST API resolved bug",
                        "status": { "id": 80 },
                        "resolution": { "id": 20 },
                        "fixed_in_version": "v3.0"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let patched: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(patched["issues"][0]["summary"], "REST API resolved bug");
    assert_eq!(patched["issues"][0]["status"]["id"], 80);
    assert_eq!(patched["issues"][0]["notes"][0]["time_tracking"], 45);

    // 8. Check Plugin Management UI (/manage_plugin_page) & toggle EventAuditLog
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_plugin_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("MantisCoreFormatting"));
    assert!(html.contains("EventAuditLog"));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_plugin_toggle/EventAuditLog")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("enabled=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(!db.is_plugin_enabled("EventAuditLog"));

    // 9. Delete issue via DELETE /api/rest/issues/:id and revoke token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/rest/issues/{}", bug_id))
                .header("authorization", &raw_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(db.get_bug(bug_id).unwrap().is_none());

    let tok_id = db.list_api_tokens(1).unwrap()[0].id;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api_token_revoke/{}", tok_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Revoked token returns 401
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rest/users/me")
                .header("authorization", &raw_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
