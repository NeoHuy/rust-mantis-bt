use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db, models::BugUpdate};
use tower::ServiceExt;

async fn login_as_admin(app: &axum::Router) -> String {
    let res = app
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

async fn set_project_cookie(app: &axum::Router, auth_cookie: &str, proj_id: i64) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", auth_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}", proj_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let proj_cookie = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    format!("{}; {}", auth_cookie, proj_cookie)
}

#[tokio::test]
async fn test_roadmap_changelog_summary_billing_news_and_email_queue() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let proj_id = db.create_project("Engine", 10, 10, "", true).unwrap();

    // Create two issues with target_version and fixed_in_version
    let b1 = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Implement SQLite WAL mode",
            "Use WAL for concurrency",
            "",
            "",
            40,
            60,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b1,
        admin.id,
        BugUpdate {
            target_version: Some("v2.0".to_string()),
            fixed_in_version: Some("v2.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    // Resolve b1 (status = 80, resolution = 20 Fixed)
    db.change_bug_status(b1, admin.id, 80, 20, Some("Completed WAL support"))
        .unwrap();

    let b2 = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Add HTTP compression",
            "Pending work",
            "",
            "",
            30,
            50,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b2,
        admin.id,
        BugUpdate {
            target_version: Some("v2.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let app = build_router(db.clone());
    let auth_cookie = login_as_admin(&app).await;
    let cookie = set_project_cookie(&app, &auth_cookie, proj_id).await;

    // 1. Roadmap page shows v2.0 with 1 / 2 resolved (50%)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/roadmap_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Target Version: v2.0 — 1 / 2 resolved (50%)"));
    assert!(html.contains("Implement SQLite WAL mode"));
    assert!(html.contains("Add HTTP compression"));

    // 2. Changelog page shows only resolved b1 under v2.0
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/changelog_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Version: v2.0 (1 issues fixed)"));
    assert!(html.contains("Implement SQLite WAL mode"));
    assert!(!html.contains("Add HTTP compression"));

    // 3. Summary page shows Open: 1 | Resolved: 1 | Closed: 0
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/summary_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Total Issues: 2"));
    assert!(html.contains("<strong>Open:</strong> 1"));
    assert!(html.contains("<strong>Resolved:</strong> 1"));

    // 4. Add bugnote with HH:MM time tracking ("01:30" = 90 mins) and check billing_page + view_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", b1))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Benchmarked+WAL+throughput&view_state=10&time_tracking=01%3A30",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", b1))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("01:30 (90 mins)"));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/billing_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("01:30 (90 mins)"));
    assert!(html.contains("Benchmarked WAL throughput"));

    // 5. Post project news & view on /news_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/news_add")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&headline=Engine+v2.0+Beta&body=All+WAL+features+landed&announcement=1&view_state=10",
                    proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/news_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Engine v2.0 Beta"));
    assert!(html.contains("All WAL features landed"));
    assert!(html.contains("Announcement"));

    // 6. Email notification queue page and flush
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_email_queue_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("root@localhost"));
    assert!(html.contains("Implement SQLite WAL mode"));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_email_queue_flush")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.list_queued_emails().unwrap().is_empty());
}
