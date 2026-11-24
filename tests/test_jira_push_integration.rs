use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use serde_json::json;
use tower::ServiceExt;

async fn spawn_mock_jira(
    status_code: StatusCode,
    body_json: serde_json::Value,
) -> (String, tokio::sync::oneshot::Sender<()>) {
    let default_key = body_json
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or("PROJ-789")
        .to_string();
    let body_str = serde_json::to_string(&body_json).unwrap();
    let app = axum::Router::new()
        .route(
            "/rest/api/2/issue",
            axum::routing::post(move || {
                let s = body_str.clone();
                async move {
                    (
                        status_code,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        s,
                    )
                }
            }),
        )
        .route(
            "/rest/api/2/issue/:key",
            axum::routing::get(move |axum::extract::Path(k): axum::extract::Path<String>| {
                let resp_key = if k.is_empty() { default_key.clone() } else { k };
                async move {
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        serde_json::to_string(&json!({
                            "id": "10055",
                            "key": resp_key,
                            "fields": {
                                "summary": "Mock Issue",
                                "issuetype": {"name": "Bug"},
                                "status": {"name": "To Do"}
                            }
                        }))
                        .unwrap(),
                    )
                }
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
async fn test_jira_push_success() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db
        .create_project("Core Project", 10, 10, "Desc", true)
        .unwrap();
    let dev_id = db
        .create_user("dev1", "Developer 1", "d1@x.com", "pass", 55, true, false)
        .unwrap();
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
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("issue_type=Bug&summary=Custom+Summary"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 1. Verify link in DB
    let link = db
        .get_jira_link_by_bug_id(bug_id)
        .unwrap()
        .expect("Jira link must exist");
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
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user("dev2", "Dev 2", "d2@x.com", "pass", 55, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Issue", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Already linked
    db.create_jira_link(
        bug_id,
        "ALREADY-1",
        "http://jira/browse/ALREADY-1",
        "Bug",
        "",
        dev_id,
    )
    .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev2", "pass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
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
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let rep_id = db
        .create_user(
            "reporter1",
            "Reporter 1",
            "r1@x.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
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
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
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
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user("dev3", "Dev 3", "d3@x.com", "pass", 55, true, false)
        .unwrap();
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
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
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

#[tokio::test]
async fn test_jira_push_effective_access_level() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    // Private project
    let priv_proj_id = db
        .create_project("Private Proj", 10, 50, "Desc", true)
        .unwrap();

    // Alice has global Reporter (25), but is Developer (55) in this project
    let alice_id = db
        .create_user("alice", "Alice", "alice@x.com", "pass", 25, true, false)
        .unwrap();
    db.set_project_user(priv_proj_id, alice_id, 55).unwrap();

    // Bob has global Developer (55), but no access to this private project
    let _bob_id = db
        .create_user("bob", "Bob", "bob@x.com", "pass", 55, true, false)
        .unwrap();

    let bug_id = db
        .create_bug(
            priv_proj_id,
            alice_id,
            1,
            "Issue in private proj",
            "Desc",
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
            "id": "10099",
            "key": "PRIV-1",
            "self": "http://jira/rest/api/2/issue/10099"
        }),
    )
    .await;
    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "PRIV")
        .unwrap();

    let app = build_router(db.clone());

    // 1. Bob (global dev, but no access on private project) MUST be forbidden (403)
    let bob_cookie = login(&app, "bob", "pass").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Alice (global reporter, but project developer) CAN push
    let alice_cookie = login(&app, "alice", "pass").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &alice_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_some());
}

#[tokio::test]
async fn test_jira_push_handles_multiline_error_without_panic() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user("dev4", "Dev 4", "d4@x.com", "pass", 55, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Issue", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Mock returns multi-line HTML error with special characters and newlines
    let multiline_html = "<html>\r\n<body>\n<h1>502 Bad Gateway</h1>\n<p>Jira backend is down & unreachable: [error code #999]</p>\r\n</body>\n</html>";
    let app_mock = axum::Router::new().route(
        "/rest/api/2/issue",
        axum::routing::post(move || async move {
            (
                StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "text/html")],
                multiline_html,
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app_mock).await.unwrap();
    });
    let mock_url = format!("http://{}", addr);

    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "KEY")
        .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev4", "pass").await;

    // Must return 303 Redirect without panicking on newline in header
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_push", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("err="));
}

#[tokio::test]
async fn test_jira_link_cascade_on_bug_delete() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user("dev5", "Dev 5", "d5@x.com", "pass", 55, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Issue to delete",
            "Desc",
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

    db.create_jira_link(
        bug_id,
        "DEL-1",
        "http://jira/browse/DEL-1",
        "Bug",
        "",
        dev_id,
    )
    .unwrap();
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_some());

    // Delete bug
    db.delete_bug(bug_id).unwrap();

    // Link must be cleanly deleted by cascade foreign key
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());
}

async fn spawn_full_mock_jira() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = axum::Router::new()
        .route(
            "/rest/api/2/issue",
            axum::routing::post(
                |axum::extract::Json(body): axum::extract::Json<serde_json::Value>| async move {
                    let parent_key = body["fields"]["parent"]["key"].as_str().unwrap_or("");
                    let res = json!({
                        "id": "10099",
                        "key": if parent_key == "JBC-10" { "JBC-105" } else { "JBC-99" },
                        "self": "http://jira.local/rest/api/2/issue/10099"
                    });
                    (
                        StatusCode::CREATED,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        serde_json::to_string(&res).unwrap(),
                    )
                },
            ),
        )
        .route(
            "/rest/api/2/issue/:key",
            axum::routing::get(
                |axum::extract::Path(key): axum::extract::Path<String>| async move {
                    let status_name = if key == "JBC-88" {
                        "Done"
                    } else if key == "JBC-105" {
                        "To Do"
                    } else {
                        "In Progress"
                    };
                    let res = json!({
                        "id": "10099",
                        "key": key,
                        "fields": {
                            "summary": "Mock Issue Summary",
                            "issuetype": { "name": "Task" },
                            "status": { "name": status_name }
                        }
                    });
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        serde_json::to_string(&res).unwrap(),
                    )
                },
            ),
        )
        .route(
            "/rest/api/3/search/jql",
            axum::routing::get(|| async {
                let res = json!({
                    "issues": [
                        {
                            "id": "10",
                            "key": "JBC-10",
                            "fields": { "summary": "Core Epic" }
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

#[tokio::test]
async fn test_jira_sync_existing_ticket_by_key_updates_mantis_status() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user(
            "dev_sync1",
            "Dev Sync 1",
            "ds1@x.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Sync Bug", "Desc", "", "", 10, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    let (mock_url, _shutdown) = spawn_full_mock_jira().await;
    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "JBC")
        .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev_sync1", "pass").await;

    // Link by typing just "99" (normalized to JBC-99)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_sync", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("jira_key=99"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify DB link and status update
    let link = db
        .get_jira_link_by_bug_id(bug_id)
        .unwrap()
        .expect("Link should exist");
    assert_eq!(link.jira_key, "JBC-99");
    assert_eq!(link.jira_status, "In Progress");
    assert_eq!(link.issue_type, "Task");

    // Bug status should now be 50 (assigned) due to "In Progress"
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 50);

    // Bugnote was created
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert!(notes
        .iter()
        .any(|n| n.note.contains("Linked to Jira issue [JBC-99]")));
}

#[tokio::test]
async fn test_jira_sync_refresh_existing_link_and_unlink() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user(
            "dev_sync2",
            "Dev Sync 2",
            "ds2@x.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Sync Bug 2",
            "Desc",
            "",
            "",
            50,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let (mock_url, _shutdown) = spawn_full_mock_jira().await;
    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "JBC")
        .unwrap();

    // Create existing link pointing to JBC-88
    db.create_jira_link(
        bug_id,
        "JBC-88",
        &format!("{}/browse/JBC-88", mock_url),
        "Task",
        "In Progress",
        dev_id,
    )
    .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev_sync2", "pass").await;

    // Refresh sync
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_sync", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let link = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(link.jira_status, "Done");

    // Bug status updated to 80 (resolved) due to "Done"
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);

    // Now test Unlink
    let unlink_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_unlink", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unlink_res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());

    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert!(notes
        .iter()
        .any(|n| n.note.contains("Unlinked Jira issue JBC-88")));
}

#[tokio::test]
async fn test_jira_sync_create_new_ticket_with_epic() {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user(
            "dev_sync3",
            "Dev Sync 3",
            "ds3@x.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Create with Epic",
            "Desc",
            "",
            "",
            10,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let (mock_url, _shutdown) = spawn_full_mock_jira().await;
    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "JBC")
        .unwrap();

    let app = build_router(db.clone());
    let cookie = login(&app, "dev_sync3", "pass").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_sync", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("epic_key=JBC-10&issue_type=Story"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let link = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(link.jira_key, "JBC-105");
    assert_eq!(link.issue_type, "Story");
    assert_eq!(link.jira_status, "To Do");

    // Bug status updated to 30 (acknowledged) due to "To Do"
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 30);
}

/// Mock that records the reporter accountId sent on issue creation.
async fn spawn_reporter_mock_jira(
    known_email: &'static str,
) -> (
    String,
    std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>,
) {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let app = axum::Router::new()
        .route(
            "/rest/api/3/user/search",
            axum::routing::get(
                move |axum::extract::Query(q): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let body = if q.get("query").map(String::as_str) == Some(known_email) {
                        json!([{"accountId": "acc-dev", "emailAddress": known_email, "active": true}])
                    } else {
                        json!([])
                    };
                    axum::Json(body)
                },
            ),
        )
        .route(
            "/rest/api/2/issue",
            axum::routing::post(
                move |axum::extract::Json(b): axum::extract::Json<serde_json::Value>| {
                    let s2 = s2.clone();
                    async move {
                        s2.lock().unwrap().push(
                            b["fields"]["reporter"]["accountId"]
                                .as_str()
                                .map(String::from),
                        );
                        (
                            StatusCode::CREATED,
                            axum::Json(json!({"id": "1", "key": "JBC-1", "self": "http://x/1"})),
                        )
                    }
                },
            ),
        )
        .route(
            "/rest/api/2/issue/:key",
            axum::routing::get(|axum::extract::Path(k): axum::extract::Path<String>| async move {
                axum::Json(json!({
                    "id": "1", "key": k,
                    "fields": {"summary": "S", "issuetype": {"name": "Bug"}, "status": {"name": "To Do"}}
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{}", addr), seen)
}

async fn sync_as_user(user_email: &str, jira_email: &'static str) -> Vec<Option<String>> {
    let db = Db::open_in_memory().unwrap();
    db.set_allow_local_jira(true);
    let proj_id = db.create_project("Project", 10, 10, "", true).unwrap();
    let dev_id = db
        .create_user("dev_rep", "Dev Rep", user_email, "pass", 55, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Reporter test",
            "Desc",
            "",
            "",
            10,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let (mock_url, seen) = spawn_reporter_mock_jira(jira_email).await;
    db.save_jira_global_config(true, &mock_url, "bot@x.com", Some("tok"), "Bug", "JBC")
        .unwrap();
    let app = build_router(db.clone());
    let cookie = login(&app, "dev_rep", "pass").await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/view_bug_page/{}/jira_sync", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("issue_type=Bug"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_some());
    let v = seen.lock().unwrap().clone();
    v
}

#[tokio::test]
async fn test_jira_sync_create_uses_user_email_as_reporter() {
    let seen = sync_as_user("dev@x.com", "dev@x.com").await;
    assert_eq!(seen, vec![Some("acc-dev".to_string())]);
}

#[tokio::test]
async fn test_jira_sync_create_falls_back_when_user_not_on_jira() {
    let seen = sync_as_user("ghost@x.com", "dev@x.com").await;
    assert_eq!(seen, vec![None]);
}
