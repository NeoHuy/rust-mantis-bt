use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}",
                    username, password
                )))
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
async fn test_token_governance_and_admin_oversight() {
    let db = Db::open_in_memory().unwrap();
    // Default admin already created (id=1, administrator/root, level=90)
    // Create Reporter user (level=25)
    let reporter_id = db
        .create_user("reporter_user", "Reporter User", "reporter@test.com", "password123", 25, true, false)
        .unwrap();
    // Create Viewer user (level=10)
    let _viewer_id = db
        .create_user("viewer_user", "Viewer User", "viewer@test.com", "password123", 10, true, false)
        .unwrap();

    let app = build_router(db.clone());

    let viewer_cookie = login(&app, "viewer_user", "password123").await;
    let reporter_cookie = login(&app, "reporter_user", "password123").await;
    let admin_cookie = login(&app, "administrator", "root").await;

    // 1. Viewer cannot view api_tokens_page (403 Forbidden)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Viewer cannot create API token (403 Forbidden)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &viewer_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ViewerToken"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Reporter can view api_tokens_page (200 OK)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &reporter_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(!body_str.contains("System-Wide API Tokens"));

    // 4. Reporter creates an API token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &reporter_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ReporterToken"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify token exists via db
    let reporter_tokens = db.list_api_tokens(reporter_id).unwrap();
    assert_eq!(reporter_tokens.len(), 1);
    let token_id = reporter_tokens[0].id;

    // 5. Admin lists all system tokens via Db
    let all_tokens = db.list_all_api_tokens().unwrap();
    assert!(all_tokens.iter().any(|(t, username, _)| t.id == token_id && username == "reporter_user"));

    // 6. Admin visits api_tokens_page and sees System-Wide API Tokens table
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("System-Wide API Tokens (Administrator View)"));
    assert!(body_str.contains("reporter_user"));

    // 7. Admin revokes Reporter's token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api_token_revoke/{}", token_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify token is gone
    let reporter_tokens = db.list_api_tokens(reporter_id).unwrap();
    assert_eq!(reporter_tokens.len(), 0);
}
