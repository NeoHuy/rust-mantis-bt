use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use rusqlite::params;
use rust_mantis_bt::{
    build_router,
    db::Db,
    models::{filter::sort_bugs, BugFilter, CustomFieldType},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

// ─── Test Helpers ────────────────────────────────────────────────────────────

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
    assert_eq!(depth, 0, "Unclosed <form> tag in HTML");
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. File-Backed Legacy SQLite & Idempotent Db::migrate() Regression
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_backed_legacy_sqlite_upgrade_idempotency_and_data_integrity() {
    let guard = TempDbGuard::new("compat_legacy_sqlite");

    // 1. Simulate an older database file created by an older version of the schema
    // Table mantis_jira_epics_cache_table is created WITHOUT the `expires_at` column.
    {
        let raw_conn = rusqlite::Connection::open(guard.path_str()).unwrap();
        raw_conn
            .execute_batch(
                "CREATE TABLE mantis_jira_epics_cache_table (
                    project_key TEXT PRIMARY KEY,
                    epics_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                INSERT INTO mantis_jira_epics_cache_table (project_key, epics_json, updated_at)
                VALUES ('LEGACY_PROJ', '{\"epics\":[{\"key\":\"LEG-1\"}]}', 1600000000);",
            )
            .unwrap();
    }

    // 2. Open via Db::open: Db::migrate() must detect missing column and add expires_at
    // without corrupting or dropping existing rows.
    let db = Db::open(guard.path_str()).unwrap();

    // Verify epic cache table has expires_at and retained existing row
    {
        let conn = rusqlite::Connection::open(guard.path_str()).unwrap();
        let (key, json_val, updated_at, expires_at): (String, String, i64, i64) = conn
            .query_row(
                "SELECT project_key, epics_json, updated_at, expires_at FROM mantis_jira_epics_cache_table WHERE project_key = 'LEGACY_PROJ'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(key, "LEGACY_PROJ");
        assert!(json_val.contains("LEG-1"));
        assert_eq!(updated_at, 1600000000);
        assert_eq!(expires_at, 0); // default value added by migration
    }

    // 3. Seed realistic legacy records across all core tables
    let rep_id = db
        .create_user(
            "leg_reporter",
            "Legacy Reporter",
            "rep@legacy.local",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let dev_id = db
        .create_user(
            "leg_developer",
            "Legacy Developer",
            "dev@legacy.local",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();

    let pub_proj_id = db
        .create_project("LegacyPubProject", 10, 10, "Legacy Public Proj", true)
        .unwrap();
    let priv_proj_id = db
        .create_project("LegacyPrivProject", 10, 50, "Legacy Private Proj", true)
        .unwrap();

    let cat_id = db.add_category(pub_proj_id, "LegacyGeneral", 0).unwrap();
    let ver_id = db
        .add_version(pub_proj_id, "v1.0.0", "Initial Release", true, false)
        .unwrap();

    // Insert bug with legacy column defaults (empty strings, 0 due_date, 0 sticky)
    let bug_id = {
        let conn = rusqlite::Connection::open(guard.path_str()).unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_table (
                project_id, reporter_id, handler_id, duplicate_id, priority, severity,
                reproducibility, status, resolution, category_id, date_submitted, last_updated,
                os, os_build, platform, version, fixed_in_version, target_version, build,
                summary, view_state, sticky, due_date
            ) VALUES (?1, ?2, ?3, 0, 30, 50, 70, 10, 10, ?4, 1600000000, 1600000000,
                      '', '', '', 'v1.0.0', '', '', '', 'Legacy Bug Summary', 10, 0, 0)",
            params![pub_proj_id, rep_id, dev_id, cat_id],
        )
        .unwrap();
        let bug_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO mantis_bug_text_table (id, description, steps_to_reproduce, additional_information)
             VALUES (?1, 'Legacy description text', 'Legacy steps', 'Legacy notes')",
            params![bug_id],
        )
        .unwrap();

        // Seed bugnotes (one public, one private)
        conn.execute(
            "INSERT INTO mantis_bugnote_text_table (id, note) VALUES (101, 'Public legacy bugnote')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bugnote_table (bug_id, reporter_id, bugnote_text_id, view_state, date_submitted, last_modified)
             VALUES (?1, ?2, 101, 10, 1600000100, 1600000100)",
            params![bug_id, rep_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO mantis_bugnote_text_table (id, note) VALUES (102, 'Private legacy bugnote')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bugnote_table (bug_id, reporter_id, bugnote_text_id, view_state, date_submitted, last_modified)
             VALUES (?1, ?2, 102, 50, 1600000200, 1600000200)",
            params![bug_id, dev_id],
        )
        .unwrap();

        // Seed numeric mantis_bug_history_table rows
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (?1, ?2, 1600000050, 'status', '10', '50', 0)",
            params![dev_id, bug_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (?1, ?2, 1600000060, 'priority', '30', '40', 0)",
            params![dev_id, bug_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (?1, ?2, 1600000070, 'handler_id', '0', '1', 0)",
            params![dev_id, bug_id],
        )
        .unwrap();

        bug_id
    };

    // Tags, news, API tokens
    let tag_id = db
        .create_tag(rep_id, "legacy-regression-tag", "Tag description")
        .unwrap();
    db.attach_tag_to_bug(bug_id, tag_id, rep_id).unwrap();
    let news_id = db
        .create_news(
            pub_proj_id,
            rep_id,
            "Legacy News Headline",
            "Legacy news body text",
            true,
            10,
        )
        .unwrap();
    let (admin_tok_id, _admin_raw_tok) = db.create_api_token(1, "LegacyAdminToken").unwrap();

    drop(db);

    // 4. Re-open the same file with Db::open and call db.migrate() multiple times
    let db2 = Db::open(guard.path_str()).unwrap();
    for _ in 0..3 {
        db2.migrate().unwrap();
    }

    // 5. Assert zero rows lost or corrupted
    let full_bug = db2.get_bug_full(bug_id).unwrap().expect("bug must exist");
    assert_eq!(full_bug.bug.summary, "Legacy Bug Summary");
    assert_eq!(full_bug.bug.project_id, pub_proj_id);
    assert_eq!(full_bug.bug.reporter_id, rep_id);
    assert_eq!(full_bug.bug.handler_id, dev_id);
    assert_eq!(full_bug.bug.sticky, 0);
    assert_eq!(full_bug.bug.due_date, 0);
    assert_eq!(full_bug.bug.build, "");
    assert_eq!(full_bug.bug.os, "");
    assert_eq!(full_bug.description, "Legacy description text");

    let notes = db2.list_bugnotes(bug_id, 90).unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].note, "Public legacy bugnote");
    assert_eq!(notes[1].note, "Private legacy bugnote");

    let history = db2.list_bug_history(bug_id).unwrap();
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].field_name, "status");
    assert_eq!(history[0].old_value, "10");
    assert_eq!(history[0].new_value, "50");
    assert_eq!(history[1].field_name, "priority");
    assert_eq!(history[1].old_value, "30");
    assert_eq!(history[1].new_value, "40");
    assert_eq!(history[2].field_name, "handler_id");
    assert_eq!(history[2].old_value, "0");
    assert_eq!(history[2].new_value, "1");
    assert_eq!(history[3].field_name, "tag_attached");

    let tags = db2.list_tags().unwrap();
    assert!(tags
        .iter()
        .any(|t| t.id == tag_id && t.name == "legacy-regression-tag"));

    let news = db2.list_news(pub_proj_id, 90).unwrap();
    assert!(news
        .iter()
        .any(|n| n.id == news_id && n.headline == "Legacy News Headline"));

    let versions = db2.list_versions(pub_proj_id).unwrap();
    assert!(versions
        .iter()
        .any(|v| v.id == ver_id && v.version == "v1.0.0"));

    let user_priv = db2.get_project_by_id(priv_proj_id).unwrap();
    assert!(user_priv.is_some());

    let tokens = db2.list_api_tokens(1).unwrap();
    assert!(tokens
        .iter()
        .any(|t| t.id == admin_tok_id && t.name == "LegacyAdminToken"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Legacy Saved Filter JSON Compatibility & Milestone 1 Regressions
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_legacy_saved_filter_json_backward_compat() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("FilterCompatProj", 10, 10, "", true)
        .unwrap();
    let cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;

    let b1 = db
        .create_bug(
            proj_id,
            1,
            0,
            "Issue New Status 10",
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

    let b2 = db
        .create_bug(
            proj_id,
            1,
            0,
            "Issue Assigned Status 50",
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
    db.change_bug_status(b2, 1, 50, 10, None).unwrap();

    // 1. Insert minimal legacy BugFilter JSON (omits sort, dir, per_page, search, etc.)
    let filter1_id = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_filters_table (user_id, project_id, is_public, name, filter_string)
             VALUES (1, ?1, 1, 'Legacy Filter Minimal', '{\"project_id\":1,\"status\":10}')",
            params![proj_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    // 2. Insert completely empty JSON string
    let filter2_id = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_filters_table (user_id, project_id, is_public, name, filter_string)
             VALUES (1, ?1, 1, 'Empty JSON Filter', '{}')",
            params![proj_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    // Verify db.get_filter cleanly deserializes
    let (sf1, bf1) = db
        .get_filter(filter1_id)
        .unwrap()
        .expect("filter1 must exist");
    assert_eq!(sf1.name, "Legacy Filter Minimal");
    assert_eq!(bf1.status, Some(10));
    assert!(bf1.sort.is_none());
    assert!(bf1.dir.is_none());
    assert!(bf1.per_page.is_none());

    let (sf2, bf2) = db
        .get_filter(filter2_id)
        .unwrap()
        .expect("filter2 must exist");
    assert_eq!(sf2.name, "Empty JSON Filter");
    assert_eq!(bf2, BugFilter::default());

    // HTTP GET /view_all_bug_page?filter_id=<id1> returns 200 OK and filters issues
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", filter1_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    assert!(html.contains(&format!("{:07}", b1)));
    assert!(!html.contains(&format!("{:07}", b2)));

    // HTTP GET /view_all_bug_page?filter_id=<id2> returns 200 OK
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_all_bug_page?filter_id={}", filter2_id))
                .header("cookie", &full_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_milestone1_pagination_sorting_sticky_pinning_and_filter_delete_permissions() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("M1RegressionProj", 10, 10, "", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    let rep1_id = db
        .create_user(
            "rep_owner",
            "Reporter Owner",
            "rep1@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep1_cookie = login_user(&app, "rep_owner", "pass123").await;
    let full_rep1_cookie = set_project_cookie(&app, &rep1_cookie, proj_id).await;

    let _rep2_id = db
        .create_user(
            "rep_other",
            "Reporter Other",
            "rep2@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep2_cookie = login_user(&app, "rep_other", "pass123").await;
    let full_rep2_cookie = set_project_cookie(&app, &rep2_cookie, proj_id).await;

    let _mgr_id = db
        .create_user(
            "mgr_m1",
            "Manager M1",
            "mgr@x.com",
            "pass123",
            70,
            true,
            false,
        )
        .unwrap();
    let mgr_cookie = login_user(&app, "mgr_m1", "pass123").await;
    let full_mgr_cookie = set_project_cookie(&app, &mgr_cookie, proj_id).await;

    // 1. Pagination Preserves Parameters across Pages
    for i in 1..=25 {
        db.create_bug(
            proj_id,
            1,
            0,
            &format!("Pagination Item {:02}", i),
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
    }

    let uri_p1 =
        "/view_all_bug_page?page=1&q=Pagination&priority=30&status=10&sort=id&dir=asc&per_page=10";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri_p1)
                .header("cookie", &full_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_p1 = response_body_string(res).await;
    assert!(html_p1.contains("Pagination Item 01"));
    assert!(html_p1.contains("page=2"));
    assert!(html_p1.contains("q=Pagination"));
    assert!(html_p1.contains("per_page=10"));
    assert!(html_p1.contains("sort=id"));
    assert!(html_p1.contains("dir=asc"));

    let uri_p2 =
        "/view_all_bug_page?page=2&q=Pagination&priority=30&status=10&sort=id&dir=asc&per_page=10";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri_p2)
                .header("cookie", &full_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html_p2 = response_body_string(res).await;
    assert!(html_p2.contains("Pagination Item 11"));

    // 2. Sticky Bugs Always Pinned at Top Regardless of Sort Key or Direction
    let normal_b1 = db
        .create_bug(
            proj_id,
            1,
            0,
            "Alpha Normal Bug",
            "Desc",
            "",
            "",
            10, // low priority
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let normal_b2 = db
        .create_bug(
            proj_id,
            1,
            0,
            "Zulu Normal Bug",
            "Desc",
            "",
            "",
            50, // urgent priority
            80,
            90,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let sticky_b = db
        .create_bug(
            proj_id,
            1,
            0,
            "Sticky Pinned Bug",
            "Desc",
            "",
            "",
            20,
            20,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Mark sticky_b as sticky = 1
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_bug_table SET sticky = 1 WHERE id = ?1",
            params![sticky_b],
        )
        .unwrap();
    }

    // Verify sort_bugs function directly
    let mut bugs_sample = vec![
        db.get_bug(normal_b1).unwrap().unwrap(),
        db.get_bug(normal_b2).unwrap().unwrap(),
        db.get_bug(sticky_b).unwrap().unwrap(),
    ];

    let sort_keys = [
        "id",
        "priority",
        "severity",
        "status",
        "last_updated",
        "summary",
    ];
    for key in sort_keys {
        for dir in ["asc", "desc"] {
            sort_bugs(&mut bugs_sample, Some(key), Some(dir));
            assert_eq!(
                bugs_sample[0].id, sticky_b,
                "Sticky bug must always be first when sorting by {} {}",
                key, dir
            );
        }
    }

    // Also verify over HTTP for sort=summary&dir=desc
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/view_all_bug_page?sort=summary&dir=desc")
                .header("cookie", &full_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = response_body_string(res).await;
    let sticky_pos = html
        .find("Sticky Pinned Bug")
        .expect("must have sticky bug");
    let zulu_pos = html.find("Zulu Normal Bug").expect("must have zulu bug");
    assert!(
        sticky_pos < zulu_pos,
        "Sticky issue must appear before Zulu bug in HTML even on summary desc sort"
    );

    // 3. POST /filter_delete/:id Permission Matrix
    let f_owner = db
        .save_filter(
            rep1_id,
            proj_id,
            false,
            "OwnerFilter1",
            &BugFilter {
                status: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

    // Unrelated reporter receives 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", f_owner))
                .header("cookie", &full_rep2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(db.get_filter(f_owner).unwrap().is_some());

    // Owner succeeds (303 See Other)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", f_owner))
                .header("cookie", &full_rep1_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_filter(f_owner).unwrap().is_none());

    // Manager (70) cannot delete another user's private filter -> 403 Forbidden
    let f_owner2 = db
        .save_filter(
            rep1_id,
            proj_id,
            false,
            "OwnerFilter2",
            &BugFilter {
                status: Some(20),
                ..Default::default()
            },
        )
        .unwrap();

    let res_mgr_priv_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", f_owner2))
                .header("cookie", &full_mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_priv_del.status(), StatusCode::FORBIDDEN);
    assert!(db.get_filter(f_owner2).unwrap().is_some());

    // Administrator (90) can moderation-delete another user's private filter
    let admin_login = app
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
    let admin_cookie = admin_login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let res_adm_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", f_owner2))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_adm_del.status(), StatusCode::SEE_OTHER);
    assert!(db.get_filter(f_owner2).unwrap().is_none());

    // Manager (70) can delete another user's public filter in their managed project
    let f_pub = db
        .save_filter(
            rep1_id,
            proj_id,
            true,
            "PublicFilterToModerate",
            &BugFilter {
                status: Some(50),
                ..Default::default()
            },
        )
        .unwrap();
    let res_mgr_pub_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", f_pub))
                .header("cookie", &full_mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_pub_del.status(), StatusCode::SEE_OTHER);
    assert!(db.get_filter(f_pub).unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Milestone 2 Regressions
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_m2_sticky_toggle_permissions_due_date_and_overdue_badge() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("M2FieldsProj", 10, 10, "M2 Proj", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    let _rep_id = db
        .create_user("m2_rep", "M2 Rep", "r@m2.com", "pass123", 25, true, false)
        .unwrap();
    let rep_cookie = login_user(&app, "m2_rep", "pass123").await;
    let full_rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    let _dev_id = db
        .create_user("m2_dev", "M2 Dev", "d@m2.com", "pass123", 55, true, false)
        .unwrap();
    let dev_cookie = login_user(&app, "m2_dev", "pass123").await;
    let full_dev_cookie = set_project_cookie(&app, &dev_cookie, proj_id).await;

    let _mgr_id = db
        .create_user("m2_mgr", "M2 Mgr", "m@m2.com", "pass123", 70, true, false)
        .unwrap();
    let mgr_cookie = login_user(&app, "m2_mgr", "pass123").await;
    let full_mgr_cookie = set_project_cookie(&app, &mgr_cookie, proj_id).await;

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            0,
            "Issue for Sticky and Dates",
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

    // 1. Sticky Toggle Permission Guards
    // Reporter (25) -> 403 Forbidden
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
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().sticky, 0);

    // Developer (55) -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_stick/{}", bug_id))
                .header("cookie", &full_dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().sticky, 0);

    // Manager (70) -> 303 See Other
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
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().sticky, 1);

    // View bug page shows [Sticky] and Unstick button
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
    let html = response_body_string(res).await;
    assert!(html.contains("[Sticky]"));
    assert!(html.contains("Unstick"));

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

    // 2. Build, Due Date, OS, OS Build, Platform fields update and Overdue badge
    let update_body = "summary=Issue+with+OS+and+Build&category_id=0&priority=30&severity=50&reproducibility=70&version=1.0&build=build-rel-2026&due_date=2021-06-15&os=macOS&os_build=15.1&platform=aarch64&view_state=10";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &full_admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let updated = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(updated.build, "build-rel-2026");
    assert_eq!(updated.os, "macOS");
    assert_eq!(updated.os_build, "15.1");
    assert_eq!(updated.platform, "aarch64");
    assert_eq!(updated.due_date, 1623715200); // 2021-06-15 UTC

    // View page renders fields and Overdue badge (open bug with past due date)
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
    let html = response_body_string(res).await;
    assert!(html.contains("build-rel-2026"));
    assert!(html.contains("macOS"));
    assert!(html.contains("15.1"));
    assert!(html.contains("aarch64"));
    assert!(html.contains("2021-06-15"));
    assert!(html.contains("Overdue"));

    // Resolve bug (status = 80 resolved): Overdue badge must NOT be displayed
    db.change_bug_status(bug_id, 1, 80, 20, Some("Resolved issue"))
        .unwrap();

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
    let html = response_body_string(res).await;
    assert!(html.contains("2021-06-15"));
    assert!(!html.contains("Overdue"));
}

#[tokio::test]
async fn test_m2_subproject_rollup_and_private_subproject_isolation() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let parent_id = db
        .create_project("ParentProj", 10, 10, "Parent", true)
        .unwrap();
    let child_pub_id = db
        .create_project("ChildPublicProj", 10, 10, "Child Public", true)
        .unwrap();
    let child_priv_id = db
        .create_project("ChildPrivateProj", 10, 50, "Child Private", true)
        .unwrap();

    db.add_subproject(parent_id, child_pub_id, true).unwrap();
    db.add_subproject(parent_id, child_priv_id, true).unwrap();

    let _dev_id = db
        .create_user(
            "rollup_dev",
            "Rollup Dev",
            "d@r.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();
    let rep_id = db
        .create_user(
            "rollup_rep",
            "Rollup Rep",
            "r@r.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_cookie = login_user(&app, "rollup_rep", "pass123").await;
    let full_rep_cookie = set_project_cookie(&app, &rep_cookie, parent_id).await;

    let mgr_cookie = login_user(&app, "administrator", "root").await;
    let full_mgr_cookie = set_project_cookie(&app, &mgr_cookie, parent_id).await;

    let b_parent = db
        .create_bug(
            parent_id,
            rep_id,
            0,
            "Parent Project Issue",
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

    let b_child_pub = db
        .create_bug(
            child_pub_id,
            rep_id,
            0,
            "Public Child Issue",
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

    let b_child_priv = db
        .create_bug(
            child_priv_id,
            rep_id,
            0,
            "Private Child Issue",
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

    // Reporter viewing Parent project issues:
    // Sees Parent Issue and Public Child Issue, but Private Child Issue is excluded!
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
    let html = response_body_string(res).await;
    assert!(html.contains(&format!("{:07}", b_parent)));
    assert!(html.contains(&format!("{:07}", b_child_pub)));
    assert!(!html.contains(&format!("{:07}", b_child_priv)));

    // Manager viewing Parent project issues:
    // Sees Parent Issue, Public Child Issue, and Private Child Issue!
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
    let html = response_body_string(res).await;
    assert!(html.contains(&format!("{:07}", b_parent)));
    assert!(html.contains(&format!("{:07}", b_child_pub)));
    assert!(html.contains(&format!("{:07}", b_child_priv)));
}

#[tokio::test]
async fn test_m2_category_auto_assign_and_legacy_unassigned_preservation() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("CategoryAutoAssignProj", 10, 10, "", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    let dev_id = db
        .create_user(
            "auto_dev",
            "Auto Developer",
            "autodev@x.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();

    let cat_id = db.add_category(proj_id, "BackendService", dev_id).unwrap();

    // 1. Report new issue via HTTP POST /bug_report in BackendService category
    let form = format!(
        "project_id={}&category_id={}&summary=AutoAssign+Bug&description=Testing&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=1.0&view_state=10",
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

    let bugs = db.list_bugs(proj_id, 0, 10).unwrap();
    let bug = bugs.iter().find(|b| b.summary == "AutoAssign Bug").unwrap();
    assert_eq!(bug.handler_id, dev_id);
    assert_eq!(bug.status, 50); // Assigned

    // 2. Existing legacy unassigned issue in DB remains untouched
    let legacy_unassigned_id = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_bug_table (
                project_id, reporter_id, handler_id, duplicate_id, priority, severity,
                reproducibility, status, resolution, category_id, date_submitted, last_updated,
                os, os_build, platform, version, fixed_in_version, target_version, build,
                summary, view_state, sticky, due_date
            ) VALUES (?1, 1, 0, 0, 30, 50, 70, 10, 10, ?2, 1600000000, 1600000000,
                      '', '', '', '', '', '', '', 'Legacy Unassigned Bug', 10, 0, 0)",
            params![proj_id, cat_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    let legacy_bug = db.get_bug(legacy_unassigned_id).unwrap().unwrap();
    assert_eq!(legacy_bug.handler_id, 0);
    assert_eq!(legacy_bug.status, 10);
}

#[tokio::test]
async fn test_m2_project_access_min_backward_compat() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // Public project with default access_min = 10
    let pub_proj = db
        .create_project("DefaultMinProj", 10, 10, "Min 10", true)
        .unwrap();

    // Public project with stricter access_min = 55 (Developer required)
    let strict_proj = db
        .create_project("StrictMinProj", 10, 10, "Min 55", true)
        .unwrap();
    db.set_project_access_min(strict_proj, 55).unwrap();

    let rep_id = db
        .create_user(
            "min_rep",
            "Min Rep",
            "r@min.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_cookie = login_user(&app, "min_rep", "pass123").await;
    let pub_rep_cookie = set_project_cookie(&app, &rep_cookie, pub_proj).await;
    let strict_rep_cookie = set_project_cookie(&app, &rep_cookie, strict_proj).await;

    let dev_cookie = login_user(&app, "administrator", "root").await;
    let strict_admin_cookie = set_project_cookie(&app, &dev_cookie, strict_proj).await;

    // Reporter can report on DefaultMinProj (25 >= 10)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &pub_rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=0&summary=Allowed+Bug&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=&view_state=10",
                    pub_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Reporter is blocked on StrictMinProj (25 < 55) -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &strict_rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=0&summary=Blocked+Bug&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=&view_state=10",
                    strict_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Admin / Developer succeeds on StrictMinProj
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &strict_admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=0&summary=Admin+Allowed+Bug&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=&view_state=10",
                    strict_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Explicit project-user override allows Reporter to report on StrictMinProj
    db.set_project_user(strict_proj, rep_id, 55).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &strict_rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=0&summary=Override+Allowed+Bug&description=Desc&steps_to_reproduce=Steps&priority=30&severity=50&reproducibility=70&version=&view_state=10",
                    strict_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn test_m2_readable_history_on_legacy_numeric_rows() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("HistoryReadableProj", 10, 10, "", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            0,
            "History Issue",
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

    // Insert legacy numeric history rows directly into mantis_bug_history_table
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (1, ?1, 1600000100, 'status', '10', '50', 0)",
            params![bug_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (1, ?1, 1600000200, 'priority', '30', '40', 0)",
            params![bug_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (1, ?1, 1600000300, 'handler_id', '0', '1', 0)",
            params![bug_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
             VALUES (1, ?1, 1600000400, 'sticky', '0', '1', 0)",
            params![bug_id],
        )
        .unwrap();
    }

    // View page renders human-readable labels instead of raw numbers
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
    let html = response_body_string(res).await;

    // Readable labels
    assert!(html.contains("new =&gt; assigned"));
    assert!(html.contains("normal =&gt; high"));
    assert!(html.contains("none =&gt; administrator"));
    assert!(html.contains("no =&gt; yes"));

    // Ensure raw numbers are not shown as the formatted change
    assert!(!html.contains("10 =&gt; 50"));
    assert!(!html.contains("30 =&gt; 40"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Legacy Custom Field Data & Edit-Page HTML Safety (Pre-M3 Baseline)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_legacy_custom_field_free_text_data_and_edit_page_html_safety() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("CustomFieldProj", 10, 10, "", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    let (_tok_id, raw_api_token) = db.create_api_token(1, "CF_Admin_Token").unwrap();

    // Create custom fields of diverse types
    let cf_num = db
        .create_custom_field(
            "CF_Numeric",
            CustomFieldType::Numeric as i32,
            "",
            "",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_float = db
        .create_custom_field(
            "CF_Float",
            CustomFieldType::Float as i32,
            "",
            "",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_enum = db
        .create_custom_field(
            "CF_Enum",
            CustomFieldType::Enum as i32,
            "OptionA|OptionB|OptionC",
            "OptionA",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_email = db
        .create_custom_field(
            "CF_Email",
            CustomFieldType::Email as i32,
            "",
            "",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_cb = db
        .create_custom_field(
            "CF_Checkbox",
            CustomFieldType::Checkbox as i32,
            "",
            "0",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_multi = db
        .create_custom_field(
            "CF_Multilist",
            CustomFieldType::Multilist as i32,
            "Red|Green|Blue",
            "",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    let cf_date = db
        .create_custom_field(
            "CF_Date",
            CustomFieldType::Date as i32,
            "",
            "",
            "",
            0,
            0,
            false,
            false,
            true,
            true,
        )
        .unwrap();

    let all_cfs = [
        cf_num, cf_float, cf_enum, cf_email, cf_cb, cf_multi, cf_date,
    ];
    for (seq, cf_id) in all_cfs.iter().enumerate() {
        db.link_custom_field_to_project(*cf_id, proj_id, seq as i32 + 1)
            .unwrap();
    }

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            0,
            "Issue with Legacy Free-Text Custom Fields",
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

    // Insert legacy free-text / non-conforming data directly via raw SQL
    {
        let conn = db.conn();
        let legacy_entries = [
            (cf_num, "legacy-non-numeric-text"),
            (cf_float, "invalid-float-value"),
            (cf_enum, "NotAnOptionInEnum"),
            (cf_email, "invalid-email-address"),
            (cf_cb, "arbitrary-string-val"),
            (cf_multi, "InvalidVal|NotListed"),
            (cf_date, "not-a-unix-timestamp"),
        ];
        for (cf_id, val) in legacy_entries {
            conn.execute(
                "INSERT INTO mantis_custom_field_string_table (field_id, bug_id, value, text)
                 VALUES (?1, ?2, ?3, ?3)",
                params![cf_id, bug_id, val],
            )
            .unwrap();
        }
    }

    // 1. GET /view_bug_page/:id returns 200 OK and renders values intact without error
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
    let view_html = response_body_string(res).await;
    assert!(view_html.contains("legacy-non-numeric-text"));
    assert!(view_html.contains("invalid-float-value"));
    assert!(view_html.contains("invalid-email-address"));

    // 2. GET /update_bug_page/:id returns 200 OK and renders fields intact
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/update_bug_page/{}", bug_id))
                .header("cookie", &full_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let update_html = response_body_string(res).await;
    assert!(update_html.contains("legacy-non-numeric-text"));

    // 3. GET /view_all_bug_page returns 200 OK
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

    // 4. REST API GET /api/rest/issues/:id preserves stored values
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/rest/issues/{}", bug_id))
                .header("authorization", format!("Bearer {}", raw_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_str = response_body_string(res).await;
    let json_val: Value = serde_json::from_str(&body_str).unwrap();
    let issues = json_val["issues"].as_array().expect("issues array");
    let issue_cfs = issues[0]["custom_fields"]
        .as_array()
        .expect("custom fields array");
    let num_cf = issue_cfs
        .iter()
        .find(|c| c["field"]["name"] == "CF_Numeric")
        .expect("CF_Numeric present");
    assert_eq!(num_cf["value"], "legacy-non-numeric-text");

    // 5. Verify no nested <form> tags across all key edit pages
    assert_no_nested_forms(&update_html);

    let edit_pages = [
        format!("/manage_proj_edit_page/{}", proj_id),
        format!("/manage_custom_field_edit_page/{}", cf_num),
        "/manage_user_edit_page/1".to_string(),
    ];
    for uri in edit_pages {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&uri)
                    .header("cookie", &full_admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "Failed loading {}", uri);
        let html = response_body_string(res).await;
        assert_no_nested_forms(&html);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Legacy Password Hashing Upgrade & Session Lifecycle
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_legacy_password_hashes_upgrade_and_session_lifecycle() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Seed user with legacy 32-char lowercase hex MD5 hash
    // Plain password = "SecretMd5Password!"
    let plain_md5 = "SecretMd5Password!";
    let md5_hash = rust_mantis_bt::auth::md5_hex(plain_md5);
    assert_eq!(md5_hash.len(), 32);

    let md5_user_id = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_user_table (
                username, realname, email, password, enabled, protected, access_level,
                login_count, cookie_string, date_created
            ) VALUES ('md5user', 'MD5 User', 'md5@x.com', ?1, 1, 0, 25, 0, 'cookie_md5_init', 1600000000)",
            params![md5_hash],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    // 2. Seed user with legacy sha256$<salt>$<hex> password hash
    // Plain password = "SecretSha256Password!"
    let plain_sha = "SecretSha256Password!";
    let sha_salt = "custom_salt_987";
    let mut hasher = Sha256::new();
    hasher.update(sha_salt.as_bytes());
    hasher.update(plain_sha.as_bytes());
    let sha_hex = format!("{:x}", hasher.finalize());
    let sha_stored = format!("sha256${}${}", sha_salt, sha_hex);

    let sha_user_id = {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_user_table (
                username, realname, email, password, enabled, protected, access_level,
                login_count, cookie_string, date_created
            ) VALUES ('shauser', 'SHA User', 'sha@x.com', ?1, 1, 0, 25, 0, 'cookie_sha_init', 1600000000)",
            params![sha_stored],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    // 3. Login md5user via POST /login
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username=md5user&password={}",
                    plain_md5
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res
        .headers()
        .get("set-cookie")
        .expect("cookie must be set")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_hdr.contains("MANTIS_STRING_COOKIE="));
    assert!(cookie_hdr.contains("HttpOnly"));
    assert!(cookie_hdr.contains("SameSite=Lax"));

    // Verify stored password in DB was transparently upgraded to pbkdf2_sha256$...
    let upgraded_md5_user = db.get_user_by_id(md5_user_id).unwrap().unwrap();
    assert!(
        upgraded_md5_user.password.starts_with("pbkdf2_sha256$"),
        "Password must be upgraded to pbkdf2, but was {}",
        upgraded_md5_user.password
    );

    // Second login with upgraded password succeeds
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username=md5user&password={}",
                    plain_md5
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 4. Login shauser via POST /login
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username=shauser&password={}",
                    plain_sha
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify stored password was transparently upgraded to pbkdf2_sha256$...
    let upgraded_sha_user = db.get_user_by_id(sha_user_id).unwrap().unwrap();
    assert!(
        upgraded_sha_user.password.starts_with("pbkdf2_sha256$"),
        "Password must be upgraded to pbkdf2, but was {}",
        upgraded_sha_user.password
    );

    // Second login with upgraded password succeeds
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username=shauser&password={}",
                    plain_sha
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let sha_cookie_pair = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // 5. Session Lifecycle & Logout Invalidation
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", &sha_cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // POST /logout
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/logout")
                .header("cookie", &sha_cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Old session cookie is now invalidated and redirected to /login
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", &sha_cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("/login"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Access Boundaries, Planning Pages, CSV Injection & REST API Regressions
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_access_boundaries_and_visibility_matrix() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let _admin_cookie = login_as_admin(&app).await;

    let viewer_id = db
        .create_user("view_user", "Viewer", "v@x.com", "pass123", 10, true, false)
        .unwrap();
    let _viewer_cookie = login_user(&app, "view_user", "pass123").await;

    let rep_a_id = db
        .create_user(
            "rep_a",
            "Reporter A",
            "ra@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_a_cookie = login_user(&app, "rep_a", "pass123").await;

    let _rep_b_id = db
        .create_user(
            "rep_b",
            "Reporter B",
            "rb@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_b_cookie = login_user(&app, "rep_b", "pass123").await;

    let dev_id = db
        .create_user(
            "dev_user",
            "Developer",
            "d@x.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();
    let dev_cookie = login_user(&app, "dev_user", "pass123").await;

    let pub_proj = db
        .create_project("PublicAccessProj", 10, 10, "", true)
        .unwrap();
    let priv_proj = db
        .create_project("PrivateAccessProj", 10, 50, "", true)
        .unwrap();
    let disabled_proj = db
        .create_project("DisabledAccessProj", 10, 10, "", true)
        .unwrap();
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_project_table SET enabled = 0 WHERE id = ?1",
            params![disabled_proj],
        )
        .unwrap();
    }

    // 1. Private Project Boundary
    // Viewer & Reporter effective access level is 0
    let viewer = db.get_user_by_id(viewer_id).unwrap().unwrap();
    assert_eq!(db.effective_access_level(&viewer, priv_proj).unwrap(), 0);

    // Manager / Admin effective access is >= 70
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    assert_eq!(db.effective_access_level(&admin, priv_proj).unwrap(), 90);

    // Explicit project assignment allows Viewer into private project
    db.set_project_user(priv_proj, viewer_id, 25).unwrap();
    assert_eq!(db.effective_access_level(&viewer, priv_proj).unwrap(), 25);

    // 2. Disabled Project Boundary
    // Non-admin (Developer 55) cannot access disabled project
    let dev = db.get_user_by_id(dev_id).unwrap().unwrap();
    assert_eq!(db.effective_access_level(&dev, disabled_proj).unwrap(), 0);
    // Administrator can access disabled project
    assert_eq!(
        db.effective_access_level(&admin, disabled_proj).unwrap(),
        90
    );

    // 3. Private Bug Boundary (view_state = 50)
    let full_pub_rep_a = set_project_cookie(&app, &rep_a_cookie, pub_proj).await;
    let full_pub_rep_b = set_project_cookie(&app, &rep_b_cookie, pub_proj).await;
    let full_pub_dev = set_project_cookie(&app, &dev_cookie, pub_proj).await;

    let priv_bug_id = db
        .create_bug(
            pub_proj,
            rep_a_id,
            0,
            "Confidential Security Vulnerability",
            "Sensitive details",
            "",
            "",
            40,
            60,
            10,
            "",
            "",
            "",
            "",
            50, // Private bug
        )
        .unwrap();

    // Reporter B (unrelated reporter) requesting /view_bug_page/:id receives 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", priv_bug_id))
                .header("cookie", &full_pub_rep_b)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Reporter A (author of private bug) can view
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", priv_bug_id))
                .header("cookie", &full_pub_rep_a)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Developer can view
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", priv_bug_id))
                .header("cookie", &full_pub_dev)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Private Bugnote Boundary (view_state = 50)
    // Create public bug
    let pub_bug_id = db
        .create_bug(
            pub_proj,
            rep_a_id,
            0,
            "Public Bug with Mixed Notes",
            "Public bug desc",
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

    // Developer adds private bugnote
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", pub_bug_id))
                .header("cookie", &full_pub_dev)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "note=Confidential+Internal+Audit+Note&view_state=50",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Developer also adds public bugnote
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bugnote_add/{}", pub_bug_id))
                .header("cookie", &full_pub_dev)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("note=Public+Note+Visible+To+All&view_state=10"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Reporter viewing public bug sees public note, but NOT private note
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", pub_bug_id))
                .header("cookie", &full_pub_rep_b)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rep_html = response_body_string(res).await;
    assert!(rep_html.contains("Public Note Visible To All"));
    assert!(!rep_html.contains("Confidential Internal Audit Note"));

    // Developer viewing public bug sees both public and private notes
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", pub_bug_id))
                .header("cookie", &full_pub_dev)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dev_html = response_body_string(res).await;
    assert!(dev_html.contains("Public Note Visible To All"));
    assert!(dev_html.contains("Confidential Internal Audit Note"));
}

#[tokio::test]
async fn test_csv_export_formula_injection_sanitization() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("CsvSanitizeProj", 10, 10, "", true)
        .unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_admin_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // Create bugs with summaries starting with formula characters
    let payload_cmd = "=CMD|' /C calc'!A0";
    let payload_plus = "+1+1";
    let payload_minus = "-2";
    let payload_at = "@SUM(A1:A10)";

    for p in [payload_cmd, payload_plus, payload_minus, payload_at] {
        db.create_bug(
            proj_id,
            1,
            0,
            p,
            "Description of formula test",
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
    }

    // Call GET /csv_export
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/csv_export")
                .header("cookie", &full_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let csv_content = response_body_string(res).await;

    // Verify all leading formula characters were neutralized with leading single quote `'`
    assert!(csv_content.contains("'=CMD"));
    assert!(csv_content.contains("'+1+1"));
    assert!(csv_content.contains("'-2"));
    assert!(csv_content.contains("'@SUM"));
}

#[tokio::test]
async fn test_planning_pages_and_rest_api_smoke_regression() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let proj_id = db
        .create_project("PlanningRestProj", 10, 10, "", true)
        .unwrap();

    let dev_id = db
        .create_user(
            "plan_dev", "Plan Dev", "pd@x.com", "pass123", 55, true, false,
        )
        .unwrap();
    let dev_cookie = login_user(&app, "plan_dev", "pass123").await;
    let full_dev_cookie = set_project_cookie(&app, &dev_cookie, proj_id).await;

    let _rep_id = db
        .create_user(
            "plan_rep", "Plan Rep", "pr@x.com", "pass123", 25, true, false,
        )
        .unwrap();
    let rep_cookie = login_user(&app, "plan_rep", "pass123").await;
    let full_rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    let (_dev_tok_id, dev_api_token) = db.create_api_token(dev_id, "DevRestToken").unwrap();
    let (_mgr_tok_id, admin_api_token) = db.create_api_token(1, "AdminRestToken").unwrap();

    // 1. Planning Pages: Role guard check
    // Reporter (25):
    let ok_reporter_pages = [
        "/my_view_page",
        "/roadmap_page",
        "/changelog_page",
        "/summary_page",
        "/news_page",
    ];
    for uri in ok_reporter_pages {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", &full_rep_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "Page {} failed for reporter",
            uri
        );
    }

    // Reporter cannot access /billing_page -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page")
                .header("cookie", &full_rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Developer (55) can access /billing_page -> 200 OK
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/billing_page")
                .header("cookie", &full_dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. REST API Regression Flow
    // GET /api/rest/users/me
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/rest/users/me")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me_json: Value = serde_json::from_str(&response_body_string(res).await).unwrap();
    assert_eq!(me_json["name"], "plan_dev");

    // GET /api/rest/projects
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/rest/projects")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let projs_json: Value = serde_json::from_str(&response_body_string(res).await).unwrap();
    assert!(!projs_json["projects"].as_array().unwrap().is_empty());

    // POST /api/rest/issues
    let create_payload = json!({
        "summary": "REST Regression Issue",
        "description": "Created via REST API",
        "project": { "id": proj_id },
        "category": { "name": "General" }
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rest/issues")
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created_json: Value = serde_json::from_str(&response_body_string(res).await).unwrap();
    let new_issue_id = created_json["issue"]["id"].as_i64().expect("new issue id");

    // GET /api/rest/issues/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/rest/issues/{}", new_issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let get_json: Value = serde_json::from_str(&response_body_string(res).await).unwrap();
    assert_eq!(get_json["issues"][0]["summary"], "REST Regression Issue");

    // PATCH /api/rest/issues/:id
    let patch_payload = json!({
        "summary": "Updated REST Issue Summary",
        "status": { "id": 50 }
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/rest/issues/{}", new_issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .header("content-type", "application/json")
                .body(Body::from(patch_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // POST /api/rest/issues/:id/notes
    let note_payload = json!({
        "text": "Bugnote added via REST API"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/rest/issues/{}/notes", new_issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .header("content-type", "application/json")
                .body(Body::from(note_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // DELETE /api/rest/issues/:id with Developer token -> 403 Forbidden (Manager required)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/rest/issues/{}", new_issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", dev_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // DELETE /api/rest/issues/:id with Administrator token -> 204 No Content
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/rest/issues/{}", new_issue_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", admin_api_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify issue is deleted
    assert!(db.get_bug(new_issue_id).unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Phase 3.1 Regressions: Advanced Filters, Manage Filters & Column Config
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_phase3_m1_backward_compat_and_no_nested_forms() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let proj_id = db.create_project("P3CompatProj", 10, 10, "", true).unwrap();
    let admin_cookie = login_as_admin(&app).await;
    let full_cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // 1. Legacy BugFilter JSON without Phase 3.1 fields deserializes cleanly & works via permalink
    let fid = db
        .save_filter(
            1,
            proj_id,
            true,
            "Legacy Compat Filter",
            &BugFilter {
                project_id: Some(proj_id),
                status: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

    for uri in [
        format!("/view_all_bug_page?filter_id={}", fid),
        format!("/print_all_bug_page?filter_id={}", fid),
        "/manage_filter_page".to_string(),
        format!("/manage_filter_edit_page/{}", fid),
        format!("/account_manage_columns_page?project_id={}", proj_id),
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&uri)
                    .header("cookie", &full_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "GET {} failed", uri);
        let html = response_body_string(res).await;
        assert_no_nested_forms(&html);
    }

    // 2. Phase 3.2 export & RSS regression checks
    for uri in [
        "/excel_all_bug_page",
        "/word_all_bug_page",
        "/bug_export_word",
        "/issues_rss",
        "/news_rss",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", &full_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "GET {} failed", uri);
    }
}
