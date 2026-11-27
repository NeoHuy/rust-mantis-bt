use crate::controllers::build_router;
use crate::db::Db;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn test_manage_overview_page_requires_admin() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/manage_overview_page")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Unauthenticated redirects to /login
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn test_manage_overview_page_admin_ok() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let app = build_router(db);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/manage_overview_page")
                .header(
                    "Cookie",
                    format!("MANTIS_STRING_COOKIE={}", admin.cookie_string),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Site Information"));
    assert!(html.contains("Runtime Configuration Status"));
}
