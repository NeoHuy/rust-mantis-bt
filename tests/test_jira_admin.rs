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
                .body(Body::from(format!("username={}&password={}", username, password)))
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
    let _ = db.create_user("devuser", "Developer", "dev@example.com", "devpass", 55, true, false).unwrap();
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
    let proj_id = db.create_project("Alpha Project", 10, 10, "Description", true).unwrap();
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
        "jira_enabled=1&jira_base_url=https://myteam.atlassian.net&jira_email=bot@myteam.com&jira_api_token=secret_tok_123&jira_default_issue_type=Task&jira_default_project_key=GLOBAL_KEY&project_map_{}=ALPHA",
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
}
