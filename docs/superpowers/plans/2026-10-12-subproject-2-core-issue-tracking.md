# Sub-project 2: Core Issue Tracking & Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the complete MantisBT issue lifecycle system (bug CRUD, status workflow, bugnotes, audit history, relationships, and file attachments) on top of the Sub-project 1 foundation.

**Architecture:** 6 new SQLite tables added via the existing `Db::migrate()` runner. New data-access methods on `db::Db`. New `src/handlers/bugs.rs` with all issue endpoints wired into `build_router`. Server-side rendered HTML extending the Sub-project 1 MantisBT layout shell (`render_layout`).

**Tech Stack:** Same as Sub-project 1: `axum`, `rusqlite` (`bundled`), `sha2`, `rand`, `serde`. File uploads use `axum::extract::Multipart` (already part of `axum`; no new deps).

**Spec:** `docs/superpowers/specs/2026-10-12-subproject-2-core-issue-tracking-design.md`

## Global Constraints

- New table names: `mantis_bug_table`, `mantis_bug_text_table`, `mantis_bugnote_table`, `mantis_bugnote_text_table`, `mantis_bug_history_table`, `mantis_bug_relationship_table`, `mantis_bug_file_table`. Column names match spec Section 2 exactly.
- `mantis_bug_table.id == mantis_bug_text_table.id` (same primary key; both inserted together in one `Db` method).
- Bug Status integer codes: `new=10`, `feedback=20`, `acknowledged=30`, `confirmed=40`, `assigned=50`, `resolved=80`, `closed=90`.
- Bug Priority: `none=10`, `low=20`, `normal=30`, `high=40`, `urgent=50`, `immediate=60`.
- Bug Severity: `feature=10`, `trivial=20`, `text=30`, `tweak=40`, `minor=50`, `major=60`, `crash=70`, `block=80`.
- Bug Reproducibility: `always=10`, `sometimes=30`, `random=50`, `have_not_tried=70`, `unable=90`, `na=100`.
- Bug Resolution: `open=10`, `fixed=20`, `reopened=30`, `unable_to_reproduce=40`, `not_fixable=50`, `duplicate=60`, `not_a_bug=70`, `suspended=80`, `wont_fix=90`.
- Relationship Type: `related_to=0`, `duplicate_of=1`, `has_duplicate=2`, `blocks=3`, `blocked_by=4`, `parent_of=5`, `child_of=6`.
- Access level thresholds (per Spec Section 4.1): `view public bug ≥ 10`, `view private bug ≥ 55`, `report bug ≥ 25`, `update any bug ≥ 40`, `assign bug ≥ 55`, `resolve bug ≥ 55`, `close bug ≥ 70`, `delete bug ≥ 70`, `delete bugnote ≥ 70`, `manage relationships ≥ 55`, `upload attachment ≥ 25`, `delete attachment ≥ 70`.
- All HTML output must escape user-controlled strings (`html_escape`).
- File attachment env vars: `MANTIS_UPLOAD_DIR` (default `""` = inline BLOB), `MANTIS_MAX_FILE_SIZE` (default `5242880` = 5MB).

## Review Focus

- Private bug (`view_state=50`) must not appear in `/view_all_bug_page` for users with `effective_access_level < 55`, and must return HTTP 403 when accessed via `/view_bug_page/:id` — bug listing query must filter at SQL level, not template level.
- Duplicate-of relationship (`relationship_type=1`) must auto-create reverse `has_duplicate (2)` entry; deleting one side must delete both sides.
- Every field changed in `POST /update_bug/:id` must produce a row in `mantis_bug_history_table` with correct `old_value` / `new_value`; unchanged fields must produce no row.
- Closed bug (`status=90`) cannot transition to `resolved/acknowledged/confirmed/assigned` — only `feedback (20)` (reopening flow sends back to feedback); attempt must return an error flash message.
- File uploads exceeding `MANTIS_MAX_FILE_SIZE` must return HTTP 413 before touching the DB.

---

### Task 1: New Models & Database Schema (6 Bug Tables)

**Files:**
- Modify: `src/models.rs`
- Modify: `src/db.rs`

**Interfaces:**
- Consumes: existing `Db`, `models::*` from Sub-project 1.
- Produces:
  - `models::BugStatus` (`New=10`, `Feedback=20`, `Acknowledged=30`, `Confirmed=40`, `Assigned=50`, `Resolved=80`, `Closed=90`, `from_i32`, `as_str`)
  - `models::BugPriority` (`None=10`, `Low=20`, `Normal=30`, `High=40`, `Urgent=50`, `Immediate=60`)
  - `models::BugSeverity` (`Feature=10`, `Trivial=20`, `Text=30`, `Tweak=40`, `Minor=50`, `Major=60`, `Crash=70`, `Block=80`)
  - `models::BugReproducibility` (`Always=10`, `Sometimes=30`, `Random=50`, `HaveNotTried=70`, `UnableToReproduce=90`, `NA=100`)
  - `models::BugResolution` (`Open=10`, `Fixed=20`, `Reopened=30`, `UnableToReproduce=40`, `NotFixable=50`, `Duplicate=60`, `NotABug=70`, `Suspended=80`, `WontFix=90`)
  - `models::RelationshipType` (`RelatedTo=0`, `DuplicateOf=1`, `HasDuplicate=2`, `Blocks=3`, `BlockedBy=4`, `ParentOf=5`, `ChildOf=6`)
  - `models::Bug` (all `mantis_bug_table` columns, no text fields)
  - `models::BugFull` (`bug: Bug`, `description: String`, `steps_to_reproduce: String`, `additional_information: String`)
  - `models::Bugnote` (all `mantis_bugnote_table` columns + `note: String` from joined `mantis_bugnote_text_table`)
  - `models::BugHistory` (all `mantis_bug_history_table` columns)
  - `models::BugRelationship` (all `mantis_bug_relationship_table` columns)
  - `models::BugFile` (all `mantis_bug_file_table` columns except `content` BLOB)
  - `db::Db::create_bug(&self, project_id, reporter_id, category_id, summary, description, steps, additional_info, priority, severity, reproducibility, version, os, os_build, platform, view_state) -> Result<i64, String>`
  - `db::Db::get_bug(&self, id: i64) -> rusqlite::Result<Option<Bug>>`
  - `db::Db::get_bug_full(&self, id: i64) -> rusqlite::Result<Option<BugFull>>`
  - `db::Db::list_bugs(&self, project_id: i64, user_access: i32, offset: i64, limit: i64) -> rusqlite::Result<Vec<Bug>>`
  - `db::Db::update_bug_fields(&self, id: i64, updater_id: i64, fields: BugUpdate) -> Result<Vec<BugHistory>, String>` (returns new history rows)
  - `db::Db::delete_bug(&self, id: i64) -> rusqlite::Result<()>`
  - `db::Db::record_history(&self, conn: &rusqlite::Connection, bug_id: i64, user_id: i64, field: &str, old: &str, new: &str, type_: i32) -> rusqlite::Result<()>`

- [ ] **Step 1: Write failing tests in `src/db.rs` for schema bootstrap and bug CRUD**

```rust
#[test]
fn test_bug_schema_bootstrap_and_crud() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("TestProj", 10, 10, "", true).unwrap();
    let uid = db.create_user("alice", "Alice", "a@x.com", "pw", 25, true, false).unwrap();

    let bug_id = db.create_bug(
        proj_id, uid, 1, "Test summary",
        "Description here", "Steps", "Additional",
        30, 50, 70, "1.0", "", "", "", 10,
    ).unwrap();
    assert!(bug_id > 0);

    let bug = db.get_bug(bug_id).unwrap().expect("bug exists");
    assert_eq!(bug.summary, "Test summary");
    assert_eq!(bug.status, 10); // new
    assert_eq!(bug.priority, 30); // normal
    assert_eq!(bug.severity, 50); // minor

    let full = db.get_bug_full(bug_id).unwrap().expect("full bug");
    assert_eq!(full.description, "Description here");
    assert_eq!(full.steps_to_reproduce, "Steps");

    let bugs = db.list_bugs(proj_id, 10, 0, 25).unwrap();
    assert_eq!(bugs.len(), 1);

    db.delete_bug(bug_id).unwrap();
    assert!(db.get_bug(bug_id).unwrap().is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_bug_schema_bootstrap_and_crud`
Expected: FAIL (new tables not yet defined)

- [ ] **Step 3: Implement new enums in `src/models.rs` and new table migrations + `create_bug`, `get_bug`, `get_bug_full`, `list_bugs`, `delete_bug`, `record_history`, `update_bug_fields` in `src/db.rs`**

`create_bug` inserts into both `mantis_bug_table` and `mantis_bug_text_table` using the same auto-generated `id` (insert into `mantis_bug_table` first via `INSERT INTO ... RETURNING id` or `conn.last_insert_rowid()`, then `INSERT INTO mantis_bug_text_table (id, ...) VALUES (?1, ...)`). `list_bugs` filters private bugs: `WHERE (b.view_state = 10 OR ?1 >= 55)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`
Expected: PASS (33 existing + new tests green)

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(db): add bug tracking schema, enums, and core CRUD methods"
```

---

### Task 2: Bug Status Workflow, History, Bugnotes & Relationships

**Files:**
- Modify: `src/db.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::Bug`, `models::BugHistory`, `models::Bugnote`, `models::BugRelationship`
- Produces:
  - `db::Db::change_bug_status(&self, bug_id: i64, user_id: i64, new_status: i32, resolution: i32, note: Option<&str>) -> Result<(), String>` (validates transition, updates status/resolution, optionally adds bugnote, records history for status and resolution fields)
  - `db::Db::assign_bug(&self, bug_id: i64, user_id: i64, handler_id: i64) -> Result<(), String>`
  - `db::Db::add_bugnote(&self, bug_id: i64, reporter_id: i64, note: &str, view_state: i32) -> Result<i64, String>`
  - `db::Db::delete_bugnote(&self, note_id: i64) -> rusqlite::Result<()>`
  - `db::Db::list_bugnotes(&self, bug_id: i64, user_access: i32) -> rusqlite::Result<Vec<Bugnote>>` (filters private notes if `user_access < 55`)
  - `db::Db::list_bug_history(&self, bug_id: i64) -> rusqlite::Result<Vec<BugHistory>>`
  - `db::Db::add_bug_relationship(&self, source_id: i64, dest_id: i64, rel_type: i32) -> Result<(), String>` (auto-creates reverse for duplicate: `1↔2`; validates both bugs exist)
  - `db::Db::delete_bug_relationship(&self, rel_id: i64) -> rusqlite::Result<()>` (also deletes reverse for duplicate pair)
  - `db::Db::list_bug_relationships(&self, bug_id: i64) -> rusqlite::Result<Vec<BugRelationship>>`

- [ ] **Step 1: Write failing tests for status transitions, history recording, bugnotes, and relationships**

```rust
#[test]
fn test_bug_status_workflow_and_history() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db.create_user("dev", "Dev", "d@x.com", "pw", 55, true, false).unwrap();
    let bug_id = db.create_bug(proj_id, uid, 1, "Summary", "Desc", "", "", 30, 50, 70, "", "", "", "", 10).unwrap();

    // new → resolved (with note)
    db.change_bug_status(bug_id, uid, 80, 20, Some("Fixed it")).unwrap();
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);
    assert_eq!(bug.resolution, 20);

    let history = db.list_bug_history(bug_id).unwrap();
    let status_change = history.iter().find(|h| h.field_name == "status").unwrap();
    assert_eq!(status_change.old_value, "10");
    assert_eq!(status_change.new_value, "80");

    let notes = db.list_bugnotes(bug_id, 55).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].note, "Fixed it");

    // closed → cannot go to resolved directly
    db.change_bug_status(bug_id, uid, 90, 20, None).unwrap();
    assert!(db.change_bug_status(bug_id, uid, 80, 20, None).is_err());
}

#[test]
fn test_bug_relationships_and_duplicate_reverse() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db.create_user("dev", "Dev", "d@x.com", "pw", 55, true, false).unwrap();
    let b1 = db.create_bug(proj_id, uid, 1, "Bug 1", "", "", "", 30, 50, 70, "", "", "", "", 10).unwrap();
    let b2 = db.create_bug(proj_id, uid, 1, "Bug 2", "", "", "", 30, 50, 70, "", "", "", "", 10).unwrap();

    // add duplicate_of relationship → reverse must be auto-created
    db.add_bug_relationship(b2, b1, 1).unwrap(); // b2 is duplicate_of b1
    let rels = db.list_bug_relationships(b1).unwrap();
    assert!(rels.iter().any(|r| r.relationship_type == 2 && r.destination_bug_id == b2)); // b1 has_duplicate b2

    // add related_to → no reverse auto-created
    db.add_bug_relationship(b1, b2, 0).unwrap();
    let rels2 = db.list_bug_relationships(b2).unwrap();
    assert!(!rels2.iter().any(|r| r.source_bug_id == b2 && r.relationship_type == 0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_bug_status_workflow_and_history test_bug_relationships_and_duplicate_reverse`
Expected: FAIL

- [ ] **Step 3: Implement `change_bug_status`, `assign_bug`, `add_bugnote`, `delete_bugnote`, `list_bugnotes`, `list_bug_history`, `add_bug_relationship`, `delete_bug_relationship`, `list_bug_relationships` in `src/db.rs`**

`change_bug_status` must validate: if current `status == 90` (closed), only allow transition to `20` (feedback/reopen) — any other target returns `Err`. Record both `status` old/new and `resolution` old/new in history. If `note.is_some()`, call `add_bugnote` and record history type=1.

`add_bug_relationship`: for `rel_type == 1` (duplicate_of), also insert `(dest, src, 2)` (has_duplicate). For `rel_type == 2` also insert `(dest, src, 1)`. For other types, insert only the one record. `delete_bug_relationship`: if `rel_type` is `1` or `2`, find and delete the paired reverse row too.

- [ ] **Step 4: Run all tests to verify they pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(db): add bug status workflow, history, bugnotes, and relationships"
```

---

### Task 3: File Attachment Storage

**Files:**
- Modify: `src/db.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::BugFile`
- Produces:
  - `db::Db::add_bug_file(&self, bug_id: i64, user_id: i64, filename: &str, file_type: &str, content: Vec<u8>, upload_dir: &str) -> Result<i64, String>` (stores inline as BLOB when `upload_dir.is_empty()`, otherwise writes to `upload_dir/<uuid>` and stores path in `diskfile`)
  - `db::Db::get_bug_file(&self, file_id: i64, upload_dir: &str) -> rusqlite::Result<Option<(BugFile, Vec<u8>)>>` (reads BLOB or reads file from disk)
  - `db::Db::delete_bug_file(&self, file_id: i64, upload_dir: &str) -> rusqlite::Result<()>` (removes BLOB or deletes disk file)
  - `db::Db::list_bug_files(&self, bug_id: i64) -> rusqlite::Result<Vec<BugFile>>`

- [ ] **Step 1: Write failing tests for file attachment CRUD (inline BLOB mode)**

```rust
#[test]
fn test_bug_file_attachment_inline_and_cascade() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db.create_user("rep", "Rep", "r@x.com", "pw", 25, true, false).unwrap();
    let bug_id = db.create_bug(proj_id, uid, 1, "Bug", "", "", "", 30, 50, 70, "", "", "", "", 10).unwrap();

    let content = b"hello file".to_vec();
    let fid = db.add_bug_file(bug_id, uid, "test.txt", "text/plain", content.clone(), "").unwrap();
    assert!(fid > 0);

    let files = db.list_bug_files(bug_id).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "test.txt");
    assert_eq!(files[0].filesize, 10);

    let (_, fetched) = db.get_bug_file(fid, "").unwrap().unwrap();
    assert_eq!(fetched, content);

    db.delete_bug_file(fid, "").unwrap();
    assert!(db.list_bug_files(bug_id).unwrap().is_empty());

    // Cascade: deleting the bug removes all attachments
    let fid2 = db.add_bug_file(bug_id, uid, "test2.txt", "text/plain", b"data".to_vec(), "").unwrap();
    assert!(fid2 > 0);
    db.delete_bug(bug_id).unwrap();
    // No orphan rows remain (no way to query after cascade, but no error = pass)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_bug_file_attachment_inline_and_cascade`
Expected: FAIL

- [ ] **Step 3: Implement file attachment CRUD in `src/db.rs`**

Inline (BLOB) mode: `content` column stores bytes. Disk mode: write bytes to `{upload_dir}/{uuid4}`, store UUID filename in `diskfile` column, `content` is NULL. `delete_bug_file` in disk mode: `std::fs::remove_file` the disk path (log warning on error, don't panic).

- [ ] **Step 4: Run all tests to verify they pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(db): add bug file attachment storage (inline BLOB + disk modes)"
```

---

### Task 4: HTTP Handlers — View & Report Bugs

**Files:**
- Create: `src/handlers/bugs.rs`
- Modify: `src/lib.rs`
- Modify: `src/handlers/mod.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::*`, `auth::AuthContext`, `ui::render_layout`
- Produces:
  - `GET /view_all_bug_page` handler — renders paginated bug list for active project; uses `db.list_bugs(project_id, user_access, offset, limit)`. Sidebar link active.
  - `GET /view_bug_page/:id` handler — renders full bug detail with text, bugnotes (filtered by privacy), history, relationships, attachments, and quick-action forms (change status, assign, add note, add relationship, upload file). Returns HTTP 403 if private bug and `effective_access_level < 55`.
  - `GET /bug_report_page` handler — bug report form (requires `≥ Reporter (25)`); pre-populated with active project, categories, versions.
  - `POST /bug_report` handler — validates and creates bug; redirects to `/view_bug_page/:id` on success.
  - `build_router` additions in `src/lib.rs`

- [ ] **Step 1: Write failing HTTP integration tests in `tests/subproject2_http_test.rs`**

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login_as_admin(app: &axum::Router) -> String {
    // ... helper that POSTs /login and returns cookie header string
}

#[tokio::test]
async fn test_bug_report_and_view() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("TestProj", 10, 10, "", true).unwrap();
    let _ = db.create_project("P2", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let cookie = login_as_admin(&app).await;

    // Set active project
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri("/set_project")
        .header("cookie", &cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("project_id={}", proj_id))).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let proj_cookie = res.headers().get("set-cookie").unwrap().to_str().unwrap().to_string();
    let full_cookie = format!("{}; {}", cookie.split(';').next().unwrap(), proj_cookie.split(';').next().unwrap());

    // GET /bug_report_page
    let res = app.clone().oneshot(Request::builder()
        .uri("/bug_report_page").header("cookie", &full_cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // POST /bug_report
    let form = format!("project_id={}&category_id=1&summary=Test+Bug&description=Desc+here&steps_to_reproduce=&priority=30&severity=50&reproducibility=70&version=&view_state=10", proj_id);
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri("/bug_report")
        .header("cookie", &full_cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap().to_string();
    assert!(loc.contains("/view_bug_page/"));

    // GET /view_bug_page/:id
    let res = app.clone().oneshot(Request::builder()
        .uri(&loc).header("cookie", &full_cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Test Bug"));
    assert!(html.contains("Desc here"));

    // GET /view_all_bug_page
    let res = app.clone().oneshot(Request::builder()
        .uri("/view_all_bug_page").header("cookie", &full_cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8(body.to_vec()).unwrap().contains("Test Bug"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test subproject2_http_test`
Expected: FAIL

- [ ] **Step 3: Implement `view_all_bug_page`, `view_bug_page`, `bug_report_page`, `bug_report` handlers in `src/handlers/bugs.rs` and wire into `build_router` in `src/lib.rs`**

`view_all_bug_page`: table listing with columns: ID, Summary, Project, Category, Severity, Priority, Status, Updated, Handler. Paginate with `?page=N` query param (25 per page). Show "No issues" when list empty.
`view_bug_page`: shows all fields; shows bugnotes below; shows history at bottom; shows relationship list; shows attachment list with download links. Shows forms to add note, change status, assign, add relationship, upload file. Returns 403 if private bug and insufficient access.
`bug_report_page`: form with project selector (defaulted to active project), category dropdown (uses `list_effective_categories`), version dropdown, all bug fields.

- [ ] **Step 4: Run all unit and integration tests to verify they pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ tests/
git commit -m "feat(web): add view_all_bug_page, view_bug_page, and bug_report handlers"
```

---

### Task 5: HTTP Handlers — Update, Workflow Actions, Notes, Relationships & Attachments

**Files:**
- Modify: `src/handlers/bugs.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: all interfaces from Tasks 1–3
- Produces: All remaining HTTP routes from Spec Section 5.2:
  - `POST /update_bug/:id` — update bug fields (summary, category, priority, severity, version, target_version, fixed_in_version, reproducibility, view_state, handler_id, description, steps_to_reproduce, additional_information); records history for changed fields; requires `≥ Updater (40)` or own bug for Reporter.
  - `POST /bug_delete/:id` — delete bug; requires `≥ Manager (70)`; redirects to `/view_all_bug_page`.
  - `POST /bug_change_status/:id` — change status (form fields: `new_status`, `resolution`, `note`); access per Spec 4.1 (resolved `≥ 55`, closed `≥ 70`); redirects back to `/view_bug_page/:id`.
  - `POST /bug_assign/:id` — assign `handler_id`; requires `≥ Developer (55)`.
  - `POST /bugnote_add/:bug_id` — add bugnote; requires `≥ Reporter (25)`.
  - `POST /bugnote_delete/:id` — delete bugnote; requires `≥ Manager (70)` or note's own `reporter_id == user.id`.
  - `POST /bug_relationship_add/:bug_id` — add relationship; requires `≥ Developer (55)`.
  - `POST /bug_relationship_delete/:id` — delete relationship (and reverse for duplicates); requires `≥ Developer (55)`.
  - `POST /bug_file_add/:bug_id` — multipart upload (max `MANTIS_MAX_FILE_SIZE`); requires `≥ Reporter (25)`; returns HTTP 413 if too large.
  - `GET /file/bug/:id/:filename` — serve file (sets `Content-Type` from `file_type`; `Content-Disposition: attachment; filename=...`).
  - `POST /bug_file_delete/:id` — delete attachment; requires `≥ Manager (70)`.

- [ ] **Step 1: Write failing HTTP integration tests covering update, delete, notes, relationships, and attachments**

```rust
#[tokio::test]
async fn test_bug_workflow_actions_notes_relationships_attachments() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());
    let cookie = login_as_admin(&app).await;

    // Create a bug via POST /bug_report
    let full_cookie = set_project_cookie(&app, &cookie, proj_id).await;
    let bug_id = create_bug(&app, &full_cookie, proj_id).await;

    // Update bug summary
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri(&format!("/update_bug/{}", bug_id))
        .header("cookie", &full_cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("summary=Updated+Summary&category_id=1&priority=40&severity=60&reproducibility=70&view_state=10")).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Change status to resolved
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri(&format!("/bug_change_status/{}", bug_id))
        .header("cookie", &full_cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("new_status=80&resolution=20&note=Fixed")).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Add bugnote
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri(&format!("/bugnote_add/{}", bug_id))
        .header("cookie", &full_cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("note=Test+note&view_state=10")).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Upload attachment
    let boundary = "----boundary123";
    let body = format!("--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello file\r\n--{}--\r\n", boundary, boundary);
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri(&format!("/bug_file_add/{}", bug_id))
        .header("cookie", &full_cookie)
        .header("content-type", &format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body)).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify bug view shows updated summary, note, and attachment
    let res = app.clone().oneshot(Request::builder()
        .uri(&format!("/view_bug_page/{}", bug_id))
        .header("cookie", &full_cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec()
    ).unwrap();
    assert!(html.contains("Updated Summary"));
    assert!(html.contains("Test note"));
    assert!(html.contains("test.txt"));

    // Delete bug
    let res = app.clone().oneshot(Request::builder()
        .method("POST").uri(&format!("/bug_delete/{}", bug_id))
        .header("cookie", &full_cookie)
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test subproject2_http_test`
Expected: FAIL

- [ ] **Step 3: Implement all remaining bug handlers in `src/handlers/bugs.rs`**

For file upload (`bug_file_add`): use `axum::extract::Multipart` to read the first file field. Check size against `MANTIS_MAX_FILE_SIZE` env var (default `5242880`). If oversized before DB write, return HTTP 413. All handlers redirect to `/view_bug_page/:id` on success or show flash error on failure.

Access level checks per Spec Section 4.1. Use `html_escape` on all user content in HTML output.

- [ ] **Step 4: Run all tests to verify they pass**

Run: `cargo test`
Expected: PASS (all unit + integration tests green)

- [ ] **Step 5: Commit**

```bash
git add src/ tests/
git commit -m "feat(web): add bug update, delete, workflow, notes, relationships, and attachment handlers"
```
