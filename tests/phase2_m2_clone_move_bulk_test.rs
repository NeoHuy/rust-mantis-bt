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

fn assert_no_nested_forms(html: &str) {
    let mut pos = 0;
    while let Some(form_start) = html[pos..].find("<form") {
        let actual_start = pos + form_start;
        let form_tag_close = html[actual_start..].find('>').expect("Unclosed form tag");
        let content_start = actual_start + form_tag_close + 1;
        let form_end = html[content_start..]
            .find("</form>")
            .expect("Unclosed form tag");
        let inner_form = &html[content_start..content_start + form_end];
        assert!(
            !inner_form.contains("<form"),
            "Detected nested <form> inside form: {}",
            inner_form
        );
        pos = content_start + form_end + 7;
    }
}

#[tokio::test]
async fn test_clone_issue_http_flow_and_separate_files() {
    let db = Db::open_in_memory().unwrap();
    let p_src = db
        .create_project("Project Alpha", 10, 10, "", true)
        .unwrap();
    let p_dst = db.create_project("Project Beta", 10, 10, "", true).unwrap();

    let u_dev = db
        .create_user(
            "dev_clone",
            "Developer",
            "dev_c@test.com",
            "pw",
            55,
            true,
            false,
        )
        .unwrap();
    let _u_viewer = db
        .create_user(
            "view_clone",
            "Viewer",
            "v_c@test.com",
            "pw",
            10,
            true,
            false,
        )
        .unwrap();

    let app = build_router(db.clone());

    // Create source bug in Project Alpha
    let bug_src = db
        .create_bug(
            p_src,
            u_dev,
            1,
            "Source Summary",
            "Source Description",
            "Steps",
            "Additional",
            30,
            50,
            10,
            "1.0",
            "macOS",
            "",
            "arm64",
            10,
        )
        .unwrap();

    // Upload attachment to source bug
    let temp_dir =
        std::env::temp_dir().join(format!("mantis_test_http_clone_{}", rand::random::<u64>()));
    let temp_str = temp_dir.to_str().unwrap().to_string();
    std::env::set_var("MANTIS_UPLOAD_DIR", &temp_str);

    let file_id = db
        .add_bug_file(
            bug_src,
            u_dev,
            "attachment.txt",
            "text/plain",
            b"original attachment bytes".to_vec(),
            &temp_str,
        )
        .unwrap();
    assert!(file_id > 0);

    // 1. Viewer cannot access clone page (access level 10 < 25)
    let viewer_cookie = login_user(&app, "view_clone", "pw").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_clone_page/{}", bug_src))
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Developer can access clone page
    let dev_cookie = login_user(&app, "dev_clone", "pw").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_clone_page/{}", bug_src))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("Clone Issue"));
    assert!(body.contains(r#"name="target_project_id""#));
    assert!(body.contains(r#"name="copy_relationships""#));
    assert!(body.contains(r#"name="copy_files""#));
    assert_no_nested_forms(&body);

    // 3. Clone bug to Project Beta via POST /bug_clone/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_clone/{}", bug_src))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "target_project_id={}&copy_relationships=1&copy_files=1",
                    p_dst
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.starts_with("/view_bug_page/"));
    let new_bug_id: i64 = location
        .trim_start_matches("/view_bug_page/")
        .split('?')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(new_bug_id, bug_src);

    // Verify cloned issue details
    let cloned_bug = db.get_bug(new_bug_id).unwrap().unwrap();
    assert_eq!(cloned_bug.project_id, p_dst);
    assert_eq!(cloned_bug.summary, "Source Summary");

    // Verify relationship
    let rels = db.list_bug_relationships(new_bug_id).unwrap();
    assert!(rels
        .iter()
        .any(|r| r.destination_bug_id == bug_src && r.relationship_type == 0));

    // Verify separate files
    let cloned_files = db.list_bug_files(new_bug_id).unwrap();
    assert_eq!(cloned_files.len(), 1);
    let cloned_file_id = cloned_files[0].id;
    let src_file_meta = db.get_bug_file_meta(file_id).unwrap().unwrap();
    let cloned_file_meta = db.get_bug_file_meta(cloned_file_id).unwrap().unwrap();
    assert_ne!(src_file_meta.diskfile, cloned_file_meta.diskfile);

    // Delete file on cloned bug -> source bug file remains intact
    db.delete_bug_file(cloned_file_id, &temp_str).unwrap();
    assert!(db.get_bug_file_meta(cloned_file_id).unwrap().is_none());
    assert!(db.get_bug_file_meta(file_id).unwrap().is_some());
    let src_data = db.get_bug_file(file_id, &temp_str).unwrap().unwrap();
    assert_eq!(src_data.1, b"original attachment bytes");

    // Check Quick Action Toolbar on /view_bug_page contains Clone button
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_src))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains(&format!(r#"href="/bug_clone_page/{}">Clone</a>"#, bug_src)));
    assert!(body.contains(&format!(r#"href="/bug_move_page/{}">Move</a>"#, bug_src)));

    std::env::remove_var("MANTIS_UPLOAD_DIR");
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_move_issue_http_flow_and_permission_guards() {
    let db = Db::open_in_memory().unwrap();
    let p_a = db
        .create_project("Project Alpha", 10, 10, "", true)
        .unwrap();
    let p_b = db
        .create_project("Project Beta", 10, 10, "", false)
        .unwrap();

    let cat_a = db.add_category(p_a, "Alpha Category", 0).unwrap();
    let cat_b = db.add_category(p_b, "Beta Category", 0).unwrap();

    let u_rep = db
        .create_user(
            "rep_move",
            "Reporter",
            "r_m@test.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let _u_dev = db
        .create_user(
            "dev_move",
            "Developer",
            "d_m@test.com",
            "pw",
            55,
            true,
            false,
        )
        .unwrap();

    let app = build_router(db.clone());

    let bug_id = db
        .create_bug(
            p_a,
            u_rep,
            cat_a,
            "Move Flow Summary",
            "Desc",
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

    // 1. Reporter cannot view move page (requires eff >= 55)
    let rep_cookie = login_user(&app, "rep_move", "pw").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_move_page/{}", bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Reporter cannot POST to /bug_move/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_move/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("target_project_id={}", p_b)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Developer can view move page
    let dev_cookie = login_user(&app, "dev_move", "pw").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_move_page/{}", bug_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("Move Issue"));
    assert!(body.contains(r#"name="target_project_id""#));
    assert_no_nested_forms(&body);

    // 3. Move issue to Project Beta
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_move/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("target_project_id={}", p_b)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers().get("location").unwrap().to_str().unwrap(),
        format!("/view_bug_page/{}?msg=Issue+moved+successfully.", bug_id)
    );

    // Verify bug moved and category fell back to cat_b
    let moved = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(moved.project_id, p_b);
    assert_eq!(moved.category_id, cat_b);

    // History verification
    let history = db.list_bug_history(bug_id).unwrap();
    assert!(history
        .iter()
        .any(|h| h.field_name == "project_id" && h.new_value == p_b.to_string()));
    assert!(history
        .iter()
        .any(|h| h.field_name == "category_id" && h.new_value == cat_b.to_string()));
}

#[tokio::test]
async fn test_bulk_actiongroup_mixed_permissions_and_execution() {
    let db = Db::open_in_memory().unwrap();
    let p_a = db
        .create_project("Project Alpha", 10, 10, "", true)
        .unwrap();
    let p_b = db.create_project("Project Beta", 10, 10, "", true).unwrap();

    let cat_a = db.add_category(p_a, "Cat Alpha", 0).unwrap();
    let cat_b = db.add_category(p_b, "Cat Beta", 0).unwrap();

    // User is Developer (55) on Proj Alpha, but Reporter (25) on Proj Beta
    let u_mixed = db
        .create_user(
            "mixed_user",
            "Mixed User",
            "m@test.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    db.set_project_user(p_a, u_mixed, 55).unwrap();
    db.set_project_user(p_b, u_mixed, 25).unwrap();

    let u_assignee = db
        .create_user("assignee", "Assignee", "a@test.com", "pw", 55, true, false)
        .unwrap();

    let app = build_router(db.clone());

    // Create 2 bugs in Project Alpha (eff=55) and 2 bugs in Project Beta (eff=25)
    let b1 = db
        .create_bug(
            p_a, u_mixed, cat_a, "Bug A1", "", "", "", 30, 50, 10, "", "", "", "", 10,
        )
        .unwrap();
    let b2 = db
        .create_bug(
            p_a, u_mixed, cat_a, "Bug A2", "", "", "", 30, 50, 10, "", "", "", "", 10,
        )
        .unwrap();
    let b3 = db
        .create_bug(
            p_b, u_mixed, cat_b, "Bug B1", "", "", "", 30, 50, 10, "", "", "", "", 10,
        )
        .unwrap();
    let b4 = db
        .create_bug(
            p_b, u_mixed, cat_b, "Bug B2", "", "", "", 30, 50, 10, "", "", "", "", 10,
        )
        .unwrap();

    let cookie = login_user(&app, "mixed_user", "pw").await;

    // 1. Bulk ASSIGN (requires eff >= 55):
    // b1, b2 are in Proj A (eff=55) -> ASSIGNED!
    // b3, b4 are in Proj B (eff=25 < 55) -> SKIPPED!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&bug_arr={}&bug_arr={}&action=ASSIGN&handler_id={}",
                    b1, b2, b3, b4, u_assignee
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("2+issue%28s%29+updated%2C+2+skipped"));

    assert_eq!(db.get_bug(b1).unwrap().unwrap().handler_id, u_assignee);
    assert_eq!(db.get_bug(b2).unwrap().unwrap().handler_id, u_assignee);
    assert_eq!(db.get_bug(b3).unwrap().unwrap().handler_id, 0); // Not assigned!
    assert_eq!(db.get_bug(b4).unwrap().unwrap().handler_id, 0); // Not assigned!

    // 2. Bulk UP_PRIOR (requires eff >= 40):
    // b1, b2 (eff=55 >= 40) -> priority updated to 60!
    // b3, b4 (eff=25 < 40) -> SKIPPED!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&bug_arr={}&bug_arr={}&action=UP_PRIOR&priority=60",
                    b1, b2, b3, b4
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    assert_eq!(db.get_bug(b1).unwrap().unwrap().priority, 60);
    assert_eq!(db.get_bug(b2).unwrap().unwrap().priority, 60);
    assert_eq!(db.get_bug(b3).unwrap().unwrap().priority, 30); // unchanged!
    assert_eq!(db.get_bug(b4).unwrap().unwrap().priority, 30); // unchanged!

    // 3. Bulk CLOSE (requires eff >= 70):
    // mixed_user has max eff 55 < 70, all 4 must be SKIPPED!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&bug_arr={}&bug_arr={}&action=CLOSE",
                    b1, b2, b3, b4
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("0+issue%28s%29+updated%2C+4+skipped"));
    assert_ne!(db.get_bug(b1).unwrap().unwrap().status, 90);

    // 4. Bulk ADD_NOTE (requires eff >= 25):
    // Both Proj A (55) and Proj B (25) >= 25 -> All 4 updated!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&bug_arr={}&bug_arr={}&action=ADD_NOTE&bugnote_text=Bulk+Note+Added",
                    b1, b2, b3, b4
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("succeeded+on+4+issue%28s%29"));

    for b in [b1, b2, b3, b4] {
        let notes = db.list_bugnotes(b, 90).unwrap();
        assert!(notes.iter().any(|n| n.note == "Bulk Note Added"));
    }

    // 5. Bulk ATTACH_TAGS (requires eff >= 25):
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&action=ATTACH_TAGS&tag_string=bulk-tag-1%2C+bulk-tag-2",
                    b1, b3
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let t1_tags = db.list_bug_tags(b1).unwrap();
    assert!(t1_tags.iter().any(|t| t.tag_name == "bulk-tag-1"));
    assert!(t1_tags.iter().any(|t| t.tag_name == "bulk-tag-2"));

    // 6. Bulk ADD_NOTE with note_view_state=50 (Private note requires Developer eff >= 55):
    // b1 (Proj A, eff=55) -> private note succeeds!
    // b3 (Proj B, eff=25) -> private note skipped!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "bug_arr={}&bug_arr={}&action=ADD_NOTE&bugnote_text=Secret+Note&note_view_state=50",
                    b1, b3
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("1+issue%28s%29+updated%2C+1+skipped"));
    let b1_notes = db.list_bugnotes(b1, 90).unwrap();
    assert!(b1_notes
        .iter()
        .any(|n| n.note == "Secret Note" && n.view_state == 50));
    let b3_notes = db.list_bugnotes(b3, 90).unwrap();
    assert!(!b3_notes.iter().any(|n| n.note == "Secret Note"));

    // 7. Unknown action fails validation and redirects with err:
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("bug_arr={}&action=UNKNOWN_ACTION", b1)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("err=Please+select+a+valid+action"));
}

#[tokio::test]
async fn test_no_nested_forms_on_html_endpoints() {
    let db = Db::open_in_memory().unwrap();
    let p = db
        .create_project("Form Check Proj", 10, 10, "", true)
        .unwrap();
    let u = db
        .create_user("admin_form", "Admin", "af@test.com", "pw", 90, true, false)
        .unwrap();
    let b = db
        .create_bug(
            p,
            u,
            1,
            "Form Test Bug",
            "Desc",
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

    let app = build_router(db.clone());
    let cookie = login_user(&app, "admin_form", "pw").await;
    let full_cookie = set_project_cookie(&app, &cookie, p).await;

    // 1. /view_all_bug_page
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
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&body);

    // 2. /bug_clone_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_clone_page/{}", b))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&body);

    // 3. /bug_move_page/:id
    // Need a second project so move page has a valid target
    let _p2 = db.create_project("Second Proj", 10, 10, "", true).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_move_page/{}", b))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&body);
}
