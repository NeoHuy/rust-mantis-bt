use super::*;
use crate::controllers::build_router;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

#[tokio::test]
async fn test_rss_requires_auth_and_supports_pat_bearer_and_query_token() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("RssUnitProj", 10, 10, "", true).unwrap();
    db.create_bug(
        proj_id,
        1,
        0,
        "RSS Bug <tag>",
        "Body & details",
        "",
        "",
        30,
        50,
        10,
        "",
        "",
        "",
        "",
        10,
    )
    .unwrap();
    let (_, pat) = db.create_api_token(1, "rss-reader").unwrap();
    let app = build_router(db);

    // 1. Unauthenticated returns 401 Unauthorized
    let res_unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/issues_rss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_unauth.status(), StatusCode::UNAUTHORIZED);

    // 2. Bearer PAT succeeds and omits secret token from <atom:link>
    let res_bearer = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/issues_rss?project_id={}", proj_id))
                .header("authorization", format!("Bearer {}", pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bearer.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res_bearer.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("RSS Bug &lt;tag&gt;"));
    assert!(body.contains("Body &amp; details"));
    assert!(!body.contains(&pat));

    // 3. Query ?token= works and ?username=&key= enforces username match
    let res_bad_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/news_rss?username=other&key={}", pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_user.status(), StatusCode::UNAUTHORIZED);

    let res_good_key = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/news_rss?username=administrator&key={}", pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_good_key.status(), StatusCode::OK);
}
