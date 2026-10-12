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
            proj_id, 1, 1, "Initial Summary", "Initial Desc", "", "", 30, 50, 70, "", "", "", "",
            10,
        )
        .unwrap();
    let bug2_id = db
        .create_bug(
            proj_id, 1, 1, "Second Bug", "Desc 2", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Update bug summary
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/update_bug/{}", bug_id))
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
                .uri(&format!("/bug_change_status/{}", bug_id))
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
                .uri(&format!("/bugnote_add/{}", bug_id))
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
                .uri(&format!("/bug_relationship_add/{}", bug2_id))
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
                .uri(&format!("/bug_file_add/{}", bug_id))
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
                .uri(&format!("/view_bug_page/{}", bug_id))
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
                .uri(&format!("/file/bug/{}/test.txt", files[0].id))
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
                .uri(&format!("/bug_delete/{}", bug_id))
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
            proj_id, rep_id, 1, "Public Issue", "Pub", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();
    let priv_bug = db
        .create_bug(
            proj_id, dev_id, 1, "Classified Issue", "Priv", "", "", 30, 50, 70, "", "", "", "", 50,
        )
        .unwrap();

    let app = build_router(db.clone());
    let viewer_cookie = set_project_cookie(&app, &login_as(&app, "viewer", "pw").await, proj_id).await;
    let rep_cookie = set_project_cookie(&app, &login_as(&app, "reporter", "pw").await, proj_id).await;
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
                .uri(&format!("/view_bug_page/{}", priv_bug))
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
                .uri(&format!("/view_bug_page/{}", priv_bug))
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
                .uri(&format!("/bug_change_status/{}", pub_bug))
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
                .uri(&format!("/bug_delete/{}", pub_bug))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
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
                .uri(&format!("/bug_assign/{}", bug1))
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
                .uri(&format!("/bug_change_status/{}", bug1))
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
                .uri(&format!("/bug_relationship_delete/{}", rel_id))
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
                .uri(&format!("/bug_file_add/{}", bug1))
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

