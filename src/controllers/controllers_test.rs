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

#[test]
fn test_sanitize_error_message_and_url_encode() {
    assert_eq!(sanitize_error_message("Normal error"), "Normal error");
    assert_eq!(
        sanitize_error_message("Line 1\r\nLine 2\tTabbed"),
        "Line 1  Line 2 Tabbed"
    );
    assert_eq!(
        sanitize_error_message("   padded message   "),
        "padded message"
    );
    let long_str = "a".repeat(150);
    let sanitized = sanitize_error_message(&long_str);
    assert_eq!(sanitized.chars().count(), 120);
    assert!(sanitized.ends_with("..."));

    assert_eq!(url_encode("hello world"), "hello+world");
    assert_eq!(url_encode("foo@bar=1&baz=2"), "foo%40bar%3D1%26baz%3D2");
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
        assert!(csp.contains("script-src 'self';"));
        assert!(!csp.contains("script-src 'self' 'unsafe-inline'"));
        assert!(csp.contains("frame-ancestors 'self'"));
        assert!(csp.contains("form-action 'self'"));
        assert!(csp.contains("font-src 'self'"));
    }
}

#[tokio::test]
async fn test_static_assets_endpoints() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db);

    let endpoints = [
        ("/assets/tailwind.css", "text/css; charset=utf-8"),
        ("/assets/font-awesome.min.css", "text/css; charset=utf-8"),
        ("/assets/mantis.js", "application/javascript; charset=utf-8"),
        (
            "/assets/mantis-init.js",
            "application/javascript; charset=utf-8",
        ),
        ("/assets/mantis_logo.png", "image/png"),
        ("/fonts/fontawesome-webfont.woff2", "font/woff2"),
        ("/fonts/fontawesome-webfont.woff", "font/woff"),
    ];

    for (uri, expected_ct) in endpoints {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "GET {uri} should return 200");
        let headers = res.headers();
        assert_eq!(
            headers.get("content-type").unwrap().to_str().unwrap(),
            expected_ct,
            "Unexpected content-type for {uri}"
        );
        assert_eq!(
            headers.get("cache-control").unwrap().to_str().unwrap(),
            "public, max-age=86400",
            "Missing cache-control for {uri}"
        );
        let csp = headers
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            csp.contains("font-src 'self'"),
            "CSP should include font-src 'self' on {uri}"
        );
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(!bytes.is_empty(), "Empty asset body for {uri}");
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

#[tokio::test]
async fn test_core_pages_have_zero_inline_js_and_zero_inline_event_handlers() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let proj_id = db
        .create_project("CSP Test Project", 10, 10, "desc", true)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            0,
            "Test Bug",
            "Test description",
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

    let app = build_router(db);
    let cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

    let view_bug_uri = format!("/view_bug_page/{}", bug_id);
    let pages: &[(&str, bool)] = &[
        ("/my_view_page", true),
        ("/view_all_bug_page", true),
        ("/bug_report_page", true),
        (view_bug_uri.as_str(), true),
        ("/news_page", true),
        ("/manage_proj_page", true),
        ("/manage_user_page", true),
        ("/manage_email_queue_page", true),
        ("/manage_overview_page", true),
        ("/account_page", true),
        ("/account_manage_columns_page", true),
        ("/api_tokens_page", true),
        ("/signup_page", false),
        ("/login", false),
    ];

    for &(uri, with_auth) in pages {
        let mut req_builder = Request::builder().uri(uri);
        if with_auth {
            req_builder = req_builder.header("cookie", &cookie);
        }
        let res = app
            .clone()
            .oneshot(req_builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "GET {uri} should return 200");

        let csp = res
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            csp.contains("script-src 'self';"),
            "CSP on {uri} must have script-src 'self'; got: {csp}"
        );

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&bytes);
        let lower = html.to_ascii_lowercase();

        // Every <script tag must have a src= attribute (no inline <script>)
        let mut search_from = 0;
        while let Some(idx) = lower[search_from..].find("<script") {
            let abs_idx = search_from + idx;
            let tag_end = lower[abs_idx..]
                .find('>')
                .map(|e| abs_idx + e)
                .unwrap_or(lower.len());
            let tag_slice = &lower[abs_idx..tag_end];
            assert!(
                tag_slice.contains("src="),
                "Inline <script> found on {uri}: {}",
                &html[abs_idx..tag_end.min(abs_idx + 120)]
            );
            search_from = tag_end;
        }

        // Zero inline event handlers
        for attr in ["onclick=", "onchange=", "onsubmit="] {
            assert!(
                !lower.contains(attr),
                "Inline handler '{attr}' found on {uri}"
            );
        }
    }
}
