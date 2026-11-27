use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login_user(app: &axum::Router, username: &str, password: &str) -> String {
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

async fn login_as_admin(app: &axum::Router) -> String {
    login_user(app, "administrator", "root").await
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
    let bug_id = db.list_bugs(proj_id, 0, 10).unwrap()[0].id;

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

#[tokio::test]
async fn test_http_my_view_timeline_rbac_hides_private_issues_and_notes() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("TimelineProj", 10, 10, "", true).unwrap();
    let rep_id = db
        .create_user(
            "rep_tl_http",
            "Reporter",
            "rep_tl@test.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let dev_id = db
        .create_user(
            "dev_tl_http",
            "Developer",
            "dev_tl@test.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();

    let pub_bug = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Public Visible Issue",
            "desc",
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
    let priv_bug = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Secret Private Issue",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();
    db.add_bugnote(pub_bug, dev_id, "Private dev note only", 50)
        .unwrap();
    let _ = rep_id;

    let app = build_router(db.clone());
    let rep_cookie = login_user(&app, "rep_tl_http", "pass123").await;
    let full_rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/my_view_page")
                .header("cookie", &full_rep_cookie)
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

    assert!(html.contains("Timeline"));
    assert!(html.contains("Public Visible Issue"));
    assert!(!html.contains("Secret Private Issue"));
    assert!(!html.contains(&format!("{:07}", priv_bug)));
    assert!(!html.contains("commented on issue"));
}

#[tokio::test]
async fn test_http_advanced_filters_manage_filters_and_column_customization() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("Phase3FilterProj", 10, 10, "", true)
        .unwrap();
    let priv_proj_id = db.create_project("SecretProj", 10, 50, "", true).unwrap();

    let rep_id = db
        .create_user(
            "p3_rep",
            "Reporter P3",
            "p3rep@test.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let dev_id = db
        .create_user(
            "p3_dev",
            "Developer P3",
            "p3dev@test.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();
    let mgr_id = db
        .create_user(
            "p3_mgr",
            "Manager P3",
            "p3mgr@test.com",
            "pass123",
            70,
            true,
            false,
        )
        .unwrap();

    let cf_browser = db
        .create_custom_field_full(
            "Browser",
            3,
            "Chrome|Firefox|Safari",
            "",
            "",
            10,
            25,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    let cf_secret = db
        .create_custom_field_full(
            "AdminSecret",
            0,
            "",
            "",
            "",
            70,
            70,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    db.link_custom_field_to_project(cf_browser, proj_id, 1)
        .unwrap();
    db.link_custom_field_to_project(cf_secret, proj_id, 2)
        .unwrap();

    let b1 = db
        .create_bug_full_with_mentions(
            proj_id,
            rep_id,
            1,
            "MultiFilter Alpha",
            "Desc 1",
            "",
            "",
            40, // high
            60, // major
            10, // always
            "v1.0",
            "Linux",
            "6.1",
            "x86_64",
            10,
            &[],
            rust_mantis_bt::db::BugCreateExtras {
                target_version: "v2.0",
                build: "b10",
                due_date: 0,
                handler_id: Some(dev_id),
            },
        )
        .unwrap();
    let b2 = db
        .create_bug_full_with_mentions(
            proj_id,
            dev_id,
            1,
            "MultiFilter Beta",
            "Desc 2",
            "",
            "",
            20, // low
            80, // block
            30, // sometimes
            "v1.1",
            "macOS",
            "15",
            "arm64",
            10,
            &[],
            rust_mantis_bt::db::BugCreateExtras {
                target_version: "v2.1",
                build: "b11",
                due_date: 0,
                handler_id: None,
            },
        )
        .unwrap();
    let b3 = db
        .create_bug_full_with_mentions(
            proj_id,
            rep_id,
            1,
            "=CMD|'calc'!A0",
            "Formula check",
            "",
            "",
            30,
            50,
            10,
            "v1.0",
            "Windows",
            "11",
            "x64",
            10,
            &[],
            rust_mantis_bt::db::BugCreateExtras::default(),
        )
        .unwrap();
    db.change_bug_status(b2, dev_id, 80, 20, None).unwrap();
    db.update_bug_fields(
        b2,
        dev_id,
        rust_mantis_bt::models::BugUpdate {
            fixed_in_version: Some("v1.1.1".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    db.set_custom_field_value(cf_browser, b1, mgr_id, "Chrome")
        .unwrap();
    db.set_custom_field_value(cf_browser, b2, mgr_id, "Firefox")
        .unwrap();
    db.set_custom_field_value(cf_secret, b1, mgr_id, "CONFIDENTIAL_X")
        .unwrap();
    db.set_custom_field_value(cf_secret, b3, mgr_id, "=SUM(1,2)")
        .unwrap();

    // Update date_submitted / last_updated for deterministic date-range HTTP check
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_bug_table SET date_submitted = 1704067200, last_updated = 1704067200 WHERE id = ?1",
            rusqlite::params![b1],
        )
        .unwrap();
        conn.execute(
            "UPDATE mantis_bug_table SET date_submitted = 1735689600, last_updated = 1735689600 WHERE id = ?1",
            rusqlite::params![b2],
        )
        .unwrap();
    }

    let app = build_router(db.clone());
    let rep_cookie =
        set_project_cookie(&app, &login_user(&app, "p3_rep", "pass123").await, proj_id).await;
    let mgr_cookie =
        set_project_cookie(&app, &login_user(&app, "p3_mgr", "pass123").await, proj_id).await;

    // 1. Multi-select + MATCH ANY HTTP test: priority=40 OR severity=80 -> matches b1 and b2, excludes b3
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?match_type=1&priority=40&severity=80")
                .header("cookie", &rep_cookie)
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
    assert!(html.contains("MultiFilter Alpha"));
    assert!(html.contains("MultiFilter Beta"));
    assert!(!html.contains("Formula check"));

    // 2. Date range + custom field + version filter HTTP test
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/view_all_bug_page?date_submitted_from=2024-01-01&date_submitted_to=2024-01-02&version=v1.0&custom_field_{}=Chrome",
                    cf_browser
                ))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_cf = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html_cf.contains("MultiFilter Alpha"));
    assert!(!html_cf.contains("MultiFilter Beta"));

    // 3. RBAC: Reporter querying a Manager-only custom field gets no match (no oracle leak)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/view_all_bug_page?custom_field_{}=CONFIDENTIAL_X",
                    cf_secret
                ))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_oracle = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!html_oracle.contains("MultiFilter Alpha"));

    // 4. Save filter (multi-select + custom field) & Manage Filters lifecycle + RBAC
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=Rep+Saved+CF&match_type=1&priority=40&priority=20&custom_field_{}=Chrome",
                    cf_browser
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let rep_fid = db.list_visible_filters(rep_id, proj_id).unwrap()[0].id;

    // Manager tries to view Reporter's private filter via permalink -> allowed (>= 70),
    // whereas another Reporter cannot!
    let other_rep_id = db
        .create_user(
            "p3_rep2",
            "Reporter2",
            "p3rep2@test.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let _ = other_rep_id;
    let rep2_cookie =
        set_project_cookie(&app, &login_user(&app, "p3_rep2", "pass123").await, proj_id).await;
    let res_priv_deny = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", rep_fid))
                .header("cookie", &rep2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_priv_deny.status(), StatusCode::FORBIDDEN);

    // Filter belonging to inaccessible private project -> 403 Forbidden
    let priv_fid = db
        .save_filter(
            mgr_id,
            priv_proj_id,
            true,
            "SecretProj Filter",
            &rust_mantis_bt::models::BugFilter::default(),
        )
        .unwrap();
    let res_priv_proj = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", priv_fid))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_priv_proj.status(), StatusCode::FORBIDDEN);

    // Manage Filters page & edit & RBAC guard on is_public
    let res_mf = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_filter_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mf.status(), StatusCode::OK);
    let mf_html = String::from_utf8(
        axum::body::to_bytes(res_mf.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(mf_html.contains("Rep Saved CF"));
    assert!(mf_html.contains(&format!("/view_all_bug_page?filter_id={}", rep_fid)));

    // Reporter cannot promote private filter to public (403 Forbidden)
    let res_pub_deny = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_filter_update/{}", rep_fid))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=Rep+Saved+CF&project_id={}&is_public=1",
                    proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_pub_deny.status(), StatusCode::FORBIDDEN);

    // Reporter CAN rename own private filter
    let res_rename = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_filter_update/{}", rep_fid))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=Renamed+Rep+Filter&project_id={}&match_type=0",
                    proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rename.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        db.get_filter(rep_fid).unwrap().unwrap().0.name,
        "Renamed Rep Filter"
    );

    // 5. Per-context Column Customization + View Issues + CSV Export formula sanitization
    let res_cols = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/account_manage_columns_page?project_id={}",
                    proj_id
                ))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_cols.status(), StatusCode::OK);
    let cols_html = String::from_utf8(
        axum::body::to_bytes(res_cols.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(cols_html.contains("Manage Columns"));
    assert!(cols_html.contains(&format!("custom_field_{}", cf_secret)));

    // Manager updates view & csv columns to include custom fields
    let res_col_save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_manage_columns_update")
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&view_issues_columns=id,target_version,custom_field_{},summary&print_issues_columns=id,summary&csv_columns=id,summary,custom_field_{}&excel_columns=id,summary",
                    proj_id, cf_browser, cf_secret
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_col_save.status(), StatusCode::SEE_OTHER);

    // View Issues renders configured columns (Target Version, Browser)
    let res_view_cols = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_view_cols.status(), StatusCode::OK);
    let view_cols_html = String::from_utf8(
        axum::body::to_bytes(res_view_cols.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(view_cols_html.contains("Target Version"));
    assert!(view_cols_html.contains("Browser"));
    assert!(view_cols_html.contains("v2.0"));
    assert!(view_cols_html.contains("Chrome"));

    // CSV Export respects custom csv_columns AND sanitizes formula injection in custom fields & summary
    let res_csv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/csv_export")
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_csv.status(), StatusCode::OK);
    let csv = String::from_utf8(
        axum::body::to_bytes(res_csv.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(csv.starts_with("Id,Summary,AdminSecret\r\n"));
    assert!(csv.contains("'=CMD|'calc'!A0"));
    assert!(csv.contains("'=SUM(1,2)"));

    // 6. C1/M1/M2/M4 HTTP regressions:
    // a) Missing saved filter redirects with err flash instead of 404
    let res_missing_f = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?filter_id=999999")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_missing_f.status(), StatusCode::SEE_OTHER);
    assert!(res_missing_f
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Saved+filter+not+found"));

    // b) Manager cannot list, view, edit, or publish Reporter's private filter; Admin can moderation-delete it
    let res_mgr_mf = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_filter_page")
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_mf.status(), StatusCode::OK);
    let mgr_mf_html = String::from_utf8(
        axum::body::to_bytes(res_mgr_mf.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!mgr_mf_html.contains("Renamed Rep Filter"));

    let res_mgr_priv_view = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", rep_fid))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_priv_view.status(), StatusCode::FORBIDDEN);

    let res_mgr_priv_edit = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/manage_filter_edit_page/{}", rep_fid))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_priv_edit.status(), StatusCode::FORBIDDEN);

    let res_mgr_priv_upd = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_filter_update/{}", rep_fid))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=Hijacked&project_id={}&is_public=1",
                    proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_priv_upd.status(), StatusCode::FORBIDDEN);

    // c) M1: Public saved filter containing restricted custom field + target_version hides secret CF from Reporter HTML/chip URL
    let mut f_with_secret_cf = rust_mantis_bt::models::BugFilter {
        project_id: Some(proj_id),
        target_versions: vec!["v2.0".to_string()],
        ..Default::default()
    };
    f_with_secret_cf
        .custom_fields
        .insert(cf_secret, vec!["TOPSECRET_VAL".to_string()]);
    let pub_cf_fid = db
        .save_filter(mgr_id, proj_id, true, "Team CF Filter", &f_with_secret_cf)
        .unwrap();
    let res_rep_pub_cf = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", pub_cf_fid))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rep_pub_cf.status(), StatusCode::OK);
    let rep_pub_cf_html = String::from_utf8(
        axum::body::to_bytes(res_rep_pub_cf.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!rep_pub_cf_html.contains("TOPSECRET_VAL"));
    assert!(!rep_pub_cf_html.contains(&format!("custom_field_{}", cf_secret)));

    // d) C1: Override-elevated Manager on proj_a vs Reporter on proj_b viewing All Projects + cf_secret (access_level_r=70)
    let cf_mgr_only = db
        .create_custom_field_full(
            "MgrOnlyCode",
            0,
            "",
            "",
            "",
            70,
            70,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    let proj_a = db.create_project("C1ProjA", 10, 10, "", true).unwrap();
    let proj_b = db.create_project("C1ProjB", 10, 10, "", true).unwrap();
    db.link_custom_field_to_project(cf_mgr_only, proj_a, 1)
        .unwrap();
    db.link_custom_field_to_project(cf_mgr_only, proj_b, 2)
        .unwrap();
    db.set_project_user(proj_a, rep_id, 70).unwrap();
    let cat_a = db.list_effective_categories(proj_a).unwrap()[0].id;
    let cat_b = db.list_effective_categories(proj_b).unwrap()[0].id;
    let bug_a = db
        .create_bug_full_with_mentions(
            proj_a,
            rep_id,
            cat_a,
            "BugA C1",
            "DescA",
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
            &[],
            rust_mantis_bt::db::BugCreateExtras::default(),
        )
        .unwrap();
    let bug_b = db
        .create_bug_full_with_mentions(
            proj_b,
            mgr_id,
            cat_b,
            "BugB C1",
            "DescB",
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
            &[],
            rust_mantis_bt::db::BugCreateExtras::default(),
        )
        .unwrap();
    db.set_custom_field_value(cf_mgr_only, bug_a, mgr_id, "SECRET_PROJ_A_OK")
        .unwrap();
    db.set_custom_field_value(cf_mgr_only, bug_b, mgr_id, "SECRET_PROJ_B_LEAK")
        .unwrap();
    let rep_all_cookie = set_project_cookie(&app, &rep_cookie, 0).await;
    let res_rep_cols = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_manage_columns_update")
                .header("cookie", &rep_all_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id=0&view_issues_columns=id,summary,custom_field_{}&print_issues_columns=id,summary&csv_columns=id,summary,custom_field_{}&excel_columns=id,summary",
                    cf_mgr_only, cf_mgr_only
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rep_cols.status(), StatusCode::SEE_OTHER);

    let res_c1_view = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &rep_all_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_c1_view.status(), StatusCode::OK);
    let c1_view_html = String::from_utf8(
        axum::body::to_bytes(res_c1_view.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(c1_view_html.contains("SECRET_PROJ_A_OK"));
    assert!(!c1_view_html.contains("SECRET_PROJ_B_LEAK"));

    let res_c1_csv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/csv_export")
                .header("cookie", &rep_all_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_c1_csv.status(), StatusCode::OK);
    let c1_csv = String::from_utf8(
        axum::body::to_bytes(res_c1_csv.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(c1_csv.contains("SECRET_PROJ_A_OK"));
    assert!(!c1_csv.contains("SECRET_PROJ_B_LEAK"));
}
