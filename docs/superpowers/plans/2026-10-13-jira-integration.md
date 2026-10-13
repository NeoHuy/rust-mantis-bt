# Jira Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow developers to triage MantisBT issues and push them directly to Atlassian Jira Cloud as new issue tickets, creating reciprocal links and updating Mantis status.

**Architecture:** A new SQLite table `mantis_jira_links_table` stores 1-to-1 links between Mantis bugs and Jira tickets, while `mantis_config_table` holds global Jira connection credentials and per-project Jira Project Key mappings. A dedicated HTTP client module `src/jira.rs` communicates with Jira Cloud REST API v2/v3, and Axum handlers manage the admin settings page (`/manage_jira_page`) and the bug triage push action (`/view_bug_page/:id/jira_push`).

**Tech Stack:** Rust 2021, Axum 0.7, Tokio 1, rusqlite 0.31, reqwest 0.12 (json, rustls-tls), serde & serde_json.

**Spec:** `docs/superpowers/specs/2026-10-13-jira-integration-design.md`

## Global Constraints

- Must compile with Rust edition 2021 and existing project dependencies.
- HTTP client requests to Jira must use a 10-second timeout.
- Authentication with Jira Cloud must use HTTP Basic Auth with `email:api_token`.
- Mantis bug status transition on successful push must update to `acknowledged` (status 30).
- Only users with Developer or higher permission (`access_level >= 55`) can push bugs to Jira.
- Only administrators (`access_level >= 90`) can configure Jira integration settings.

## Review Focus

1. **Jira credential/network failure:** When Jira API returns 401, 400, or times out, Mantis bug state and bugnotes must not be modified, and a clear error message must be shown.
2. **Duplicate issue creation:** If a bug already has a linked Jira ticket, attempting to push again must be rejected idempotently.
3. **Missing Jira configuration:** If Jira URL, token, or project mapping is missing when a push is attempted, the user receives an informative configuration error instead of a crash.
4. **Deleted bug cascade:** When a bug is deleted from MantisBT, its Jira link record in `mantis_jira_links_table` must be cleanly deleted by cascade foreign key.
5. **Authorization barrier:** Users with Reporter or Viewer role (`access_level < 55`) must not be able to trigger Jira issue creation.

---

### Task 1: Add Dependencies, JiraLink Model, Database Schema & DB Methods

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/models.rs`
- Modify: `src/db.rs`
- Test: `src/db.rs` (unit tests)

**Interfaces:**
- Produces:
  - `JiraLink` struct in `src/models.rs`:
    ```rust
    pub struct JiraLink {
        pub id: i64,
        pub bug_id: i64,
        pub jira_key: String,
        pub jira_url: String,
        pub issue_type: String,
        pub created_by: i64,
        pub created_at: i64,
    }
    ```
  - `Db` methods in `src/db.rs`:
    ```rust
    pub fn create_jira_link(&self, bug_id: i64, jira_key: &str, jira_url: &str, issue_type: &str, user_id: i64) -> rusqlite::Result<i64>;
    pub fn get_jira_link_by_bug_id(&self, bug_id: i64) -> rusqlite::Result<Option<JiraLink>>;
    pub fn delete_jira_link_by_bug_id(&self, bug_id: i64) -> rusqlite::Result<bool>;
    pub fn get_jira_config(&self, project_id: i64) -> rusqlite::Result<JiraResolvedConfig>;
    pub fn save_jira_global_config(&self, enabled: bool, base_url: &str, email: &str, api_token: Option<&str>, default_issue_type: &str, default_project_key: &str) -> rusqlite::Result<()>;
    pub fn set_jira_project_mapping(&self, project_id: i64, jira_project_key: &str) -> rusqlite::Result<()>;
    ```

- [ ] **Step 1: Write the failing tests in `src/db.rs`**

Add unit tests `test_jira_links_crud` and `test_jira_config_resolution` in `src/db.rs` asserting:
1. `create_jira_link` creates a link and `get_jira_link_by_bug_id` retrieves it with matching fields.
2. Duplicate `create_jira_link` for same `bug_id` fails due to unique constraint.
3. `save_jira_global_config` and `set_jira_project_mapping` persist and resolve correctly in `get_jira_config`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_jira_links_crud`
Expected: Compilation failure or missing methods.

- [ ] **Step 3: Implement dependencies, models, and DB methods**

1. Add `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` to `Cargo.toml`.
2. Add `JiraLink` and `JiraResolvedConfig` to `src/models.rs`.
3. Add `CREATE TABLE IF NOT EXISTS mantis_jira_links_table` to `Db::migrate()` in `src/db.rs`.
4. Implement CRUD and config helpers in `src/db.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_jira_links_crud && cargo test test_jira_config_resolution`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/models.rs src/db.rs
git commit -m "feat(db): add Jira links table, models, and config helpers"
```

---

### Task 2: Jira REST API Client Module

**Files:**
- Create: `src/jira.rs`
- Modify: `src/lib.rs`
- Test: `src/jira.rs` (unit tests)

**Interfaces:**
- Consumes: `JiraResolvedConfig` from `src/models.rs`.
- Produces:
  ```rust
  pub struct JiraClient {
      client: reqwest::Client,
  }
  impl JiraClient {
      pub fn new() -> Self;
      pub async fn test_connection(&self, base_url: &str, email: &str, api_token: &str) -> Result<JiraMyselfResponse, JiraError>;
      pub async fn create_issue(&self, config: &JiraResolvedConfig, summary: &str, description: &str, issue_type: &str) -> Result<JiraCreateIssueResponse, JiraError>;
  }
  ```

- [ ] **Step 1: Write the failing tests in `src/jira.rs`**

Add unit tests:
1. `test_basic_auth_header_format`: Verifies base64 encoding of credentials.
2. `test_issue_payload_serialization`: Asserts JSON structure for `/rest/api/2/issue`.
3. `test_response_deserialization`: Asserts parsing of Jira issue key and browse URL calculation.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test jira::tests`
Expected: FAIL (module or functions not found).

- [ ] **Step 3: Implement `src/jira.rs` and expose in `src/lib.rs`**

1. Implement `JiraClient` with `reqwest::Client` (10s timeout).
2. Implement Basic Auth builder `format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", email, token)))` or using reqwest `.basic_auth(email, Some(token))`.
3. Implement `test_connection` against `GET {base_url}/rest/api/3/myself`.
4. Implement `create_issue` against `POST {base_url}/rest/api/2/issue`.
5. Map HTTP error status codes into descriptive `JiraError`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test jira::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/jira.rs src/lib.rs
git commit -m "feat(jira): implement Jira REST API client and unit tests"
```

---

### Task 3: Admin Jira Configuration Page & Project Mapping

**Files:**
- Create: `src/handlers/jira.rs`
- Modify: `src/handlers/mod.rs`
- Modify: `src/ui.rs`
- Test: `tests/test_jira_admin.rs`

**Interfaces:**
- Consumes: `Db`, `JiraClient`, `AuthContext`.
- Produces:
  - `GET /manage_jira_page`
  - `POST /manage_jira_config`
  - `POST /manage_jira_test`

- [ ] **Step 1: Write the failing test in `tests/test_jira_admin.rs`**

Add tests:
1. `test_admin_jira_access_denied_for_developer`: Non-admin users receive 403 on `/manage_jira_page`.
2. `test_admin_jira_save_config`: Admin posts form updating Jira global config and per-project mapping.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_jira_admin`
Expected: FAIL (routes or handlers missing).

- [ ] **Step 3: Implement handlers and UI**

1. Create `src/handlers/jira.rs` with `manage_jira_page`, `manage_jira_config`, and `manage_jira_test`.
2. Render global config form (URL, email, token, defaults) and project mapping table.
3. Add "Manage Jira" link in admin navigation bar in `src/ui.rs`.
4. Wire routes in `src/handlers/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test test_jira_admin`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/handlers/jira.rs src/handlers/mod.rs src/ui.rs tests/test_jira_admin.rs
git commit -m "feat(jira): add Jira admin management page and project mapping"
```

---

### Task 4: Bug Action "Push to Jira", Badge Display & End-to-End Tests

**Files:**
- Modify: `src/handlers/jira.rs`
- Modify: `src/handlers/bugs.rs`
- Modify: `src/ui.rs`
- Test: `tests/test_jira_push_integration.rs`

**Interfaces:**
- Consumes: `Db`, `JiraClient`, `AuthContext`.
- Produces:
  - `POST /view_bug_page/:id/jira_push`
  - Jira badge on bug view and issue list.

- [ ] **Step 1: Write the failing test in `tests/test_jira_push_integration.rs`**

Add tests using mock HTTP server:
1. `test_jira_push_success`:
   - Developer submits push action.
   - Mock Jira returns 201 Created with key `PROJ-789`.
   - Bug status in DB updates to `acknowledged` (30).
   - Bugnote added in DB mentioning `PROJ-789`.
   - Link stored in `mantis_jira_links_table`.
2. `test_jira_push_already_linked`:
   - Duplicate push request is rejected.
3. `test_jira_push_permission_denied`:
   - Reporter (access level 25) receives 403.
4. `test_jira_push_jira_error_keeps_bug_state`:
   - Mock Jira returns 400 Bad Request.
   - Bug status is unchanged and no bugnote is created.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_jira_push_integration`
Expected: FAIL.

- [ ] **Step 3: Implement push handler and UI elements**

1. Implement `push_bug_to_jira` handler in `src/handlers/jira.rs`:
   - Validate `ctx.user.access_level >= 55`.
   - Check if bug is already linked in `mantis_jira_links_table`.
   - Resolve Jira config for the bug's project.
   - Call `JiraClient::create_issue`.
   - In DB transaction: insert Jira link, add bugnote, update status to `acknowledged`, add bug history entry.
2. In `src/handlers/bugs.rs` (`view_bug_page`):
   - If linked, render Jira badge: `<a href="{url}" target="_blank" class="badge badge-primary">Jira: {key} ↗</a>`.
   - If unlinked and Jira enabled, render "Push to Jira" action button and confirmation modal.
3. In `src/handlers/bugs.rs` (`view_all_bug_page`):
   - Render small Jira badge next to bug ID/summary if linked.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test test_jira_push_integration && cargo test`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/handlers/jira.rs src/handlers/bugs.rs src/ui.rs tests/test_jira_push_integration.rs
git commit -m "feat(jira): implement push to Jira triage action and badge display"
```
