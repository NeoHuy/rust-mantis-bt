use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
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
async fn test_custom_fields_tags_monitors_and_my_view() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("AppProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // 1. Create a Custom Field via HTTP
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_custom_field_create")
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=Browser&field_type=0&default_value=Chrome&display_report=1&display_update=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cf_id = db.list_custom_fields().unwrap()[0].id;

    // Link Custom Field to Project
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_custom_field_proj_add/{}", cf_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}&sequence=1", proj_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 2. Report a bug with the custom field value
    let form = format!(
        "project_id={}&category_id=1&summary=Crash+on+Safari&description=Webkit+crash&custom_field_{}=Safari+17",
        proj_id, cf_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_id = db.list_bugs(proj_id, 90, 0, 10).unwrap()[0].id;

    // 3. Attach tags to the bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_tag_attach/{}", bug_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("tag_string=safari,+frontend"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 4. Monitor the bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", bug_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 5. View bug page -> verify custom field value and tags appear
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("Browser"));
    assert!(html.contains("Safari 17"));
    assert!(html.contains("safari"));
    assert!(html.contains("frontend"));
    assert!(html.contains("End Monitoring"));

    // 6. My View page -> shows the bug under Reported by Me and Monitored by Me
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/my_view_page")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let my_view_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(my_view_html.contains("Assigned to Me"));
    assert!(my_view_html.contains("Monitored by Me (1)"));
    assert!(my_view_html.contains("Crash on Safari"));
}

#[tokio::test]
async fn test_search_saved_filters_and_csv_export_sanitization() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("ExportProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    db.create_bug(
        proj_id,
        1,
        1,
        "Database deadlock on commit",
        "SQLite busy",
        "",
        "",
        40,
        60,
        10,
        "",
        "",
        "",
        "",
        10,
    )
    .unwrap();
    db.create_bug(
        proj_id,
        1,
        1,
        "=CMD|' /C calc'!A0",
        "Formula injection test",
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

    // 1. Keyword search filters down to matching bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?q=deadlock")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("Database deadlock on commit"));
    assert!(!html.contains("calc"));

    // 2. Save a filter and apply it via ?filter_id=
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=Deadlock+Filter&search=deadlock&is_public=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let fid = db.list_visible_filters(1, proj_id).unwrap()[0].id;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", fid))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html2 = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html2.contains("Database deadlock on commit"));
    assert!(!html2.contains("calc"));

    // 3. CSV export sanitizes formula injection with leading single quote
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/csv_export")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let csv = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(csv.contains("Id,Project,Reporter,Assigned To"));
    assert!(csv.contains("'=CMD|' /C calc'!A0"));
}
