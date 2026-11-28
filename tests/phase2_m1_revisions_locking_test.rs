use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db, models::RevisionType};
use tower::ServiceExt;

// ─── Helpers ─────────────────────────────────────────────────────────────────

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

async fn response_body_string(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn assert_no_nested_forms(html: &str) {
    let mut depth = 0;
    let mut rest = html;
    while !rest.is_empty() {
        let open_pos = rest.find("<form");
        let close_pos = rest.find("</form");
        match (open_pos, close_pos) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (Some(_), Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (Some(o), None) => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (None, Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (None, None) => break,
        }
    }
}

// ─── Integration Test ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_phase2_m1_revisions_locking_end_to_end() {
    let db = Db::open_in_memory().unwrap();

    // 1. Verify Idempotent Migration
    assert!(db.migrate().is_ok());
    assert!(db.migrate().is_ok());

    // Setup project and users across all access levels
    let proj_id = db
        .create_project("Revisions & Locking Project", 10, 10, "Test project", true)
        .unwrap();

    let _admin_id = db
        .create_user(
            "admin_user",
            "Admin",
            "admin@test.com",
            "root",
            90,
            true,
            false,
        )
        .unwrap();
    let mgr_id = db
        .create_user(
            "mgr_user",
            "Manager",
            "mgr@test.com",
            "pass",
            70,
            true,
            false,
        )
        .unwrap();
    let _dev_id = db
        .create_user(
            "dev_user",
            "Developer",
            "dev@test.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let rep_id = db
        .create_user(
            "rep_user",
            "Reporter",
            "rep@test.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let _viewer_id = db
        .create_user(
            "viewer_user",
            "Viewer",
            "viewer@test.com",
            "pass",
            10,
            true,
            false,
        )
        .unwrap();

    let app = build_router(db.clone());

    let admin_cookie =
        set_project_cookie(&app, &login_as(&app, "admin_user", "root").await, proj_id).await;
    let mgr_cookie =
        set_project_cookie(&app, &login_as(&app, "mgr_user", "pass").await, proj_id).await;
    let dev_cookie =
        set_project_cookie(&app, &login_as(&app, "dev_user", "pass").await, proj_id).await;
    let rep_cookie =
        set_project_cookie(&app, &login_as(&app, "rep_user", "pass").await, proj_id).await;
    let viewer_cookie =
        set_project_cookie(&app, &login_as(&app, "viewer_user", "pass").await, proj_id).await;

    // Create a public test bug
    let bug_id = db
        .create_bug(
            proj_id,
            rep_id,
            1,
            "Initial Summary",
            "Initial Description",
            "Initial Steps",
            "Initial Additional Info",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10, // public
        )
        .unwrap();

    // ─── 2. Bugnote Edit & Revision Creation ─────────────────────────────────

    // Add note as reporter
    let note_id = db
        .add_bugnote(bug_id, rep_id, "First Note Content", 10)
        .unwrap();

    // GET /bugnote_edit_page/:id as note owner (Reporter)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bugnote_edit_page/{}", note_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let edit_page_html = response_body_string(res).await;
    assert!(edit_page_html.contains(r#"action="/bugnote_update/"#));
    assert!(edit_page_html.contains("First Note Content"));
    assert_no_nested_forms(&edit_page_html);

    // Non-owner reporter cannot edit other's note
    let _other_rep_id = db
        .create_user(
            "other_rep",
            "Other Rep",
            "other@test.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let other_rep_cookie =
        set_project_cookie(&app, &login_as(&app, "other_rep", "pass").await, proj_id).await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bugnote_edit_page/{}", note_id))
                .header("cookie", &other_rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // POST /bugnote_update/:id as reporter updates note and creates revision
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_update/{}", note_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("note=Updated+Note+Content"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify note is updated in DB
    let updated_note = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(updated_note.note, "Updated Note Content");

    // Verify revision was saved in mantis_bug_revision_table with previous content
    let note_revs = db.list_bug_revisions(bug_id, Some(note_id)).unwrap();
    assert_eq!(note_revs.len(), 1);
    assert_eq!(note_revs[0].bugnote_id, note_id);
    assert_eq!(note_revs[0].rev_type, RevisionType::Bugnote as i32);
    assert_eq!(note_revs[0].value, "First Note Content");

    // ─── 3. Bugnote View-State Toggle & Role Guards (403 for Reporter) ───────

    // Reporter cannot toggle view-state -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_set_view_state/{}", note_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("view_state=50"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Developer CAN toggle view-state to private (50)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_set_view_state/{}", note_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("view_state=50"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let priv_note = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(priv_note.view_state, 50);

    // Edit private note as Developer -> creates a 2nd revision
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_update/{}", note_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("note=Private+Secret+Note+V3"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // ─── 4. Private Bugnote Revision Hiding from Reporter/Viewer ─────────────

    // Developer / Manager can view revisions for private bugnote
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/bug_revision_view_page/{}?bugnote_id={}",
                    bug_id, note_id
                ))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dev_rev_html = response_body_string(res).await;
    assert!(dev_rev_html.contains("First Note Content"));
    assert_no_nested_forms(&dev_rev_html);

    // Reporter requesting revisions for private note directly gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/bug_revision_view_page/{}?bugnote_id={}",
                    bug_id, note_id
                ))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Viewer requesting revisions for private note directly gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/bug_revision_view_page/{}?bugnote_id={}",
                    bug_id, note_id
                ))
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Reporter viewing all revisions for bug does NOT see private note revisions
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_revision_view_page/{}", bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rep_all_rev_html = response_body_string(res).await;
    assert!(!rep_all_rev_html.contains("Private Secret Note"));
    assert!(!rep_all_rev_html.contains("First Note Content"));

    // S1 Regression test: create a private note, edit it to create revision, then delete note.
    let priv_note_id = db
        .add_bugnote(bug_id, mgr_id, "Very Secret Private Note Initial", 50)
        .unwrap();
    db.update_bugnote(priv_note_id, mgr_id, "Very Secret Private Note Modified")
        .unwrap();
    // Delete note
    db.delete_bugnote(priv_note_id).unwrap();

    // Reporter viewing all revisions for bug must STILL NOT see "Very Secret Private Note Initial"
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_revision_view_page/{}", bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rep_rev_after_del = response_body_string(res).await;
    assert!(!rep_rev_after_del.contains("Very Secret Private Note Initial"));

    // Toggle note back to public via toggle (no body)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_set_view_state/{}", note_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let pub_again_note = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(pub_again_note.view_state, 10);

    // ─── 5. Bug Description/Steps Revision Tracking & Drop (403 Dev, 303 Mgr) ─

    // Update bug description and steps
    let update_body = "summary=Initial+Summary&category_id=1&priority=30&severity=50&reproducibility=70&description=Updated+Description+V2&steps_to_reproduce=Updated+Steps+V2";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Check revisions in DB
    let desc_rev = db
        .list_bug_revisions(bug_id, None)
        .unwrap()
        .into_iter()
        .find(|r| r.rev_type == RevisionType::Description as i32)
        .unwrap();
    assert_eq!(desc_rev.value, "Initial Description");

    // Developer attempting to drop revision gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_revision_drop/{}", desc_rev.id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Manager dropping revision succeeds (303 See Other)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_revision_drop/{}", desc_rev.id))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify dropped revision is gone
    assert!(db.get_bug_revision_by_id(desc_rev.id).unwrap().is_none());

    // ─── 6. Optimistic Locking (409 Conflict on Stale last_updated) ──────────

    let bug_current = db.get_bug(bug_id).unwrap().unwrap();
    let current_lu = bug_current.last_updated;
    let stale_lu = current_lu - 1000;

    let stale_update_body = format!(
        "summary=Stale+Attempt&category_id=1&priority=30&severity=50&reproducibility=70&last_updated={}",
        stale_lu
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(stale_update_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let conflict_html = response_body_string(res).await;
    assert!(conflict_html.contains("Conflict"));
    assert!(conflict_html.contains("modified by another user or session"));

    // DB row was NOT overwritten
    let bug_unchanged = db.get_bug(bug_id).unwrap().unwrap();
    assert_ne!(bug_unchanged.summary, "Stale Attempt");
    assert_eq!(bug_unchanged.summary, "Initial Summary");

    // Successful update with matching last_updated
    let matching_update_body = format!(
        "summary=Valid+Optimistic+Update&category_id=1&priority=30&severity=50&reproducibility=70&last_updated={}",
        current_lu
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(matching_update_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_valid = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug_valid.summary, "Valid Optimistic Update");

    // ─── 7. Backward Compatibility Without last_updated ─────────────────────

    let backward_compat_body =
        "summary=Backward+Compat+Update&category_id=1&priority=30&severity=50&reproducibility=70";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(backward_compat_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_bc = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug_bc.summary, "Backward Compat Update");

    // Backward compat with empty last_updated=
    let empty_lu_body = "summary=Empty+LU+Update&category_id=1&priority=30&severity=50&reproducibility=70&last_updated=";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(empty_lu_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_empty_lu = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug_empty_lu.summary, "Empty LU Update");

    // Malformed last_updated returns 400 Bad Request
    let malformed_lu_body = "summary=Malformed+LU&category_id=1&priority=30&severity=50&reproducibility=70&last_updated=invalid_epoch";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(malformed_lu_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // ─── 8. Form Validation & No Nested Forms Gate ───────────────────────────

    // GET /view_bug_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let view_html = response_body_string(res).await;
    assert_no_nested_forms(&view_html);
    assert!(view_html.contains(r#"href="/bug_revision_view_page/"#));
    assert!(view_html.contains("Revisions"));

    // GET /update_bug_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/update_bug_page/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let update_html = response_body_string(res).await;
    assert_no_nested_forms(&update_html);
    assert!(update_html.contains(r#"<input type="hidden" name="last_updated""#));

    // GET /bug_revision_view_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_revision_view_page/{}", bug_id))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rev_page_html = response_body_string(res).await;
    assert_no_nested_forms(&rev_page_html);
    assert!(rev_page_html.contains("Revisions"));

    // GET /bug_revision_view_page/:id?err=... displays flash error banner
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/bug_revision_view_page/{}?err=CustomRevisionError",
                    bug_id
                ))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rev_page_err_html = response_body_string(res).await;
    assert!(rev_page_err_html.contains("CustomRevisionError"));
}
