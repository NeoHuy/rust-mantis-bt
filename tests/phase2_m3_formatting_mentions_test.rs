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

#[tokio::test]
async fn test_phase2_m3_core_formatting_mentions_end_to_end() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Setup Projects and Users
    let pub_proj_id = db
        .create_project("PublicProject", 10, 10, "Public project", true)
        .unwrap();
    let priv_proj_id = db
        .create_project("SecretProject", 50, 10, "Private project", true)
        .unwrap();

    // Developer bob (eff >= 55 on projects)
    let _dev_id = db
        .create_user(
            "developer_bob",
            "Bob Developer",
            "bob@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();

    // Reporter alice (eff = 25)
    let _rep_id = db
        .create_user(
            "reporter_alice",
            "Alice Reporter",
            "alice@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();

    // Outsider charlie (Reporter = 25, no access to SecretProject)
    let _out_id = db
        .create_user(
            "outsider_charlie",
            "Charlie Outsider",
            "charlie@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();

    let admin_cookie = login_user(&app, "administrator", "root").await;
    let charlie_cookie = login_user(&app, "outsider_charlie", "password").await;

    // 2. Create Issues:
    // Bug 1: in PublicProject
    let bug1_id = db
        .create_bug(
            pub_proj_id,
            1, // admin
            1, // General
            "Public Issue 1",
            "Here is **bold markdown**, *italic*, and `inline code`.\nAlso check https://mantisbt.org for docs.",
            "Steps:\n- item 1\n- item 2",
            "Additional notes",
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

    // Bug 2: in SecretProject (view_state = 50 private)
    let bug2_id = db
        .create_bug(
            priv_proj_id,
            1, // admin
            1,
            "Confidential Vulnerability In Secret Engine",
            "Secret exploit details",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50, // private
        )
        .unwrap();

    // Bug 3: in PublicProject, referencing Bug 1 and Bug 2
    let bug3_desc = format!("Cross-references: see #{} and see #{}.", bug1_id, bug2_id);
    let bug3_id = db
        .create_bug(
            pub_proj_id,
            1,
            1,
            "Cross Reference Bug",
            &bug3_desc,
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

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 1: Rendering when MantisCoreFormatting is ENABLED vs DISABLED
    // ─────────────────────────────────────────────────────────────────────────
    assert!(db.is_plugin_enabled("MantisCoreFormatting"));

    // Visit /view_bug_page/{bug1_id} with MantisCoreFormatting enabled
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug1_id))
                .header("cookie", &admin_cookie)
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

    assert!(
        html.contains(r#"<div class="formatted-text">"#),
        "Formatted text container missing"
    );
    assert!(html.contains("<strong>bold markdown</strong>"));
    assert!(html.contains("<em>italic</em>"));
    assert!(html.contains(r#"<code class="inline-code">inline code</code>"#));
    assert!(html.contains(r#"<a href="https://mantisbt.org" target="_blank" rel="noopener noreferrer nofollow">https://mantisbt.org</a>"#));
    assert!(html.contains("<ul><li>item 1</li><li>item 2</li></ul>"));

    // Toggle plugin OFF via /manage_plugin_toggle/MantisCoreFormatting
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_plugin_toggle/MantisCoreFormatting")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("enabled=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(!db.is_plugin_enabled("MantisCoreFormatting"));

    // Visit /view_bug_page/{bug1_id} when disabled -> plain text, no markdown tags
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug1_id))
                .header("cookie", &admin_cookie)
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

    assert!(
        !html.contains(r#"<div class="formatted-text">"#),
        "Formatted container must not appear when plugin is disabled"
    );
    assert!(
        !html.contains("<strong>bold markdown</strong>"),
        "Markdown tags must not render when plugin is disabled"
    );
    assert!(
        !html.contains("<a href=\"https://mantisbt.org\""),
        "URLs must not be auto-linked when plugin is disabled"
    );
    assert!(html.contains("**bold markdown**"));

    // Re-enable MantisCoreFormatting
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_plugin_toggle/MantisCoreFormatting")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("enabled=1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.is_plugin_enabled("MantisCoreFormatting"));

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 2: Non-leakage of private issue summaries in #123 links
    // ─────────────────────────────────────────────────────────────────────────

    // As Admin: both Bug 1 and Bug 2 are visible
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug3_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html_admin = String::from_utf8_lossy(&body);

    // Admin can see links to both bug 1 and bug 2 with their summaries
    assert!(html_admin.contains(&format!("/view_bug_page/{}", bug1_id)));
    assert!(html_admin.contains(&format!("/view_bug_page/{}", bug2_id)));
    assert!(html_admin.contains("Public Issue 1"));
    assert!(html_admin.contains("Confidential Vulnerability In Secret Engine"));

    // As Charlie (Reporter without access to SecretProject):
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug3_id))
                .header("cookie", &charlie_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html_charlie = String::from_utf8_lossy(&body);

    // Charlie CAN see the link to public Bug 1
    assert!(html_charlie.contains(&format!("/view_bug_page/{}", bug1_id)));
    assert!(html_charlie.contains("Public Issue 1"));

    // CRITICAL: Charlie MUST NOT see link or summary for private Bug 2
    assert!(
        !html_charlie.contains(&format!("/view_bug_page/{}", bug2_id)),
        "Private bug URL must not leak to unauthorized user"
    );
    assert!(
        !html_charlie.contains("Confidential Vulnerability In Secret Engine"),
        "Private bug summary must not leak to unauthorized user in title"
    );
    assert!(
        html_charlie.contains(&format!("#{}", bug2_id)),
        "Private bug reference should remain as plain text '#{}'",
        bug2_id
    );

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 3: @mentions and Email Queueing with Access Control
    // ─────────────────────────────────────────────────────────────────────────
    db.flush_email_queue().unwrap();
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);

    // 3a. Public Bugnote with mentions for developer_bob and reporter_alice
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Public+review+needed+by+@developer_bob+and+@reporter_alice&view_state=10",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let emails = db.list_queued_emails().unwrap();
    let email_recipients: Vec<String> = emails.iter().map(|e| e.email.clone()).collect();

    // Both bob and alice should have been queued emails (via mention, without duplicate note emails)
    assert!(
        email_recipients.contains(&"bob@example.com".to_string()),
        "developer_bob should receive email"
    );
    assert!(
        email_recipients.contains(&"alice@example.com".to_string()),
        "reporter_alice should receive email"
    );
    assert_eq!(
        emails
            .iter()
            .filter(|e| e.email == "bob@example.com")
            .count(),
        1,
        "developer_bob should receive exactly 1 email (no duplicate)"
    );
    assert_eq!(
        emails
            .iter()
            .filter(|e| e.email == "alice@example.com")
            .count(),
        1,
        "reporter_alice should receive exactly 1 email (no duplicate)"
    );

    // Verify mention email format
    let bob_email = emails
        .iter()
        .find(|e| e.email == "bob@example.com" && e.subject.contains("Mentioned by"))
        .expect("developer_bob should receive a distinct mention email");
    assert!(bob_email.subject.contains("administrator"));
    assert!(bob_email.body.contains("Public review needed"));

    // Verify rendered bugnote has anchor id="c{note_id}"
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html_notes = String::from_utf8_lossy(&body);
    assert!(
        html_notes.contains(r#"<tr id="c"#),
        "Bugnote row must have an id='c...' anchor"
    );
    assert!(
        html_notes.contains(r#"<span class="mention">@developer_bob</span>"#),
        "Mention must be highlighted"
    );
    assert!(
        html_notes.contains(r#"<span class="mention">@reporter_alice</span>"#),
        "Mention must be highlighted"
    );

    // 3b. Private Bugnote: only developer_bob (eff >= 55) should receive mention email, NOT reporter_alice (eff = 25)
    db.flush_email_queue().unwrap();
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Private+internal+note+cc+@developer_bob+and+@reporter_alice&view_state=50",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let emails = db.list_queued_emails().unwrap();
    let priv_recipients: Vec<String> = emails.iter().map(|e| e.email.clone()).collect();

    assert!(
        priv_recipients.contains(&"bob@example.com".to_string()),
        "developer_bob (Developer access) must receive private note email"
    );
    assert!(
        !priv_recipients.contains(&"alice@example.com".to_string()),
        "reporter_alice (Reporter access) MUST NOT receive private note email"
    );

    // 3c. Mention on private issue: outsider_charlie has no access, must NOT receive email
    db.flush_email_queue().unwrap();
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", bug2_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Secret+issue+note+cc+@developer_bob+and+@outsider_charlie&view_state=10",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let emails = db.list_queued_emails().unwrap();
    let secret_recipients: Vec<String> = emails.iter().map(|e| e.email.clone()).collect();

    assert!(
        secret_recipients.contains(&"bob@example.com".to_string()),
        "developer_bob (Developer access) receives email on secret issue"
    );
    assert!(
        !secret_recipients.contains(&"charlie@example.com".to_string()),
        "outsider_charlie (no access to SecretProject) MUST NOT receive email"
    );

    // 3d. Update issue: only newly added mentions receive email (prevents spamming existing mentions)
    db.flush_email_queue().unwrap();
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);

    // Initial update adding @developer_bob
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "category=General&severity=50&priority=40&summary=Updated+Public+Issue\
                     &description=Updated+desc+with+@developer_bob\
                     &steps_to_reproduce=\
                     &additional_information=",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // developer_bob was newly mentioned -> 1 email queued
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].email, "bob@example.com");

    db.flush_email_queue().unwrap();

    // Second update: change priority without adding any new mentions
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "category=General&severity=50&priority=50&summary=Updated+Priority+Issue\
                     &description=Updated+desc+with+@developer_bob\
                     &steps_to_reproduce=\
                     &additional_information=",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // No newly added mentions -> 0 emails queued!
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(
        emails.len(),
        0,
        "No email should be queued when updating without adding new mentions"
    );

    // Third update: add @reporter_alice to steps_to_reproduce while keeping @developer_bob
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "category=General&severity=50&priority=50&summary=Updated+Priority+Issue\
                     &description=Updated+desc+with+@developer_bob\
                     &steps_to_reproduce=New+step+calling+@reporter_alice\
                     &additional_information=",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let emails = db.list_queued_emails().unwrap();
    let update_recipients: Vec<String> = emails.iter().map(|e| e.email.clone()).collect();
    assert!(
        update_recipients.contains(&"alice@example.com".to_string()),
        "newly mentioned @reporter_alice must receive mention email on update"
    );
    assert!(
        !update_recipients.contains(&"bob@example.com".to_string()),
        "previously mentioned @developer_bob MUST NOT be re-notified"
    );

    // 6. Verification: Reporter does NOT see link to private note (~note_id), while Developer DOES
    let priv_note_id = db
        .add_bugnote(bug1_id, 1, "Top secret note content", 50)
        .unwrap();
    let _ = db
        .add_bugnote(
            bug1_id,
            1,
            &format!("Cross reference to ~{}", priv_note_id),
            10,
        )
        .unwrap();

    let alice_cookie = login_user(&app, "reporter_alice", "password").await;
    let bob_cookie = login_user(&app, "developer_bob", "password").await;

    // View as developer_bob (access >= 55)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug1_id))
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_dev = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html_dev = String::from_utf8_lossy(&body_dev);
    assert!(
        html_dev.contains(&format!("/view_bug_page/{}#c{}", bug1_id, priv_note_id)),
        "Developer MUST see clickable ~note_id link to private note"
    );

    // View as reporter_alice (access = 25 < 55)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug1_id))
                .header("cookie", &alice_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_rep = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html_rep = String::from_utf8_lossy(&body_rep);
    assert!(
        !html_rep.contains(&format!("/view_bug_page/{}#c{}", bug1_id, priv_note_id)),
        "Reporter MUST NOT see clickable link to private note anchor"
    );
    assert!(
        html_rep.contains(&format!("~{}", priv_note_id)),
        "Reporter should see plain unlinked ~note_id text"
    );

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 4: Manage Plugins UI toggle for MantisCoreFormatting
    // ─────────────────────────────────────────────────────────────────────────
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_plugin_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let plugin_html = String::from_utf8_lossy(&body);
    assert!(
        plugin_html.contains(r#"action="/manage_plugin_toggle/MantisCoreFormatting""#),
        "Manage Plugins UI must render toggle button for MantisCoreFormatting"
    );

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 5: Bulk ADD_NOTE @mentions & deduplication
    // ─────────────────────────────────────────────────────────────────────────
    db.flush_email_queue().unwrap();
    // Assign developer_bob as handler on bug1_id so standard note notification would target him too
    db.assign_bug(bug1_id, 1, _dev_id).unwrap();
    db.flush_email_queue().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_actiongroup")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "action=ADD_NOTE&bug_arr={}&note_view_state=10&bugnote_text=Bulk+ping+@developer_bob",
                    bug1_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let bulk_emails = db.list_queued_emails().unwrap();
    let bob_bulk_emails: Vec<_> = bulk_emails
        .iter()
        .filter(|e| e.email == "bob@example.com")
        .collect();
    assert_eq!(
        bob_bulk_emails.len(),
        1,
        "Bulk ADD_NOTE must send exactly 1 mention email to @developer_bob without duplicate note email"
    );
    assert!(bob_bulk_emails[0].subject.contains("Mentioned by"));

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 6: bugnote_update newly-added @mentions & private note RBAC
    // ─────────────────────────────────────────────────────────────────────────
    // Create a private note by admin mentioning @developer_bob
    let edit_note_id = db
        .add_bugnote(bug1_id, 1, "Initial private note @developer_bob", 50)
        .unwrap();
    // Reporter alice (access 25 < 55) MUST NOT be able to edit private note even if she tries
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_update/{}", edit_note_id))
                .header("cookie", &alice_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("note=Hacked+private+note"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Public note edited by admin: keeps @developer_bob, newly adds @reporter_alice
    let pub_edit_note_id = db
        .add_bugnote(bug1_id, 1, "Initial public note @developer_bob", 10)
        .unwrap();
    db.flush_email_queue().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_update/{}", pub_edit_note_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Edited+public+note+@developer_bob+and+@reporter_alice",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let edit_emails = db.list_queued_emails().unwrap();
    assert_eq!(
        edit_emails.len(),
        1,
        "Only newly mentioned @reporter_alice should be notified on bugnote_update"
    );
    assert_eq!(edit_emails[0].email, "alice@example.com");
    assert!(edit_emails[0].subject.contains("Mentioned by"));

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 7: bug_change_status with optional note @mention (no duplicate status email)
    // ─────────────────────────────────────────────────────────────────────────
    db.flush_email_queue().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug1_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "new_status=80&resolution=20&note=Resolved+for+@developer_bob",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let status_emails = db.list_queued_emails().unwrap();
    let bob_status_emails: Vec<_> = status_emails
        .iter()
        .filter(|e| e.email == "bob@example.com")
        .collect();
    assert_eq!(
        bob_status_emails.len(),
        1,
        "Mentioned handler bob should receive 1 mention email and NO duplicate status email"
    );
    assert!(bob_status_emails[0].subject.contains("Mentioned by"));

    // ─────────────────────────────────────────────────────────────────────────
    // TEST PART 8: REST API @mentions (POST /issues, PATCH /issues/:id + status, POST /notes)
    // ─────────────────────────────────────────────────────────────────────────
    let (_tok_id, raw_token) = db.create_api_token(1, "rest-mention-test").unwrap();
    let bearer = format!("Bearer {}", raw_token);

    // 8a. POST /api/rest/issues with @mention
    db.flush_email_queue().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rest/issues")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"summary":"REST issue","description":"Hello @developer_bob","project":{{"id":{}}}}}"#,
                    pub_proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rest_body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let rest_json: serde_json::Value = serde_json::from_slice(&rest_body).unwrap();
    let rest_bug_id = rest_json["issue"]["id"].as_i64().unwrap();

    let create_emails = db.list_queued_emails().unwrap();
    assert!(create_emails
        .iter()
        .any(|e| e.email == "bob@example.com" && e.subject.contains("Mentioned by")));

    // 8b. PATCH /api/rest/issues/:id combining newly mentioned @reporter_alice + status change
    // First subscribe alice as monitor on rest_bug_id so she would normally get the status email too
    db.add_bug_monitor(rest_bug_id, _rep_id).unwrap();
    db.flush_email_queue().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/rest/issues/{}", rest_bug_id))
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"description":"Updated for @developer_bob and @reporter_alice","status":{"id":50}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let patch_emails = db.list_queued_emails().unwrap();
    let alice_patch_emails: Vec<_> = patch_emails
        .iter()
        .filter(|e| e.email == "alice@example.com")
        .collect();
    assert_eq!(
        alice_patch_emails.len(),
        1,
        "Newly mentioned monitor @reporter_alice must receive 1 mention email and NO duplicate status email on combined PATCH"
    );
    assert!(alice_patch_emails[0].subject.contains("Mentioned by"));
    assert!(
        !patch_emails.iter().any(|e| e.email == "bob@example.com"),
        "Previously mentioned @developer_bob must not be re-notified"
    );

    // 8c. POST /api/rest/issues/:id/notes with @mention
    db.flush_email_queue().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rest/issues/{}/notes", rest_bug_id))
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"text":"REST note for @developer_bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let note_emails = db.list_queued_emails().unwrap();
    let bob_note_emails: Vec<_> = note_emails
        .iter()
        .filter(|e| e.email == "bob@example.com")
        .collect();
    assert_eq!(
        bob_note_emails.len(),
        1,
        "Mentioned @developer_bob should receive exactly 1 mention email on REST add_note"
    );
    assert!(bob_note_emails[0].subject.contains("Mentioned by"));
}
