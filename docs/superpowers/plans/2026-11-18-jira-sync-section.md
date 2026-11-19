# Jira Section & Status Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inline "Push to Jira" button with a dedicated "Jira Integration" panel on the issue detail page, supporting linking existing tickets or creating new ones, selecting Epics and Issue Types (Task/Story/Bug), bidirectional status synchronization, and unlinking.

**Architecture:** Extend `JiraClient` in `src/jira.rs` with `get_issue` and `list_epics` endpoints. Update `mantis_jira_links_table` schema to persist `jira_status`. Render a dedicated MantisBT panel in `src/views/jira.rs` and `src/views/bugs.rs`. Orchestrate Sync/Create/Unlink requests in `src/controllers/jira.rs` with atomic DB transactions that map Jira statuses to MantisBT statuses and append audit notes.

**Tech Stack:** Rust 2021 Edition, `axum` v0.7, `rusqlite` v0.31, `reqwest` v0.12, Atlassian Jira Cloud REST API v2, `mockito` for integration tests.

**Spec:** Conversational design approved on 2026-11-18 in brainstorming session.

## Global Constraints

- Never commit directly to `main` or `develop`. Work on `feat/jira-sync-section`.
- Target branch for Pull Request is `develop`.
- Maintain strict MVC layer separation:
  - Domain models in `src/models/jira.rs`
  - Database queries in `src/db.rs`
  - Request orchestration in `src/controllers/jira.rs` and `src/controllers/bugs.rs`
  - HTML views in `src/views/jira.rs` and `src/views/bugs.rs`
- 1:1 companion unit test files (`*_test.rs`) must accompany each module.
- Quality gate before completion:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

## Review Focus

1. **Jira Key Normalization:** Correctly extracts issue key whether user enters raw number (`123`), full key (`JBC-123`), or full browse URL (`https://site.atlassian.net/browse/JBC-123`).
2. **Status Mapping Resilience:** Case-insensitive fuzzy matching for custom Jira status names (e.g., "In Progress", "In Dev", "QA", "Done").
3. **Permission Checks:** Enforces `effective_access_level >= 55` on project level for both sync and unlink actions.
4. **Error Handling & Flash Messages:** Multi-line or API error responses from Jira must be sanitized and url-encoded to prevent header injection or panics.
5. **Atomic Transactions:** Linking, unlinking, bug status changes, and bugnote creation must occur within a single SQLite transaction.

---

## Tasks

### Task 1: Jira Models & Client Extensions

- [ ] **Step 1: Write failing unit tests for new Jira models and client methods**
  - Add tests in `src/jira_test.rs` for:
    - Deserialization of Jira issue detail response (`id`, `key`, `fields.summary`, `fields.issuetype.name`, `fields.status.name`).
    - Deserialization of Jira search response for Epics (`issues` array with `key` and `fields.summary`).
    - Status mapping function `map_jira_status_to_mantis`.
    - Key normalization helper `normalize_jira_key`.

- [ ] **Step 2: Run tests to verify failure**
  - Run `cargo test jira::tests` and confirm compilation/test failure.

- [ ] **Step 3: Implement domain models and JiraClient methods**
  - In `src/models/jira.rs`:
    - Add `JiraEpic { pub key: String, pub summary: String }`.
    - Add `JiraIssueDetails { pub id: String, pub key: String, pub summary: String, pub issue_type: String, pub status: String }`.
    - Update `JiraLink` struct with `pub jira_status: String`.
  - In `src/jira.rs`:
    - Implement `normalize_jira_key(input: &str, default_project_key: &str) -> String`.
    - Implement `map_jira_status_to_mantis(jira_status: &str) -> i32`.
    - Implement `JiraClient::get_issue(&self, config: &JiraResolvedConfig, key: &str) -> Result<JiraIssueDetails, JiraError>`.
    - Implement `JiraClient::list_epics(&self, config: &JiraResolvedConfig) -> Result<Vec<JiraEpic>, JiraError>`.
    - Update `JiraClient::create_issue` to support optional `epic_key: Option<&str>` (setting `fields.parent = { "key": epic_key }`).

- [ ] **Step 4: Run tests to ensure green**
  - Run `cargo test jira::tests` and verify all tests pass.

- [ ] **Step 5: Commit**
  - `git commit -am "feat(jira): add Jira models, epic search, issue details, and status mapper"`

---

### Task 2: Database Layer Updates

- [ ] **Step 1: Write failing unit tests for DB sync and unlink methods**
  - In `src/db_test.rs`:
    - Test `record_jira_sync_success` on new link insertion and existing link update.
    - Test that bug status and `mantis_bug_history_table` are updated when Jira status changes.
    - Test `delete_jira_link_with_note` removes link and adds audit bugnote.

- [ ] **Step 2: Run tests to verify failure**
  - Run `cargo test db::tests::test_jira` and verify failure.

- [ ] **Step 3: Implement database migrations and query methods**
  - In `src/db.rs`:
    - Update schema migration in `Db::migrate()` to ensure column `jira_status` exists on `mantis_jira_links_table` (via table info check or column check).
    - Update `create_jira_link` to persist `jira_status`.
    - Update `get_jira_link_by_bug_id` to read `jira_status`.
    - Implement `record_jira_sync_success(bug_id, jira_key, jira_url, issue_type, jira_status, user_id, username, is_new)`:
      - Inserts or updates `mantis_jira_links_table`.
      - Maps `jira_status` to Mantis status ID via `map_jira_status_to_mantis`.
      - Updates `mantis_bug_table` status and `last_updated`.
      - Inserts history entry in `mantis_bug_history_table` if status changed.
      - Inserts audit note into `mantis_bugnote_table`.
    - Implement `delete_jira_link_with_note(bug_id, user_id, username)`.

- [ ] **Step 4: Run tests to ensure green**
  - Run `cargo test db::tests` and verify all tests pass.

- [ ] **Step 5: Commit**
  - `git commit -am "feat(db): update Jira link schema with jira_status and atomic sync transaction"`

---

### Task 3: Views Layer Updates

- [ ] **Step 1: Write failing unit tests for Jira Section view rendering**
  - In `src/views/jira_test.rs` and `src/views/bugs_test.rs`:
    - Test rendering of Jira section when unlinked with developer access (shows key input, epic dropdown, issue type dropdown, Sync button).
    - Test rendering of Jira section when unlinked with viewer access (shows "No Jira ticket linked").
    - Test rendering of Jira section when linked (shows ticket key badge, status badge, Sync and Unlink buttons).
    - Test that the old push form is no longer rendered in actions row.

- [ ] **Step 2: Run tests to verify failure**
  - Run `cargo test views::jira::tests` and confirm failure.

- [ ] **Step 3: Implement view templates**
  - In `src/views/jira.rs`:
    - Implement `render_jira_section(ctx: &AuthContext, bug_id: i64, jira_link: Option<&JiraLink>, epics: &[JiraEpic], config_enabled: bool, eff: i32) -> String`.
  - In `src/views/bugs.rs`:
    - Remove old inline Push button from `render_bug_view_page` actions row.
    - Insert the rendered Jira section panel into the page layout (between Attached Files and Activities/Bugnotes).
    - Pass Jira section HTML to `render_layout`.

- [ ] **Step 4: Run tests to ensure green**
  - Run `cargo test views::` and verify all tests pass.

- [ ] **Step 5: Commit**
  - `git commit -am "feat(views): add Jira Integration panel and remove inline push form"`

---

### Task 4: Controllers Layer Updates

- [ ] **Step 1: Write failing unit tests for Jira Sync and Unlink endpoints**
  - In `src/controllers/jira_test.rs`:
    - Test `jira_sync` form deserialization.
    - Test developer permission guard on `jira_sync` and `jira_unlink`.
    - Test unlinked submit with key vs empty key.

- [ ] **Step 2: Run tests to verify failure**
  - Run `cargo test controllers::jira::tests` and confirm failure.

- [ ] **Step 3: Implement controller logic and routing**
  - In `src/controllers/bugs.rs`:
    - In `view_bug_page`, if `jira_conf.enabled`, query `client.list_epics(&jira_conf).await.unwrap_or_default()`.
    - Call `views::jira::render_jira_section` and pass into `render_bug_view_page`.
  - In `src/controllers/jira.rs`:
    - Define `JiraSyncForm` struct: `jira_key: Option<String>`, `epic_key: Option<String>`, `issue_type: Option<String>`.
    - Implement `sync_bug_with_jira`:
      - Verify permissions (`eff >= 55`).
      - If already linked: fetch ticket via `client.get_issue`, sync status & type into DB, redirect with success.
      - If not linked:
        - If `jira_key` is provided: normalize key, fetch ticket via `client.get_issue`, link and sync into DB, redirect with success.
        - If `jira_key` is empty: create new ticket via `client.create_issue` (with summary, desc, issue_type, epic_key), link and sync into DB, redirect with success.
    - Implement `unlink_bug_from_jira`:
      - Verify permissions (`eff >= 55`).
      - Call `db.delete_jira_link_with_note`.
      - Redirect with success.
    - Wire routes in `src/controllers/jira.rs::router()`:
      - `.route("/view_bug_page/:id/jira_sync", post(sync_bug_with_jira))`
      - `.route("/view_bug_page/:id/jira_unlink", post(unlink_bug_from_jira))`
      - `.route("/view_bug_page/:id/jira_push", post(sync_bug_with_jira))` (legacy alias)

- [ ] **Step 4: Run tests to ensure green**
  - Run `cargo test controllers::jira::tests` and verify all tests pass.

- [ ] **Step 5: Commit**
  - `git commit -am "feat(controllers): implement jira_sync and jira_unlink handlers"`

---

### Task 5: Integration Tests & Quality Verification

- [ ] **Step 1: Update & extend integration tests in `tests/test_jira_push_integration.rs`**
  - Add test: `test_jira_sync_existing_ticket_by_key_updates_mantis_status`.
  - Add test: `test_jira_sync_create_new_ticket_with_epic`.
  - Add test: `test_jira_sync_refresh_existing_link`.
  - Add test: `test_jira_unlink_removes_link_and_appends_note`.
  - Add test: `test_jira_sync_permission_denied_for_reporter`.

- [ ] **Step 2: Run full integration tests**
  - Run `cargo test --test test_jira_push_integration`.

- [ ] **Step 3: Run the mandatory verification checklist**
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --verbose`

- [ ] **Step 4: Commit and push**
  - Commit final integration tests.
  - Push branch `feat/jira-sync-section` to remote.
