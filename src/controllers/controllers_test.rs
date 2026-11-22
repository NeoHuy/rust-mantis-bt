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
    }
}
