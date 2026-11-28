use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login_as(app: &axum::Router, username: &str, password: &str) -> String {
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
    login_as(app, "administrator", "root").await
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
async fn test_bug_report_and_view() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("TestProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // GET /bug_report_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/bug_report_page")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // POST /bug_report
    let form = format!(
        "project_id={}&category_id=1&summary=Test+Bug&description=Desc+here&steps_to_reproduce=Step1&priority=30&severity=50&reproducibility=70&version=&view_state=10",
        proj_id
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
    let loc = res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.contains("/view_bug_page/"));

    // GET /view_bug_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&loc)
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Test Bug"));
    assert!(html.contains("Desc here"));
    assert!(html.contains("Step1"));

    // GET /view_all_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(String::from_utf8(body.to_vec())
        .unwrap()
        .contains("Test Bug"));
}

#[tokio::test]
async fn test_bug_workflow_actions_notes_relationships_attachments() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());
    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Initial Summary",
            "Initial Desc",
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
    let bug2_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Second Bug",
            "Desc 2",
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

    // Update bug summary
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "summary=Updated+Summary&category_id=1&priority=40&severity=60&reproducibility=70&view_state=10",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Change status to resolved
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("new_status=80&resolution=20&note=Fixed"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Add bugnote
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", bug_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("note=Test+note&view_state=10"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Add duplicate relationship (bug2 duplicate_of bug1)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_relationship_add/{}", bug2_id))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "dest_bug_id={}&relationship_type=1",
                    bug_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Upload attachment
    let boundary = "----boundary123";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello file\r\n--{}--\r\n",
        boundary, boundary
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &full_cookie)
                .header(
                    "content-type",
                    &format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify bug view shows updated summary, note, reverse relationship, and attachment
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
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("Updated Summary"));
    assert!(html.contains("Test note"));
    assert!(html.contains("has duplicate"));
    assert!(html.contains("test.txt"));

    // Download attachment
    let files = db.list_bug_files(bug_id).unwrap();
    assert_eq!(files.len(), 1);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/bug/{}/test.txt", files[0].id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dl_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&dl_bytes[..], b"hello file");

    // Delete bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_delete/{}", bug_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_bug(bug_id).unwrap().is_none());
}

#[tokio::test]
async fn test_private_bug_visibility_and_access_control() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("PublicProj", 10, 10, "", true).unwrap();
    let _viewer_id = db
        .create_user("viewer", "Viewer", "v@x.com", "pw", 10, true, false)
        .unwrap();
    let rep_id = db
        .create_user("reporter", "Reporter", "r@x.com", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("dev", "Developer", "d@x.com", "pw", 55, true, false)
        .unwrap();

    let pub_bug = db
        .create_bug(
            proj_id,
            rep_id,
            1,
            "Public Issue",
            "Pub",
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
    let priv_bug = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Classified Issue",
            "Priv",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();

    let app = build_router(db.clone());
    let viewer_cookie =
        set_project_cookie(&app, &login_as(&app, "viewer", "pw").await, proj_id).await;
    let rep_cookie =
        set_project_cookie(&app, &login_as(&app, "reporter", "pw").await, proj_id).await;
    let dev_cookie = set_project_cookie(&app, &login_as(&app, "dev", "pw").await, proj_id).await;

    // Viewer sees public bug in list, NOT private bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &viewer_cookie)
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
    assert!(html.contains("Public Issue"));
    assert!(!html.contains("Classified Issue"));

    // Viewer gets 403 when requesting private bug directly
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", priv_bug))
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Developer gets 200 on private bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", priv_bug))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Reporter cannot close or delete bug (403)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", pub_bug))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("new_status=90&resolution=20"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_delete/{}", pub_bug))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Reporter-owned private bug
    let rep_priv_bug = db
        .create_bug(
            proj_id,
            rep_id,
            1,
            "Reporter Secret Issue",
            "Priv Rep",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();

    // Reporter CAN view their own private bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", rep_priv_bug))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Reporter CAN attach a tag to their own private bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_tag_attach/{}", rep_priv_bug))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("tag_string=sec_tag"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Another reporter (rep2) CANNOT view reporter 1's private bug
    let rep2_id = db
        .create_user("reporter2", "Reporter2", "r2@x.com", "pw", 25, true, false)
        .unwrap();
    let rep2_cookie =
        set_project_cookie(&app, &login_as(&app, "reporter2", "pw").await, proj_id).await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", rep_priv_bug))
                .header("cookie", &rep2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Rep2 also gets 403 Forbidden with "Insufficient access to view private issue." on tag detach
    let tag_id = db.get_tag_by_name("sec_tag").unwrap().unwrap().id;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_tag_detach/{}/{}", rep_priv_bug, tag_id))
                .header("cookie", &rep2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body_text = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body_text.contains("Insufficient access to view private issue."));

    // Private bug handled by reporter2: reporter2 CAN view it
    let handled_priv_bug = db
        .create_bug(
            proj_id,
            dev_id,
            1,
            "Handled By Rep2 Secret Issue",
            "Priv Handled",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();
    db.assign_bug(handled_priv_bug, dev_id, rep2_id).unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", handled_priv_bug))
                .header("cookie", &rep2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Reporter 2 sees handled_priv_bug in /view_all_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &rep2_cookie)
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
    assert!(html.contains("Handled By Rep2 Secret Issue"));
    assert!(!html.contains("Reporter Secret Issue"));
}

#[tokio::test]
async fn test_file_size_limit_and_remaining_actions() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());
    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    let bug1 = db
        .create_bug(
            proj_id, 1, 1, "Bug One", "D1", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();
    let bug2 = db
        .create_bug(
            proj_id, 1, 1, "Bug Two", "D2", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Assign bug to admin (id=1) -> status transitions from 10 (new) to 50 (assigned)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_assign/{}", bug1))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("handler_id=1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(db.get_bug(bug1).unwrap().unwrap().status, 50);

    // Closed bug cannot transition to resolved (returns redirect with ?err=...)
    db.change_bug_status(bug1, 1, 90, 20, None).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug1))
                .header("cookie", &full_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("new_status=80&resolution=20"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("err="));

    // Add & delete relationship via HTTP
    db.add_bug_relationship(bug1, bug2, 1).unwrap();
    let rel_id = db.list_bug_relationships(bug1).unwrap()[0].id;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_relationship_delete/{}", rel_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.list_bug_relationships(bug1).unwrap().is_empty());
    assert!(db.list_bug_relationships(bug2).unwrap().is_empty());

    // Oversized file upload returns 413 PAYLOAD_TOO_LARGE before touching DB
    let boundary = "----b413";
    let huge = vec![b'X'; 5_242_881]; // default limit is 5_242_880
    let mut mp = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"big.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n",
        boundary
    )
    .into_bytes();
    mp.extend_from_slice(&huge);
    mp.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug1))
                .header("cookie", &full_cookie)
                .header(
                    "content-type",
                    &format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(mp))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(db.list_bug_files(bug1).unwrap().is_empty());
}

#[tokio::test]
async fn test_p1_m2_issue_fields_subprojects_history_e2e() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Setup users:
    // admin (90), mgr (70), dev (55), rep (25)
    let admin_cookie = login_as_admin(&app).await;

    let _mgr_id = db
        .create_user(
            "mgr_user",
            "Manager",
            "mgr@example.com",
            "root123",
            70,
            true,
            false,
        )
        .unwrap();
    let mgr_cookie = login_as(&app, "mgr_user", "root123").await;

    let dev_id = db
        .create_user(
            "dev_user",
            "Developer",
            "dev@example.com",
            "root123",
            55,
            true,
            false,
        )
        .unwrap();

    let rep_id = db
        .create_user(
            "rep_user",
            "Reporter",
            "rep@example.com",
            "root123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_cookie = login_as(&app, "rep_user", "root123").await;

    // 2. Setup project & category with default assignee (dev_user)
    let proj_id = db
        .create_project("MainRollupProj", 10, 10, "Main Proj", true)
        .unwrap();
    let cat_id = db.add_category(proj_id, "DevCategory", dev_id).unwrap();

    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;
    let full_mgr_cookie = set_project_cookie(&app, &mgr_cookie, proj_id).await;
    let full_rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    // 3. Test Bug report with build, due_date (overdue in past), and default category assignee
    let form = format!(
        "project_id={}&category_id={}&summary=P1M2+Bug&description=Testing+M2&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=1.0&build=build-alpha-1&due_date=2024-01-15&view_state=10",
        proj_id, cat_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &full_admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify bug in DB
    let bugs = db.list_bugs(proj_id, 0, 10).unwrap();
    let bug = bugs.iter().find(|b| b.summary == "P1M2 Bug").unwrap();
    let bug_id = bug.id;
    assert_eq!(bug.handler_id, dev_id); // Default assignee applied!
    assert_eq!(bug.status, 50); // Assigned!
    assert_eq!(bug.build, "build-alpha-1");
    assert_eq!(bug.due_date, 1705276800); // 2024-01-15 00:00:00 UTC

    // 4. GET /view_bug_page/{bug_id} displays build, due_date, and Overdue badge
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &full_admin_cookie)
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
    assert!(html.contains("build-alpha-1"));
    assert!(html.contains("2024-01-15"));
    assert!(html.contains("Overdue"));

    // 5. Sticky Toggle Permissions:
    // Non-manager (Reporter: 25) attempts POST /bug_stick/{bug_id} -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_stick/{}", bug_id))
                .header("cookie", &full_rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bug_check = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug_check.sticky, 0);

    // Manager (70) posts /bug_stick/{bug_id} -> 303 See Other
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_stick/{}", bug_id))
                .header("cookie", &full_mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_check = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug_check.sticky, 1);

    // Verify Sticky indicator and Unstick button on view_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &full_mgr_cookie)
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
    assert!(html.contains("[Sticky]"));
    assert!(html.contains("Unstick"));

    // Verify Sticky indicator on view_all_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &full_admin_cookie)
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
    assert!(html.contains("[Sticky]"));

    // Manager un-sticks bug
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_stick/{}", bug_id))
                .header("cookie", &full_mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().sticky, 0);

    // 6. Test update_bug with os, os_build, platform, build, due_date
    let update_form = format!(
        "summary=P1M2+Bug+Updated&category_id={}&priority=40&severity=60&reproducibility=30&version=2.0&os=FreeBSD&os_build=14.0&platform=amd64&build=build-release-1&due_date=2028-06-30&view_state=10",
        cat_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &full_admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let updated_bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(updated_bug.os, "FreeBSD");
    assert_eq!(updated_bug.os_build, "14.0");
    assert_eq!(updated_bug.platform, "amd64");
    assert_eq!(updated_bug.build, "build-release-1");
    assert_eq!(updated_bug.due_date, 1845936000); // 2028-06-30

    // View page verifies updated fields, lack of Overdue badge (future date), and readable history
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &full_admin_cookie)
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
    assert!(html.contains("FreeBSD"));
    assert!(html.contains("14.0"));
    assert!(html.contains("amd64"));
    assert!(html.contains("build-release-1"));
    assert!(html.contains("2028-06-30"));
    assert!(!html.contains("Overdue")); // future date -> not overdue!
    assert!(html.contains("no =&gt; yes")); // Human readable history
    assert!(html.contains("yes =&gt; no"));

    // 7. Subproject roll-up & private isolation
    let child_proj = db
        .create_project("ChildRollup", 10, 10, "Child", true)
        .unwrap();
    let priv_child_proj = db
        .create_project("PrivateChildRollup", 10, 50, "Private child", true)
        .unwrap();
    db.add_subproject(proj_id, child_proj, true).unwrap();
    db.add_subproject(proj_id, priv_child_proj, true).unwrap();

    let bug_child = db
        .create_bug(
            child_proj,
            dev_id,
            0,
            "Child Project Bug",
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
    let bug_priv = db
        .create_bug(
            priv_child_proj,
            dev_id,
            0,
            "Private Child Bug",
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

    // Reporter viewing all bugs for MainRollupProj -> sees bug_id and bug_child, but NOT bug_priv!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
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
    assert!(html.contains(&format!("{:07}", bug_id)));
    assert!(html.contains(&format!("{:07}", bug_child)));
    assert!(!html.contains(&format!("{:07}", bug_priv)));

    // Manager (access 70) viewing all bugs for MainRollupProj -> sees all 3!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page")
                .header("cookie", &full_mgr_cookie)
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
    assert!(html.contains(&format!("{:07}", bug_id)));
    assert!(html.contains(&format!("{:07}", bug_child)));
    assert!(html.contains(&format!("{:07}", bug_priv)));

    // 8. Enforce Project.access_min
    let strict_proj = db
        .create_project("StrictAccessProj", 10, 10, "Min access 40", true)
        .unwrap();
    db.set_project_access_min(strict_proj, 40).unwrap();

    // Reporter (25 < 40) is blocked when submitting to StrictAccessProj -> 403 Forbidden
    let strict_form = format!(
        "project_id={}&category_id=1&summary=Strict+Bug&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=1.0&view_state=10",
        strict_proj
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &full_rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(strict_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // After granting explicit membership override (access 55 >= 40)
    db.set_project_user(strict_proj, rep_id, 55).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &full_rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=1&summary=Strict+Bug+Allowed&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=1.0&view_state=10",
                    strict_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}
