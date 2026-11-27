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
    let (_, api_pat) = db.create_api_token(1, "full-api").unwrap();
    let (_, rss_pat) = db
        .create_api_token_with_scope(1, "rss-reader", "rss")
        .unwrap();
    let app = build_router(db.clone());

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
                .header("authorization", format!("Bearer {}", api_pat))
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
    assert!(!body.contains(&api_pat));

    // 3. Full API token is rejected in query string by default; RSS-scoped token works in query string
    let res_api_query = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/issues_rss?token={}", api_pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_api_query.status(), StatusCode::UNAUTHORIZED);

    // 4. Read-only RSS token is rejected on /api/rest/*
    let res_rss_rest = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/rest/issues")
                .header("authorization", format!("Bearer {}", rss_pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rss_rest.status(), StatusCode::UNAUTHORIZED);

    // 5. Query ?username=&key= with rss token enforces uniform 401 on username mismatch and accepts match
    let res_bad_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/news_rss?username=other&key={}", rss_pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_user.status(), StatusCode::UNAUTHORIZED);
    let bad_user_body = String::from_utf8(
        axum::body::to_bytes(res_bad_user.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(bad_user_body, "Invalid or revoked RSS feed token");

    let res_good_key = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/news_rss?username=administrator&key={}&project_id={}&project_id=0",
                    rss_pat, 999999
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_good_key.status(), StatusCode::OK);

    // 6. Non-Bearer Authorization (e.g. reverse proxy Basic) falls through to session cookie auth
    let login_res = app
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
    let adm_cookie = login_res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let res_basic_fallback = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/issues_rss")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .header("cookie", &adm_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_basic_fallback.status(), StatusCode::OK);

    // 7. Token creation normalizes RSS/API scopes cleanly and rejects unknown scopes instead of downgrading to full api
    let res_bad_scope = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &adm_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=bad_scope_tok&scope=read"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_scope.status(), StatusCode::SEE_OTHER);
    let bad_loc = res_bad_scope
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(bad_loc.contains("err=Invalid"));
    assert!(db
        .list_api_tokens(1)
        .unwrap()
        .iter()
        .all(|t| t.name != "bad_scope_tok"));

    let res_upper_rss = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &adm_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=upper_rss_tok&scope=RSS"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_upper_rss.status(), StatusCode::SEE_OTHER);
    let created_rss = db
        .list_api_tokens(1)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "upper_rss_tok")
        .expect("RSS token should be created");
    assert_eq!(created_rss.scope, "rss");

    // 8. Uppercase BEARER scheme is accepted on /issues_rss
    let res_upper_bearer = build_router(db)
        .oneshot(
            Request::builder()
                .uri("/issues_rss")
                .header("authorization", format!("BEARER {}", api_pat))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_upper_bearer.status(), StatusCode::OK);
}
