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
async fn test_http_view_issues_pagination_preserves_active_filter_params() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("PaginationProj", 10, 10, "", true)
        .unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // Create 35 bugs with status=10 (new), priority=40 (high)
    for i in 1..=35 {
        db.create_bug(
            proj_id,
            1,
            1,
            &format!("Test issue number {:02}", i),
            "Description of bug",
            "",
            "",
            40,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    }

    // Request page 1 with active filters: q=issue, priority=40, status=10, sort=id, dir=asc, per_page=10
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?page=1&q=issue&priority=40&status=10&sort=id&dir=asc&per_page=10")
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

    // Verify next link points to page=2 and retains all active parameters
    assert!(html.contains("Next »"));
    assert!(html.contains(r#"href="/view_all_bug_page?page=2"#));
    assert!(html.contains("q=issue"));
    assert!(html.contains("priority=40"));
    assert!(html.contains("status=10"));
    assert!(html.contains("sort=id"));
    assert!(html.contains("dir=asc"));
    assert!(html.contains("per_page=10"));

    // Request page 2
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?page=2&q=issue&priority=40&status=10&sort=id&dir=asc&per_page=10")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html2 = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    // Both Prev (page 1) and Next (page 3) links exist and retain all parameters
    assert!(html2.contains("« Prev"));
    assert!(html2.contains(r#"href="/view_all_bug_page?page=1"#));
    assert!(html2.contains("Next »"));
    assert!(html2.contains(r#"href="/view_all_bug_page?page=3"#));
    assert!(html2.contains("sort=id"));
    assert!(html2.contains("dir=asc"));
    assert!(html2.contains("per_page=10"));
}

#[tokio::test]
async fn test_http_view_issues_sorting_and_per_page() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("SortProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // Normal bug with low priority (20)
    let _bug1 = db
        .create_bug(
            proj_id,
            1,
            1,
            "Beta low priority",
            "",
            "",
            "",
            20,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Normal bug with high priority (50)
    let _bug2 = db
        .create_bug(
            proj_id,
            1,
            1,
            "Zeta high priority",
            "",
            "",
            "",
            50,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Sticky bug with low priority (10)
    let bug3 = db
        .create_bug(
            proj_id,
            1,
            1,
            "Alpha sticky low",
            "",
            "",
            "",
            10,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.conn()
        .execute(
            "UPDATE mantis_bug_table SET sticky = 1 WHERE id = ?1",
            rusqlite::params![bug3],
        )
        .unwrap();

    // Sticky bug with highest priority (60)
    let bug4 = db
        .create_bug(
            proj_id,
            1,
            1,
            "Omega sticky critical",
            "",
            "",
            "",
            60,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.conn()
        .execute(
            "UPDATE mantis_bug_table SET sticky = 1 WHERE id = ?1",
            rusqlite::params![bug4],
        )
        .unwrap();

    // 1. Sort by priority DESC: sticky bugs must come first (bug4 then bug3), then normal (bug2 then bug1)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?sort=priority&dir=desc")
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

    let pos_bug4 = html.find("Omega sticky critical").unwrap();
    let pos_bug3 = html.find("Alpha sticky low").unwrap();
    let pos_bug2 = html.find("Zeta high priority").unwrap();
    let pos_bug1 = html.find("Beta low priority").unwrap();

    assert!(
        pos_bug4 < pos_bug3,
        "Bug 4 (sticky 60) should precede Bug 3 (sticky 10)"
    );
    assert!(
        pos_bug3 < pos_bug2,
        "Bug 3 (sticky 10) should precede Bug 2 (normal 50)"
    );
    assert!(
        pos_bug2 < pos_bug1,
        "Bug 2 (normal 50) should precede Bug 1 (normal 20)"
    );

    // 2. Sort by summary ASC: sticky bugs must come first (bug3 Alpha then bug4 Omega), then normal (bug1 Beta then bug2 Zeta)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?sort=summary&dir=asc")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html2 = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    let pos_bug3_asc = html2.find("Alpha sticky low").unwrap();
    let pos_bug4_asc = html2.find("Omega sticky critical").unwrap();
    let pos_bug1_asc = html2.find("Beta low priority").unwrap();
    let pos_bug2_asc = html2.find("Zeta high priority").unwrap();

    assert!(
        pos_bug3_asc < pos_bug4_asc,
        "Alpha (sticky) should precede Omega (sticky)"
    );
    assert!(
        pos_bug4_asc < pos_bug1_asc,
        "Omega (sticky) should precede Beta (normal)"
    );
    assert!(
        pos_bug1_asc < pos_bug2_asc,
        "Beta (normal) should precede Zeta (normal)"
    );

    // 3. Lenient per_page query handling: non-numeric or invalid per_page should NOT return 400 Bad Request
    let res_abc = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?per_page=abc")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_abc.status(), StatusCode::OK);

    let res_neg = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?per_page=-1")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_neg.status(), StatusCode::OK);

    let res_empty = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?per_page=")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_empty.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_saved_filter_delete_ui_and_permissions() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("FilterProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    // Create users: reporter1 (id 2), manager (id 3)
    let rep1_id = db
        .create_user(
            "rep_one",
            "Reporter One",
            "r1@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let mgr_id = db
        .create_user(
            "mgr_user",
            "Manager User",
            "m@x.com",
            "pass123",
            70,
            true,
            false,
        )
        .unwrap();

    // 1. Reporter 1 logs in and saves a private filter
    let cookie_rep1 = login_user(&app, "rep_one", "pass123").await;
    let full_cookie_rep1 = set_project_cookie(&app, &cookie_rep1, proj_id).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &full_cookie_rep1)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=Reporter+1+Filter&search=alpha"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let fid_rep1 = db.list_visible_filters(rep1_id, proj_id).unwrap()[0].id;

    // 2. Manager logs in and saves a public filter
    let cookie_mgr = login_user(&app, "mgr_user", "pass123").await;
    let full_cookie_mgr = set_project_cookie(&app, &cookie_mgr, proj_id).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &full_cookie_mgr)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=Manager+Public+Filter&search=beta&is_public=1&sort=priority&dir=asc&per_page=50",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let fid_pub = db
        .list_visible_filters(mgr_id, proj_id)
        .unwrap()
        .into_iter()
        .find(|f| f.name == "Manager Public Filter")
        .unwrap()
        .id;

    // Verify saved-filter sort and per-page are reflected in UI headers and CSV export link
    let res_filter_view = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", fid_pub))
                .header("cookie", &full_cookie_mgr)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_filter_view.status(), StatusCode::OK);
    let html_filter_view = String::from_utf8(
        axum::body::to_bytes(res_filter_view.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    // Priority column header must show asc caret ('fa-caret-up') and target 'dir=desc' when clicked
    assert!(
        html_filter_view.contains(r#"title="Priority""#)
            && html_filter_view.contains("fa-caret-up")
    );
    assert!(html_filter_view.contains(&format!("filter_id={}", fid_pub)));
    assert!(html_filter_view.contains("sort=priority"));
    // CSV export button must include filter_id, sort, and dir
    assert!(html_filter_view.contains(&format!(
        r#"href="/csv_export?filter_id={}&sort=priority&dir=asc""#,
        fid_pub
    )));

    // 3. Reporter 1 views issues page:
    // - Sees fid_rep1 (owner) -> has delete button
    // - Sees fid_pub (public, but not owner & access < 70) -> does NOT have delete button
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &full_cookie_rep1)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_rep1 = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html_rep1.contains(&format!(r#"action="/filter_delete/{}""#, fid_rep1)));
    assert!(html_rep1.contains("Manager Public Filter"));
    assert!(!html_rep1.contains(&format!(r#"action="/filter_delete/{}""#, fid_pub)));

    // 4. Reporter 1 attempts to POST delete the Manager's filter: denied with 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", fid_pub))
                .header("cookie", &full_cookie_rep1)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(
        db.get_filter(fid_pub).unwrap().is_some(),
        "Public filter must still exist"
    );

    // 5. Manager logs in: sees delete buttons for the public filter
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &full_cookie_mgr)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_mgr = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html_mgr.contains(&format!(r#"action="/filter_delete/{}""#, fid_pub)));

    // 6. Reporter 1 deletes own filter: succeeds (303 See Other)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", fid_rep1))
                .header("cookie", &full_cookie_rep1)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(
        db.get_filter(fid_rep1).unwrap().is_none(),
        "Reporter filter must be deleted"
    );

    // 7. Manager can delete any filter (e.g. fid_pub): succeeds (303 See Other)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", fid_pub))
                .header("cookie", &full_cookie_mgr)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(
        db.get_filter(fid_pub).unwrap().is_none(),
        "Public filter must be deleted"
    );
}
