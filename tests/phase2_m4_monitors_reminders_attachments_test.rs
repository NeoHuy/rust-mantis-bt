use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use std::path::PathBuf;
use tower::ServiceExt;

// ─── RAII Temp File Guard for SQLite Database ───────────────────────────────

struct TempDbGuard {
    path: PathBuf,
}

impl TempDbGuard {
    fn new(prefix: &str) -> Self {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("{}_{}_{}.db", prefix, pid, unique_id));
        Self { path }
    }

    fn path_str(&self) -> &str {
        self.path.to_str().expect("valid utf-8 path")
    }
}

impl Drop for TempDbGuard {
    fn drop(&mut self) {
        let base = self.path_str();
        let _ = std::fs::remove_file(base);
        let _ = std::fs::remove_file(format!("{}-wal", base));
        let _ = std::fs::remove_file(format!("{}-shm", base));
        let _ = std::fs::remove_file(format!("{}.db-wal", base));
        let _ = std::fs::remove_file(format!("{}.db-shm", base));
    }
}

// ─── Helper Functions ────────────────────────────────────────────────────────

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
    assert_eq!(
        res.status(),
        StatusCode::SEE_OTHER,
        "Login failed for user '{}'",
        username
    );
    res.headers()
        .get("set-cookie")
        .expect("login should set cookie")
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
    let mut depth: usize = 0;
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
                depth = depth.saturating_sub(1);
                rest = &rest[c + 6..];
            }
            (Some(o), None) => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (None, Some(c)) => {
                depth = depth.saturating_sub(1);
                rest = &rest[c + 6..];
            }
            (None, None) => break,
        }
    }
}

// ─── Test Suite 1: Multi-User Monitors ───────────────────────────────────────

#[tokio::test]
async fn test_multi_user_monitors_permissions_and_workflow() {
    let temp = TempDbGuard::new("m4_monitors");
    let db = Db::open(temp.path_str()).unwrap();

    // Create a project
    let pid = db
        .create_project("Monitor Project", 10, 10, "Monitor tests", true)
        .unwrap();

    // Create users with different access levels:
    // dev1 (Developer = 55), rep1 (Reporter = 25), view1 (Viewer = 10), disabled_u (disabled)
    let dev1_id = db
        .create_user(
            "dev1",
            "Developer One",
            "dev1@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let rep1_id = db
        .create_user(
            "rep1",
            "Reporter One",
            "rep1@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let view1_id = db
        .create_user(
            "view1",
            "Viewer One",
            "view1@example.com",
            "password",
            10,
            true,
            false,
        )
        .unwrap();
    let _disabled_id = db
        .create_user(
            "disabled_u",
            "Disabled User",
            "dis@example.com",
            "password",
            25,
            false,
            false,
        )
        .unwrap();

    // Create a public bug
    let bug_id = db
        .create_bug(
            pid,
            dev1_id,
            1,
            "Public Bug for Monitors",
            "Details",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10, // view_state = 10 (public)
        )
        .unwrap();

    let app = build_router(db.clone());
    let dev_cookie =
        set_project_cookie(&app, &login_user(&app, "dev1", "password").await, pid).await;
    let rep_cookie =
        set_project_cookie(&app, &login_user(&app, "rep1", "password").await, pid).await;

    // 1. Reporter attempting to add another user (view1) as monitor -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=view1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Reporter self-monitoring with empty body -> 303 See Other
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id, rep1_id);

    // Verify history recorded for self-monitor addition
    let hist = db.list_bug_history(bug_id).unwrap();
    let add_hist = hist.iter().find(|h| h.field_name == "monitor_added");
    assert!(add_hist.is_some());
    assert_eq!(add_hist.unwrap().new_value, "rep1");

    // 3. Developer adding another user (view1) by username -> 303 See Other
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=view1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert_eq!(monitors.len(), 2);
    // Ordered by username ASC: rep1, view1
    assert_eq!(monitors[0].username, "rep1");
    assert_eq!(monitors[1].username, "view1");

    // 4. Developer attempting to add disabled user -> redirects with error
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=disabled_u"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err="),
        "Expected error redirect for disabled user"
    );

    // 5. Developer attempting to add viewer to private issue -> redirects with error
    let priv_bug_id = db
        .create_bug(
            pid,
            dev1_id,
            1,
            "Private Bug",
            "Secret Details",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50, // view_state = 50 (private)
        )
        .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_add/{}", priv_bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=view1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err="),
        "Expected error redirect for user without private issue access"
    );

    // 6. Reporter attempting to remove another user (view1) -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_delete/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("user_id={}", view1_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 7. Developer deleting another user (view1) -> 303 See Other
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_delete/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("user_id={}", view1_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id, rep1_id);

    // Verify history recorded for monitor removal (type 13 / BUG_UNMONITOR)
    let hist = db.list_bug_history(bug_id).unwrap();
    let rem_hist = hist.iter().find(|h| h.field_name == "monitor_removed");
    assert!(rem_hist.is_some());
    let r_entry = rem_hist.unwrap();
    assert_eq!(r_entry.old_value, "view1");
    assert_eq!(r_entry.type_, 13);

    // 8. Reporter self-unmonitoring with empty body -> 303 See Other
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_monitor_delete/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert!(monitors.is_empty());

    // 9. Inspect view_bug_page markup:
    // Check "Users Monitoring This Issue" section, Add Monitor form, and no nested forms
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    assert!(html.contains("Users Monitoring This Issue"));
    assert!(html.contains("bug_monitor_add"));
    assert_no_nested_forms(&html);
}

// ─── Test Suite 2: Issue Reminders ───────────────────────────────────────────

#[tokio::test]
async fn test_issue_reminders_workflow_and_permissions() {
    let temp = TempDbGuard::new("m4_reminders");
    let db = Db::open(temp.path_str()).unwrap();

    let pid = db
        .create_project("Reminder Project", 10, 10, "Reminder tests", true)
        .unwrap();

    let dev1_id = db
        .create_user(
            "lead_dev",
            "Lead Developer",
            "lead@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let _rep1_id = db
        .create_user(
            "reporter1",
            "Normal Reporter",
            "rep1@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let dev2_id = db
        .create_user(
            "colleague_dev",
            "Colleague Developer",
            "colleague@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();

    let bug_id = db
        .create_bug(
            pid,
            dev1_id,
            1,
            "Urgent Production Outage",
            "Needs fast attention",
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
    let dev_cookie =
        set_project_cookie(&app, &login_user(&app, "lead_dev", "password").await, pid).await;
    let rep_cookie =
        set_project_cookie(&app, &login_user(&app, "reporter1", "password").await, pid).await;

    // 1. Reporter attempting to view reminder page -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/bug_reminder_page/{}", bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Reporter attempting to post reminder -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_reminder/{}", bug_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("to={}&body=Hello", dev2_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Developer viewing reminder page -> 200 OK with eligible recipients form
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/bug_reminder_page/{}", bug_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    assert!(html.contains("Send a Reminder"));
    assert!(html.contains("colleague_dev"));
    assert_no_nested_forms(&html);

    // Clear any previous queued emails before testing reminders
    {
        let conn = db.conn();
        conn.execute("DELETE FROM mantis_email_table", []).unwrap();
    }

    // 4. Developer posting reminder to colleague_dev
    let post_body = format!(
        "to={}&body=Please+check+the+database+replication+failure+now&store_as_note=1&add_monitors=1",
        dev2_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_reminder/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(post_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("msg="));

    // Database verification:
    // a. Reminder bugnote created with note_type == 1
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    let reminder_note = notes.iter().find(|n| n.note_type == 1);
    assert!(reminder_note.is_some(), "Expected note with note_type == 1");
    let rnote = reminder_note.unwrap();
    assert_eq!(rnote.reporter_id, dev1_id);
    assert!(rnote.note.contains("Please check the database replication"));
    assert!(rnote.note.contains("colleague_dev"));

    // b. Recipient added as issue monitor
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert!(monitors.iter().any(|m| m.id == dev2_id));

    // c. Queued reminder email in mantis_email_table: ONLY recipient receives email, NOT reporter
    {
        let conn = db.conn();
        let queued_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_email_table WHERE email = 'colleague@example.com' AND subject LIKE '%Reminder%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            queued_count, 1,
            "Expected exactly 1 queued reminder email for recipient"
        );

        let reporter_queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_email_table WHERE email = 'lead@example.com' AND subject LIKE '%Reminder%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            reporter_queued, 0,
            "Reporter must NOT receive reminder email meant for recipient"
        );
    }

    // 4b. Test unchecked checkboxes: store_as_note=0 and add_monitors=0
    let dev3_id = db
        .create_user(
            "another_dev",
            "Another Dev",
            "another@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();

    let notes_count_before = db.list_bugnotes(bug_id, 90).unwrap().len();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_reminder/{}", bug_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "to={}&body=Email+only+reminder&store_as_note=0&add_monitors=0",
                    dev3_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify: no new bugnote was created when store_as_note=0
    let notes_count_after = db.list_bugnotes(bug_id, 90).unwrap().len();
    assert_eq!(
        notes_count_after, notes_count_before,
        "Unchecking store_as_note must prevent bugnote creation"
    );

    // Verify: recipient was NOT added to monitors when add_monitors=0
    let monitors = db.list_bug_monitor_users(bug_id).unwrap();
    assert!(
        !monitors.iter().any(|m| m.id == dev3_id),
        "Unchecking add_monitors must not add recipient as monitor"
    );

    // 5. Inspect view_bug_page markup:
    // Should show "Send a Reminder" link in toolbar for Developer and Reminder badge on the note
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    assert!(html.contains("Send a Reminder"));
    assert!(html.contains("Reminder"));
    assert_no_nested_forms(&html);
}

// ─── Test Suite 3: Enhanced Attachments & Backward Compatibility ─────────────

#[tokio::test]
async fn test_attachments_guard_multi_file_and_inline_previews() {
    let temp = TempDbGuard::new("m4_attachments");
    let db = Db::open(temp.path_str()).unwrap();

    let pid = db
        .create_project("Attachment Project", 10, 10, "Upload tests", true)
        .unwrap();

    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let app = build_router(db.clone());
    let admin_cookie =
        set_project_cookie(&app, &login_user(&app, "administrator", "root").await, pid).await;

    // 1. Dangerous extension rejection on POST /bug_file_add/:bug_id
    let bug_id = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Attachment Testing Bug",
            "Details",
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

    let boundary = "---------------------------m4boundary123";
    let evil_payload = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"attack.svg\"\r\n\
         Content-Type: image/svg+xml\r\n\r\n\
         <svg onload=\"alert(1)\"></svg>\r\n\
         --{b}--\r\n",
        b = boundary
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(evil_payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err="),
        "Expected error redirect for .svg upload"
    );

    // Also test .exe
    let evil_exe = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"trojan.EXE\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n\
         MZ_FAKE_BINARY\r\n\
         --{b}--\r\n",
        b = boundary
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(evil_exe))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err="),
        "Expected error redirect for .exe upload"
    );

    // Also test trailing dot bypass attempt (e.g. attack.html.)
    let evil_dot = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"attack.html.\"\r\n\
         Content-Type: text/html\r\n\r\n\
         <html>evil</html>\r\n\
         --{b}--\r\n",
        b = boundary
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(evil_dot))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err="),
        "Expected error redirect for trailing dot attack.html."
    );

    // Ensure no files were added
    let files = db.list_bug_files(bug_id).unwrap();
    assert!(files.is_empty(), "Dangerous files must not be stored");

    // 2. Multi-file upload on POST /bug_file_add/:bug_id
    let multi_payload = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"syslog.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         Warning: high load detected on worker-01\r\n\
         --{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"diagram.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         FAKE_PNG_BINARY_BYTES\r\n\
         --{b}--\r\n",
        b = boundary
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(multi_payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("msg="));

    let files = db.list_bug_files(bug_id).unwrap();
    assert_eq!(files.len(), 2, "Expected 2 uploaded files");
    let txt_file = files.iter().find(|f| f.filename == "syslog.txt").unwrap();
    let png_file = files.iter().find(|f| f.filename == "diagram.png").unwrap();

    // 2b. Test upload with > 10 files: must reject with error rather than silently dropping files
    let mut eleven_files = String::new();
    for i in 1..=11 {
        eleven_files.push_str(&format!(
            "--{b}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"file{i}.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             content {i}\r\n",
            b = boundary,
            i = i
        ));
    }
    eleven_files.push_str(&format!("--{}--\r\n", boundary));
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(eleven_files))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("err=") && loc.contains("Maximum"),
        "Must report error when uploading more than 10 files"
    );

    // 2c. Test upload of a 0-byte file: should be preserved and not discarded
    let zero_byte_payload = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"empty_log.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         --{b}--\r\n",
        b = boundary
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_file_add/{}", bug_id))
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(zero_byte_payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let updated_files = db.list_bug_files(bug_id).unwrap();
    let zero_file = updated_files.iter().find(|f| f.filename == "empty_log.txt");
    assert!(
        zero_file.is_some(),
        "0-byte file with valid filename must be stored"
    );
    assert_eq!(zero_file.unwrap().filesize, 0);

    // 3. Safe inline image previews vs forced attachment
    // a. GET png file with ?inline=1 -> Content-Disposition: inline + nosniff
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/file/bug/{}/diagram.png?inline=1", png_file.id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let disp = res
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        disp.starts_with("inline;"),
        "PNG with inline=1 must be inline"
    );
    let nosniff = res
        .headers()
        .get("x-content-type-options")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(nosniff, "nosniff");

    // b. GET png file WITHOUT ?inline=1 -> Content-Disposition: attachment
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/file/bug/{}/diagram.png", png_file.id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let disp = res
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        disp.starts_with("attachment;"),
        "PNG without inline=1 must default to attachment"
    );

    // c. GET text file WITH ?inline=1 -> MUST STILL BE attachment (never inline non-images!)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/file/bug/{}/syslog.txt?inline=1", txt_file.id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let disp = res
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        disp.starts_with("attachment;"),
        "Text file with inline=1 must be forced to attachment"
    );

    // 4. Initial attachment upload on POST /bug_report (multipart)
    let report_multipart = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"summary\"\r\n\r\n\
         Issue with Initial Attachments\r\n\
         --{b}\r\n\
         Content-Disposition: form-data; name=\"description\"\r\n\r\n\
         Testing initial upload on report\r\n\
         --{b}\r\n\
         Content-Disposition: form-data; name=\"category_id\"\r\n\r\n\
         1\r\n\
         --{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"report_crash.log\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         Stacktrace line 42: NullPointerException\r\n\
         --{b}--\r\n",
        b = boundary
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &admin_cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(report_multipart))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.starts_with("/view_bug_page/"));

    // Extract created bug ID from redirect URL
    let new_bug_id: i64 = loc
        .trim_start_matches("/view_bug_page/")
        .split('?')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    let new_files = db.list_bug_files(new_bug_id).unwrap();
    assert_eq!(new_files.len(), 1);
    assert_eq!(new_files[0].filename, "report_crash.log");

    // 5. Backward Compatibility: Standard urlencoded POST /bug_report
    let urlencoded_body =
        "summary=Urlencoded+Report&description=Tested+via+urlencoded+form&category_id=1";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(urlencoded_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.starts_with("/view_bug_page/"));

    // 6. Inspect view_bug_page and bug_report_page for previews, dropzone, and form tags
    // a. Check view_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    // Thumbnail preview for PNG
    assert!(html.contains("inline=1"));
    assert!(html.contains("<img"));
    // Collapsible text preview for syslog.txt
    assert!(html.contains("<details"));
    assert!(html.contains("Preview"));
    assert!(html.contains("Warning: high load detected on worker-01"));
    // Multi-file input
    assert!(html.contains("multiple"));
    assert_no_nested_forms(&html);

    // b. Check bug_report_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/bug_report_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    assert!(html.contains("enctype=\"multipart/form-data\""));
    assert!(html.contains("multiple"));
    assert_no_nested_forms(&html);
}
