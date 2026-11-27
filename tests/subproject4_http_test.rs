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
    assert!(html.contains("<th>Open</th><th>Resolved</th><th>Closed</th><th>Total</th>"));
    assert!(html.contains(
        "<tr class=\"row-1\"><td>1</td><td>1</td><td>0</td><td><strong>2</strong></td></tr>"
    ));

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

async fn login_with_creds(app: &axum::Router, username: &str, password: &str) -> String {
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
async fn test_phase3_m3_my_view_9box_config_and_rbac() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let rep_id = db
        .create_user("mv_rep", "MV Rep", "mvr@x.com", "pw123", 25, true, false)
        .unwrap();
    let pub_proj = db.create_project("MyViewPub", 10, 10, "", true).unwrap();

    // Feedback bug & Verify bug reported by rep, plus commented public bug & private bug
    let b_fb = db
        .create_bug(
            pub_proj,
            rep_id,
            1,
            "Feedback Needed Issue",
            "D",
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
    db.change_bug_status(b_fb, admin.id, 20, 10, None).unwrap();

    let b_ver = db
        .create_bug(
            pub_proj,
            rep_id,
            1,
            "Verify Me Issue",
            "D",
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
    db.change_bug_status(b_ver, admin.id, 80, 20, None).unwrap();

    let b_comm = db
        .create_bug(
            pub_proj,
            admin.id,
            1,
            "Commented By Rep Issue",
            "D",
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
    db.add_bugnote(b_comm, rep_id, "Rep public comment", 10)
        .unwrap();

    // Private bug where rep is NOT reporter/handler; should never show up for rep
    let b_priv = db
        .create_bug(
            pub_proj,
            admin.id,
            1,
            "Secret Admin Bug",
            "D",
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
    db.add_bugnote(b_priv, rep_id, "Stale comment", 10).unwrap();

    let app = build_router(db.clone());
    let rep_cookie = set_project_cookie(
        &app,
        &login_with_creds(&app, "mv_rep", "pw123").await,
        pub_proj,
    )
    .await;

    // 1. Config page renders cleanly
    let res_cfg = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/my_view_config_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_cfg.status(), StatusCode::OK);

    // 2. Enable Feedback (order 1), Verify (order 2), MyComments (order 3), hide timeline
    let res_upd = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/my_view_config_update")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "enabled_feedback=1&order_feedback=1&enabled_verify=1&order_verify=2&enabled_my_comments=1&order_my_comments=3",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_upd.status(), StatusCode::SEE_OTHER);

    let res_mv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/my_view_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mv.status(), StatusCode::OK);
    let mv_html = String::from_utf8(
        axum::body::to_bytes(res_mv.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let idx_fb = mv_html.find("Awaiting Feedback").unwrap();
    let idx_ver = mv_html.find("Waiting for Verification").unwrap();
    let idx_comm = mv_html.find("Issues I Have Commented On").unwrap();
    assert!(idx_fb < idx_ver && idx_ver < idx_comm);
    assert!(mv_html.contains("Feedback Needed Issue"));
    assert!(mv_html.contains("Verify Me Issue"));
    assert!(mv_html.contains("Commented By Rep Issue"));
    assert!(!mv_html.contains("Secret Admin Bug"));
    assert!(!mv_html.contains(r#"id="timeline""#));

    // 3. Reset back to defaults restores 6 default boxes + timeline
    let res_reset = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/my_view_config_update")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("action=reset"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_reset.status(), StatusCode::SEE_OTHER);
    let res_mv2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/my_view_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mv2_html = String::from_utf8(
        axum::body::to_bytes(res_mv2.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(mv2_html.contains(r#"id="timeline""#));
    assert!(mv2_html.contains("Assigned to Me (Unresolved)"));
}

#[tokio::test]
async fn test_phase3_m3_extended_summary_and_svg_graphs_with_rbac() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let rep_id = db
        .create_user("sum_rep", "Sum Rep", "sr@x.com", "pw123", 25, true, false)
        .unwrap();
    let pub_proj = db.create_project("SummaryPub", 10, 10, "", true).unwrap();
    let priv_proj = db.create_project("SummaryPriv", 10, 50, "", true).unwrap();

    let b1 = db
        .create_bug(
            pub_proj,
            rep_id,
            1,
            "Public Summary Issue",
            "D",
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
    db.change_bug_status(b1, admin.id, 80, 20, Some("Done"))
        .unwrap();

    let _b_open = db
        .create_bug(
            pub_proj,
            rep_id,
            1,
            "Open Active Issue",
            "D",
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
    let _b_priv = db
        .create_bug(
            priv_proj,
            admin.id,
            1,
            "Hidden Private Proj Issue",
            "D",
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
    let admin_cookie = login_as_admin(&app).await;
    let rep_cookie = login_with_creds(&app, "sum_rep", "pw123").await;

    // Admin sees all 3 issues, all Phase 3 Summary sections, and inline SVG charts
    let res_adm = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/summary_page?days=30")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_adm.status(), StatusCode::OK);
    let adm_html = String::from_utf8(
        axum::body::to_bytes(res_adm.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(adm_html.contains("Overview (Total Issues: 3)"));
    assert!(adm_html.contains("By Project"));
    assert!(adm_html.contains("SummaryPub"));
    assert!(adm_html.contains("SummaryPriv"));
    assert!(adm_html.contains("By Date (Opened vs Resolved)"));
    assert!(adm_html.contains("Longest Open Issues"));
    assert!(adm_html.contains("Most Active Open Issues"));
    assert!(adm_html.contains("Developer &times; Resolution Breakdown"));
    assert!(adm_html.contains("Reporter Effectiveness"));
    assert!(adm_html.contains("<svg"));

    // Reporter sees only 2 public issues (Private project isolated cleanly)
    let res_rep = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/summary_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let rep_html = String::from_utf8(
        axum::body::to_bytes(res_rep.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rep_html.contains("Overview (Total Issues: 2)"));
    assert!(!rep_html.contains("SummaryPriv"));
    assert!(!rep_html.contains("Hidden Private Proj Issue"));

    // Graphs page returns inline SVGs
    let res_g = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/summary_graph_page?days=365")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_g.status(), StatusCode::OK);
    let g_html = String::from_utf8(
        axum::body::to_bytes(res_g.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(g_html.contains("Issues Trend (Last 365 Days)"));
    assert!(g_html.contains("<svg"));
}

#[tokio::test]
async fn test_phase3_m3_billing_upgrade_subproject_rollup_and_exports() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev_id = db
        .create_user("bill_dev", "Bill Dev", "bd@x.com", "pw123", 55, true, false)
        .unwrap();
    let _rep_id = db
        .create_user("bill_rep", "Bill Rep", "br@x.com", "pw123", 25, true, false)
        .unwrap();

    let parent_p = db.create_project("BillParent", 10, 10, "", true).unwrap();
    let pub_child = db.create_project("BillPubChild", 10, 10, "", true).unwrap();
    let priv_child = db
        .create_project("BillPrivChild", 10, 50, "", true)
        .unwrap();
    db.add_subproject(parent_p, pub_child, true).unwrap();
    db.add_subproject(parent_p, priv_child, true).unwrap();

    let b_par = db
        .create_bug(
            parent_p,
            dev_id,
            1,
            "=CMD|'calc'!A0 Parent",
            "D",
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
    let b_pub = db
        .create_bug(
            pub_child,
            dev_id,
            1,
            "Pub Child Task",
            "D",
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
    let b_priv = db
        .create_bug(
            priv_child,
            admin.id,
            1,
            "Secret Private Child Task",
            "D",
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

    db.add_bugnote_with_time(b_par, dev_id, "Parent work log", 10, 60)
        .unwrap();
    db.add_bugnote_with_time(b_pub, dev_id, "@SUM(1,2) Child log & <tag>", 10, 120)
        .unwrap();
    db.add_bugnote_with_time(b_priv, admin.id, "Confidential child log", 10, 180)
        .unwrap();

    let app = build_router(db);
    let dev_cookie = set_project_cookie(
        &app,
        &login_with_creds(&app, "bill_dev", "pw123").await,
        parent_p,
    )
    .await;
    let rep_cookie = set_project_cookie(
        &app,
        &login_with_creds(&app, "bill_rep", "pw123").await,
        parent_p,
    )
    .await;

    // 1. Reporter gets 403 on billing page & exports
    for uri in [
        "/billing_page",
        "/billing_export_csv",
        "/billing_export_excel",
    ] {
        let res_deny = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", &rep_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_deny.status(), StatusCode::FORBIDDEN, "uri={}", uri);
    }

    // 2. Developer with subproject roll-up + rate sees Parent (60m) + PubChild (120m) = 180m ($360.00), NOT PrivChild
    let res_bill = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page?include_subprojects=1&cost=120")
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bill.status(), StatusCode::OK);
    let bill_html = String::from_utf8(
        axum::body::to_bytes(res_bill.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(bill_html.contains("03:00 (180 mins)"));
    assert!(bill_html.contains("$360.00"));
    assert!(bill_html.contains("Pub Child Task"));
    assert!(!bill_html.contains("Secret Private Child Task"));

    // 3. Future date range filters out entries cleanly
    let res_empty = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page?from=2099-01-01&include_subprojects=1")
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let empty_html = String::from_utf8(
        axum::body::to_bytes(res_empty.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(empty_html.contains("No time tracking entries recorded."));

    // 4. Billing CSV & Excel exports sanitize formula prefixes & isolate PrivChild
    let res_csv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_export_csv?include_subprojects=1&cost=120")
                .header("cookie", &dev_cookie)
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
    assert!(csv.contains("'=CMD|'calc'!A0 Parent"));
    assert!(csv.contains("'@SUM(1,2) Child log & <tag>"));
    assert!(csv.contains("240.00"));
    assert!(!csv.contains("Secret Private Child Task"));

    let res_xls = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_export_excel?include_subprojects=1&cost=120")
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_xls.status(), StatusCode::OK);
    let xls = String::from_utf8(
        axum::body::to_bytes(res_xls.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(xls.contains("<Workbook"));
    assert!(xls.contains("&apos;=CMD|&apos;calc&apos;!A0 Parent"));
    assert!(xls.contains("&apos;@SUM(1,2) Child log &amp; &lt;tag&gt;"));
    assert!(!xls.contains("Secret Private Child Task"));
}
