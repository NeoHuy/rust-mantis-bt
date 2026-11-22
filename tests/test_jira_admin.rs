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
async fn test_admin_jira_access_denied_for_developer() {
    let db = Db::open_in_memory().unwrap();
    let _ = db
        .create_user(
            "devuser",
            "Developer",
            "dev@example.com",
            "devpass",
            55,
            true,
            false,
        )
        .unwrap();
    let app = build_router(db.clone());

    let cookie = login(&app, "devuser", "devpass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_jira_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_jira_save_config() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("Alpha Project", 10, 10, "Description", true)
        .unwrap();
    let app = build_router(db.clone());

    let cookie = login(&app, "administrator", "root").await;

    // View manage page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_jira_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Save config
    let form_body = format!(
        "jira_enabled=1&jira_base_url=https://myteam.atlassian.net&jira_email=bot@myteam.com&jira_api_token=secret_tok_123&jira_default_issue_type=Task&jira_default_project_key=GLOBAL_KEY&project_map_{}=ALPHA&status_jira_1=In+Testing&status_mantis_1=20&status_jira_2=Deployed&status_mantis_2=80",
        proj_id
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_jira_config")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify DB
    let conf = db.get_jira_config(proj_id).unwrap();
    assert!(conf.enabled);
    assert_eq!(conf.base_url, "https://myteam.atlassian.net");
    assert_eq!(conf.email, "bot@myteam.com");
    assert_eq!(conf.api_token, "secret_tok_123");
    assert_eq!(conf.default_issue_type, "Task");
    assert_eq!(conf.project_key, "ALPHA");

    let sm = db.list_jira_status_mappings().unwrap();
    assert_eq!(sm.len(), 2);
    assert_eq!(sm[0], ("In Testing".to_string(), 20));
    assert_eq!(sm[1], ("Deployed".to_string(), 80));
}

#[tokio::test]
async fn test_admin_jira_refresh_epics_permissions_and_success() {
    let app_jira = axum::Router::new().route(
        "/rest/api/3/search/jql",
        axum::routing::get(|| async {
            let res = serde_json::json!({
                "issues": [
                    {
                        "id": "10",
                        "key": "ALPHA-1",
                        "fields": { "summary": "Alpha Epic One" }
                    },
                    {
                        "id": "11",
                        "key": "ALPHA-2",
                        "fields": { "summary": "Alpha Epic Two" }
                    }
                ]
            });
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::to_string(&res).unwrap(),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mock_url = format!("http://{}", addr);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app_jira)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });

    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    db.save_jira_global_config(
        true,
        &mock_url,
        "bot@example.com",
        Some("secret_token"),
        "Bug",
        "ALPHA",
    )
    .unwrap();

    let _ = db
        .create_user(
            "devuser",
            "Developer",
            "dev@example.com",
            "devpass",
            55,
            true,
            false,
        )
        .unwrap();

    let app = build_router(db.clone());

    // 1. Developer should be denied
    let dev_cookie = login(&app, "devuser", "devpass").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_jira_refresh_epics")
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Administrator should succeed
    let admin_cookie = login(&app, "administrator", "root").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_jira_refresh_epics")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify cache in DB
    let cached = db.get_cached_jira_epics("ALPHA", 86400).unwrap().unwrap();
    assert_eq!(cached.len(), 2);
    assert_eq!(cached[0].key, "ALPHA-1");
    assert_eq!(cached[0].summary, "Alpha Epic One");
    assert_eq!(cached[1].key, "ALPHA-2");

    let _ = tx.send(());
}

#[tokio::test]
async fn test_admin_jira_rejects_ssrf_url_by_default() {
    use axum::body::to_bytes;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let admin_cookie = login(&app, "administrator", "root").await;

    for blocked_url in [
        "http://127.0.0.1:8080",
        "https://169.254.169.254",
        "https://metadata.google.internal",
        "https://localhost:8443",
    ] {
        let form = format!(
            "jira_enabled=1&jira_base_url={}&jira_email=bot@example.com&jira_api_token=tok&jira_default_issue_type=Bug&jira_default_project_key=PROJ",
            blocked_url
        );
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/manage_jira_config")
                    .header("cookie", &admin_cookie)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let loc = res.headers().get("location").unwrap().to_str().unwrap();
        assert!(
            loc.contains("err=Failed+to+save+config"),
            "Expected SSRF rejection for {}, got {}",
            blocked_url,
            loc
        );
        assert_eq!(db.get_jira_config(0).unwrap().base_url, "");
    }

    // Simulate legacy on-prem/internal URL saved prior to SSRF restriction
    db.save_jira_global_config(
        true,
        "http://127.0.0.1:8080",
        "bot@example.com",
        Some("tok"),
        "Bug",
        "PROJ",
    )
    .unwrap();

    // Admin page should display clear SSRF warning & MANTIS_ALLOW_LOCAL_JIRA=1 remediation
    let page_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_jira_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page_res.status(), StatusCode::OK);
    let page_body = String::from_utf8(
        to_bytes(page_res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(page_body.contains("blocked by SSRF policy"));
    assert!(page_body.contains("MANTIS_ALLOW_LOCAL_JIRA=1"));

    // Connection test using legacy blocked URL must be rejected by default
    let test_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_jira_test")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(test_res.status(), StatusCode::SEE_OTHER);
    let test_loc = test_res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        test_loc.contains("Connection+failed"),
        "Expected Connection+failed redirect, got {}",
        test_loc
    );

    // HTTPS localhost/loopback saved directly is also rejected at connection test time
    db.save_jira_global_config(
        true,
        "https://127.0.0.1:8080",
        "bot@example.com",
        Some("tok"),
        "Bug",
        "PROJ",
    )
    .unwrap();
    let test_res_https = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_jira_test")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(test_res_https.status(), StatusCode::SEE_OTHER);
    let test_loc_https = test_res_https
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        test_loc_https.contains("Connection+failed"),
        "Expected Connection+failed redirect for https://127.0.0.1:8080, got {}",
        test_loc_https
    );
}
