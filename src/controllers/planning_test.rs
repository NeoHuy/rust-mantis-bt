use super::*;

#[test]
fn test_news_add_form_deserialize() {
    let form: NewsAddForm = serde_json::from_str(
        r#"{
            "headline": "Sprint Freeze",
            "body": "Testing phase",
            "announcement": "1",
            "view_state": 10
        }"#,
    )
    .unwrap();
    assert_eq!(form.headline, "Sprint Freeze");
    assert_eq!(form.body, "Testing phase");
    assert_eq!(form.announcement.as_deref(), Some("1"));
    assert_eq!(form.view_state, Some(10));
}

#[test]
fn test_news_add_form_defaults() {
    let form: NewsAddForm = serde_json::from_str(
        r#"{
            "headline": "Sprint Freeze",
            "body": "Testing phase"
        }"#,
    )
    .unwrap();
    assert_eq!(form.headline, "Sprint Freeze");
    assert_eq!(form.body, "Testing phase");
    assert_eq!(form.announcement, None);
    assert_eq!(form.view_state, None);
    assert_eq!(form.project_id, None);
}

#[test]
fn test_planning_flash_query_deserialize() {
    let q: FlashQuery = serde_json::from_str(r#"{"msg": "Updated", "err": "Failed"}"#).unwrap();
    assert_eq!(q.msg.as_deref(), Some("Updated"));
    assert_eq!(q.err.as_deref(), Some("Failed"));

    let q_empty: FlashQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q_empty.msg, None);
    assert_eq!(q_empty.err, None);
}

#[tokio::test]
async fn test_phase3_m3_summary_graph_and_billing_exports() {
    use crate::controllers::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("GraphCtrlProj", 10, 10, "", true)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "=CMD|'calc'!A0 Bug",
            "Body",
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
    db.add_bugnote_with_time(bug_id, 1, "@SUM(1,2) note", 10, 90)
        .unwrap();
    let app = build_router(db.clone());

    let login_res = app
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
    let cookie = login_res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let res_graph = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/summary_graph_page?days=90")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_graph.status(), StatusCode::OK);
    let graph_html = String::from_utf8(
        axum::body::to_bytes(res_graph.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(graph_html.contains("<svg"));
    assert!(graph_html.contains("Last 90 Days"));

    let res_csv = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_export_csv?cost=120")
                .header("cookie", &cookie)
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
    assert!(csv.contains("'=CMD|'calc'!A0 Bug"));
    assert!(csv.contains("'@SUM(1,2) note"));
    assert!(csv.contains("180.00"));

    // Invalid summary days string falls back cleanly to 30 days (200 OK)
    let res_bad_days = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/summary_page?days=abc")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_days.status(), StatusCode::OK);
    let bad_days_html = String::from_utf8(
        axum::body::to_bytes(res_bad_days.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(bad_days_html.contains("Last 30 Days"));

    // Billing extreme rate clamps cleanly & reversed/invalid dates surface warnings without crashing
    let res_rev = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page?from=2026-12-01&to=2026-01-01&cost=1e308")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rev.status(), StatusCode::OK);
    let rev_html = String::from_utf8(
        axum::body::to_bytes(res_rev.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rev_html.contains("Start date is after end date"));
    assert!(rev_html.contains("No time tracking entries recorded."));

    let res_bad_date = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page?from=not-a-date")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_date.status(), StatusCode::OK);
    let bad_date_html = String::from_utf8(
        axum::body::to_bytes(res_bad_date.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(bad_date_html.contains("Invalid date filter ignored"));
    assert!(!bad_date_html.contains(r#"value="not-a-date""#));

    // Global Reporter with project-level Developer override accesses /billing_page under All Projects
    let rep_id = db
        .create_user(
            "projdev",
            "dev@example.com",
            "Proj Dev",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    db.set_project_user(proj_id, rep_id, 55).unwrap();
    let rep_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=projdev&password=password"))
                .unwrap(),
        )
        .await
        .unwrap();
    let rep_cookie = rep_login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let res_projdev_bill = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_projdev_bill.status(), StatusCode::OK);

    // Reporter (global 25) is denied per-developer/reporter charts on /summary_graph_page
    let _global_rep_id = db
        .create_user(
            "rep_only",
            "rep@example.com",
            "Rep Only",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let global_rep_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=rep_only&password=password"))
                .unwrap(),
        )
        .await
        .unwrap();
    let global_rep_cookie = global_rep_login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let res_rep_graph = app
        .oneshot(
            Request::builder()
                .uri("/summary_graph_page")
                .header("cookie", &global_rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rep_graph.status(), StatusCode::OK);
    let rep_graph_html = String::from_utf8(
        axum::body::to_bytes(res_rep_graph.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!rep_graph_html.contains("By Developer"));
    assert!(!rep_graph_html.contains("By Reporter"));
}
