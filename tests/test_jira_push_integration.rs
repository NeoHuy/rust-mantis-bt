use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use serde_json::json;
use tower::ServiceExt;

async fn spawn_mock_jira(status_code: StatusCode, body_json: serde_json::Value) -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = axum::Router::new().route(
        "/rest/api/2/issue",
        axum::routing::post(move || async move {
            (
                status_code,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::to_string(&body_json).unwrap(),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    (format!("http://{}", addr), tx)
}

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
async fn test_jira_push_success() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("Core Project", 10, 10, "Desc", true).unwrap();
    let dev_id = db.create_user("dev1", "Developer 1", "d1@x.com", "pass", 55, true, false).unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Broken button",
            "Button does not click",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let (mock_url, _shutdown) = spawn_mock_jira(
        StatusCode::CREATED,
        json!({
            "id": "10055",
            "key": "PROJ-789",
            "self": "http://jira.local/rest/api/2/issue/10055"
        }),
    )
    .await;

    // Configure Jira
    db.save_jira_global_config(
        true,
        &mock_url,
        "jira-bot@example.com",
        Some("dummy-api-token"),
        "Bug",
        "PROJ",
    )
    .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev1", "pass").await;

    // Push bug to Jira
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("issue_type=Bug&summary=Custom+Summary"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 1. Verify link in DB
    let link = db.get_jira_link_by_bug_id(bug_id).unwrap().expect("Jira link must exist");
    assert_eq!(link.jira_key, "PROJ-789");
    assert_eq!(link.jira_url, format!("{}/browse/PROJ-789", mock_url));
    assert_eq!(link.created_by, dev_id);

    // 2. Verify bug status updated to acknowledged (30)
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 30);

    // 3. Verify bugnote created
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].note.contains("PROJ-789"));
}

#[tokio::test]
async fn test_jira_push_already_linked() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db.create_user("dev2", "Dev 2", "d2@x.com", "pass", 55, true, false).unwrap();
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Issue", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Already linked
    db.create_jira_link(bug_id, "ALREADY-1", "http://jira/browse/ALREADY-1", "Bug", dev_id).unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev2", "pass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Must redirect with error message
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("err="));
}

#[tokio::test]
async fn test_jira_push_permission_denied() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let rep_id = db.create_user("reporter1", "Reporter 1", "r1@x.com", "pass", 25, true, false).unwrap();
    let bug_id = db
        .create_bug(
            proj_id, rep_id, 1, "Issue", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "reporter1", "pass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_jira_push_jira_error_keeps_bug_state() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db.create_user("dev3", "Dev 3", "d3@x.com", "pass", 55, true, false).unwrap();
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Issue", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Mock returns 400 Bad Request
    let (mock_url, _shutdown) = spawn_mock_jira(
        StatusCode::BAD_REQUEST,
        json!({
            "errorMessages": ["Project key invalid"]
        }),
    )
    .await;

    db.save_jira_global_config(
        true,
        &mock_url,
        "jira-bot@example.com",
        Some("dummy-api-token"),
        "Bug",
        "INVALID_KEY",
    )
    .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev3", "pass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("issue_type=Bug"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("err="));

    // Bug status MUST still be new (10)
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 10);

    // No bugnotes added
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert_eq!(notes.len(), 0);

    // No link added
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());
}
