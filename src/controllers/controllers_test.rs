use super::*;

#[test]
fn test_parse_bool() {
    assert!(parse_bool(Some("1")));
    assert!(parse_bool(Some("true")));
    assert!(parse_bool(Some("TRUE")));
    assert!(parse_bool(Some("  True  ")));
    assert!(parse_bool(Some("yes")));
    assert!(parse_bool(Some("YES")));
    assert!(parse_bool(Some("on")));
    assert!(parse_bool(Some("ON")));

    assert!(!parse_bool(Some("0")));
    assert!(!parse_bool(Some("false")));
    assert!(!parse_bool(Some("FALSE")));
    assert!(!parse_bool(Some("no")));
    assert!(!parse_bool(Some("off")));
    assert!(!parse_bool(Some("")));
    assert!(!parse_bool(Some("unknown")));
    assert!(!parse_bool(None));
}

#[tokio::test]
async fn test_security_headers_middleware() {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    for response in [
        res,
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    ] {
        let headers = response.headers();
        assert_eq!(headers.get("x-frame-options").unwrap(), "SAMEORIGIN");
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        let csp = headers
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("frame-ancestors 'self'"));
        assert!(csp.contains("form-action 'self'"));
    }
}

#[tokio::test]
async fn test_csrf_protection_middleware() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let user = db.authenticate("administrator", "root").unwrap().unwrap();
    let session_cookie = format!("MANTIS_STRING_COOKIE={}", user.cookie_string);
    let csrf_token = crate::auth::csrf_token_for_session(&user.cookie_string);
    let app = build_router(db);

    // 1. Cross-site Sec-Fetch-Site rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("sec-fetch-site", "cross-site")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. same-site Sec-Fetch-Site rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("sec-fetch-site", "same-site")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Origin mismatch rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("host", "localhost:3000")
                .header("origin", "https://evil.example.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 4. Null Origin rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("origin", "null")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 5. Referer mismatch rejected when Origin is absent (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("host", "localhost:3000")
                .header("referer", "https://evil.example.com/attack")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 6. Invalid X-CSRF-Token rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", "invalid")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 7. Valid same-origin + Origin + Host + X-CSRF-Token succeeds (303)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("sec-fetch-site", "same-origin")
                .header("host", "localhost:3000")
                .header("origin", "http://localhost:3000")
                .header("x-csrf-token", &csrf_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 8. Stateless REST API route is NOT blocked by Web UI CSRF middleware (401, not 403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rest/issues")
                .header("sec-fetch-site", "cross-site")
                .header("origin", "https://external-client.example.com")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 9. Reverse proxy with X-Forwarded-Host succeeds
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("sec-fetch-site", "same-origin")
                .header("x-forwarded-host", "mantis.mycompany.com")
                .header("origin", "https://mantis.mycompany.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 10. Missing Host header with cross-origin Origin is rejected (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("origin", "https://attacker.example.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 11. MANTIS_PUBLIC_ORIGIN scheme matching validation
    std::env::set_var("MANTIS_PUBLIC_ORIGIN", "https://mantis.prod.com");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("origin", "https://mantis.prod.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Mismatched scheme (http instead of https) is rejected (403)
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &session_cookie)
                .header("origin", "http://mantis.prod.com")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    std::env::remove_var("MANTIS_PUBLIC_ORIGIN");
}
