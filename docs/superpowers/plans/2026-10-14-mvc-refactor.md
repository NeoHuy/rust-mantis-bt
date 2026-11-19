# MVC Architecture Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `rust-mantis-bt` into a clean Model-View-Controller (MVC) architecture with encapsulated controller routing, dedicated view rendering modules, and modular domain models while preserving 100% of existing behavior and tests.

**Architecture:** Split existing monolithic `handlers/`, `ui.rs`, and `models.rs` into three strictly separated layers: `src/models/` for domain structs/enums, `src/views/` for pure Rust HTML rendering functions, and `src/controllers/` for HTTP extractors, authorization guards, and routing definitions merged into a central router.

**Tech Stack:** Rust 2021, Axum 0.7, Tokio 1.0, Rusqlite 0.31, Serde.

**Spec:** `docs/superpowers/specs/2026-10-14-mvc-refactor-design.md`

## Global Constraints
- Preserve exact HTTP route paths, HTTP methods, and query parameter names.
- Preserve exact HTML form field names (`<input name="...">`, `<select name="...">`), buttons, and CSRF/action targets.
- Preserve exact REST API JSON payloads, status codes (200, 400, 403, 404, 500), and cookie names (`mantis_user`).
- No external template engine crate; all views use pure Rust HTML generation functions.
- All 34 unit tests and 13 integration tests must pass with zero regressions.

## Review Focus
- Permission regressions: Admin-only pages must return 403 Forbidden when accessed by users with access level < 90.
- Cookie guard regressions: Project switching and private project access control must retain identical cookie behaviors.
- Form field naming regressions: Input field names on edit/create forms must match the deserialization structs expected by controllers.
- HTML escaping: User inputs must be escaped via `views::html_escape` to prevent XSS across all converted views.
- Router route collisions: Sub-routers merged via `.merge()` must not create duplicate or conflicting route paths.

---

### Task 1: Modularize the Model Layer (`src/models/`)

**Files:**
- Create: `src/models/mod.rs`
- Create: `src/models/user.rs`
- Create: `src/models/project.rs`
- Create: `src/models/bug.rs`
- Create: `src/models/tag.rs`
- Create: `src/models/custom_field.rs`
- Create: `src/models/filter.rs`
- Create: `src/models/planning.rs`
- Create: `src/models/api.rs`
- Delete: `src/models.rs`

**Interfaces:**
- Produces: `crate::models::{AccessLevel, User, Project, ProjectStatus, ViewState, Category, Version, Bug, BugNote, BugHistory, BugRelationship, BugFile, BugMonitor, Tag, BugTag, CustomField, CustomFieldProject, CustomFieldValue, SavedFilter, FilterCriteria, News, EmailQueueItem, TimeTrackingSummary, ApiToken, PluginInfo}`

- [ ] **Step 1: Create `src/models/` directory and split domain models from `src/models.rs`**
  - Extract `AccessLevel`, `User`, `UserProfile` to `src/models/user.rs`.
  - Extract `Project`, `ProjectHierarchy`, `ProjectStatus`, `ViewState`, `Category`, `Version` to `src/models/project.rs`.
  - Extract `Bug`, `BugNote`, `BugHistory`, `BugRelationship`, `BugFile`, `BugMonitor` to `src/models/bug.rs`.
  - Extract `Tag`, `BugTag` to `src/models/tag.rs`.
  - Extract `CustomField`, `CustomFieldProject`, `CustomFieldValue` to `src/models/custom_field.rs`.
  - Extract `SavedFilter`, `FilterCriteria` to `src/models/filter.rs`.
  - Extract `News`, `EmailQueueItem`, `TimeTrackingSummary` to `src/models/planning.rs`.
  - Extract `ApiToken`, `PluginInfo`, REST payload structs to `src/models/api.rs`.
  - Set up `src/models/mod.rs` re-exporting all submodules with `pub use`. Port existing tests from `src/models.rs` (`test_access_level_conversion_and_strings`, `test_project_status_conversion_and_strings`, `test_view_state_conversion_and_strings`).

- [ ] **Step 2: Delete old `src/models.rs`**
  - Remove `src/models.rs`.

- [ ] **Step 3: Run unit tests to verify compilation and test passage**
  - Run: `cargo test --lib models::tests`
  - Expected: PASS (3 tests passed)

- [ ] **Step 4: Commit**
  ```bash
  git add src/models/
  git rm src/models.rs
  git commit -m "refactor(models): modularize domain models into src/models/"
  ```

---

### Task 2: Create View Foundation & Layout (`src/views/`)

**Files:**
- Create: `src/views/mod.rs`
- Create: `src/views/layout.rs`
- Modify: `src/lib.rs`
- Modify: `src/ui.rs` (forward to `src/views`)

**Interfaces:**
- Consumes: `crate::auth::AuthContext`, `crate::models::AccessLevel`
- Produces: `views::html_escape`, `views::format_timestamp`, `views::status_color`, `views::render_status_badge`, `views::render_status_legend`, `views::layout::render_layout`, `views::render_error_page`, `views::render_access_denied_page`, `views::render_not_found_page`

- [ ] **Step 1: Create `src/views/layout.rs`**
  - Move `CSS` constant and `render_layout` from `src/ui.rs` into `src/views/layout.rs`.

- [ ] **Step 2: Create `src/views/mod.rs`**
  - Move `html_escape`, `format_timestamp`, `status_color`, `render_status_badge`, `render_status_legend` from `src/ui.rs` into `src/views/mod.rs`.
  - Add standardized error page helpers: `render_error_page`, `render_access_denied_page`, `render_not_found_page`.
  - Expose `pub mod layout;`.

- [ ] **Step 3: Update `src/ui.rs` to re-export from `src/views` as a migration bridge**
  - In `src/ui.rs`, re-export everything from `crate::views` to avoid breaking any callers while migrating.
  - In `src/lib.rs`, declare `pub mod views;`.

- [ ] **Step 4: Run tests to verify compatibility**
  - Run: `cargo test`
  - Expected: PASS (all 47 tests pass)

- [ ] **Step 5: Commit**
  ```bash
  git add src/views/ src/ui.rs src/lib.rs
  git commit -m "feat(views): establish view foundation and master layout"
  ```

---

### Task 3: Migrate Auth & Users into MVC (`views/auth.rs`, `views/users.rs`, `controllers/auth.rs`, `controllers/users.rs`)

**Files:**
- Create: `src/views/auth.rs`
- Create: `src/views/users.rs`
- Create: `src/controllers/auth.rs`
- Create: `src/controllers/users.rs`
- Create: `src/controllers/mod.rs` (initial version)

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::User`, `crate::views::*`
- Produces: `controllers::auth::router() -> Router<Db>`, `controllers::users::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/auth.rs`**
  - Move HTML rendering for `/login` (`render_login_page`) and `/account_page` (`render_account_page`) from `handlers/auth.rs` into `src/views/auth.rs`.

- [ ] **Step 2: Create `src/views/users.rs`**
  - Move HTML rendering for `/manage_user_page` (`render_manage_users_page`), `/manage_user_create_page` (`render_user_create_page`), `/manage_user_edit_page` (`render_user_edit_page`) from `handlers/users.rs` into `src/views/users.rs`.

- [ ] **Step 3: Create `src/controllers/auth.rs`**
  - Port handlers: `login_page`, `login_submit`, `logout`, `set_project`, `account_page`, `account_update`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 4: Create `src/controllers/users.rs`**
  - Port handlers: `manage_user_page`, `manage_user_create_page`, `manage_user_create`, `manage_user_edit_page`, `manage_user_update`, `manage_user_delete`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 5: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src/views/auth.rs src/views/users.rs src/controllers/auth.rs src/controllers/users.rs
  git commit -m "refactor(mvc): migrate auth and users to controllers and views"
  ```

---

### Task 4: Migrate Projects, Categories & Versions (`views/projects.rs`, `controllers/projects.rs`)

**Files:**
- Create: `src/views/projects.rs`
- Create: `src/controllers/projects.rs`

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::{Project, Category, Version, User}`, `crate::views::*`
- Produces: `controllers::projects::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/projects.rs`**
  - Move HTML rendering for `/manage_proj_page` (`render_manage_proj_page`), `/manage_proj_create_page` (`render_proj_create_page`), `/manage_proj_edit_page` (`render_proj_edit_page` with subprojects, categories, versions, and user permissions panels) from `handlers/projects.rs`.

- [ ] **Step 2: Create `src/controllers/projects.rs`**
  - Port handlers: `manage_proj_page`, `manage_proj_create_page`, `manage_proj_create`, `manage_proj_edit_page`, `manage_proj_update`, `manage_proj_delete`, `manage_proj_subproj_add`, `manage_proj_subproj_delete`, `manage_proj_cat_add`, `manage_proj_cat_update`, `manage_proj_cat_delete`, `manage_proj_ver_add`, `manage_proj_ver_update`, `manage_proj_ver_delete`, `manage_proj_user_add`, `manage_proj_user_remove`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 3: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/views/projects.rs src/controllers/projects.rs
  git commit -m "refactor(mvc): migrate projects, categories and versions to controllers and views"
  ```

---

### Task 5: Migrate Bugs, Notes, Attachments & My View (`views/bugs.rs`, `controllers/bugs.rs`)

**Files:**
- Create: `src/views/bugs.rs`
- Create: `src/controllers/bugs.rs`

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::{Bug, BugNote, BugHistory, BugRelationship, BugFile}`, `crate::views::*`
- Produces: `controllers::bugs::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/bugs.rs`**
  - Move HTML rendering for `/view_all_bug_page` (`render_view_all_bug_page`), `/view_bug_page/:id` (`render_view_bug_page`), `/bug_report_page` (`render_bug_report_page`), `/update_bug_page/:id` (`render_update_bug_page`), `/my_view_page` (`render_my_view_page`) from `handlers/bugs.rs`.

- [ ] **Step 2: Create `src/controllers/bugs.rs`**
  - Port handlers: `view_all_bug_page`, `view_bug_page`, `bug_report_page`, `bug_report`, `update_bug_page`, `update_bug`, `bug_delete`, `bug_change_status`, `bug_assign`, `bugnote_add`, `bugnote_delete`, `bug_relationship_add`, `bug_relationship_delete`, `bug_file_add`, `bug_file_download`, `bug_file_delete`, `my_view_page`, `csv_export`, `bug_monitor_add`, `bug_monitor_delete`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 3: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/views/bugs.rs src/controllers/bugs.rs
  git commit -m "refactor(mvc): migrate bugs, notes, workflow and my view to controllers and views"
  ```

---

### Task 6: Migrate Tags, Custom Fields & Filters (`views/tags.rs`, `views/custom_fields.rs`, `controllers/tags.rs`, `controllers/custom_fields.rs`, `controllers/filters.rs`)

**Files:**
- Create: `src/views/tags.rs`
- Create: `src/views/custom_fields.rs`
- Create: `src/controllers/tags.rs`
- Create: `src/controllers/custom_fields.rs`
- Create: `src/controllers/filters.rs`

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::{Tag, CustomField, SavedFilter}`, `crate::views::*`
- Produces: `controllers::tags::router() -> Router<Db>`, `controllers::custom_fields::router() -> Router<Db>`, `controllers::filters::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/tags.rs` and `src/views/custom_fields.rs`**
  - Move HTML rendering for `/manage_tags_page` into `src/views/tags.rs`.
  - Move HTML rendering for `/manage_custom_field_page` and `/manage_custom_field_edit_page/:id` into `src/views/custom_fields.rs`.

- [ ] **Step 2: Create `src/controllers/tags.rs`**
  - Port handlers: `manage_tags_page`, `tag_create`, `tag_update`, `tag_delete`, `bug_tag_attach`, `bug_tag_detach`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 3: Create `src/controllers/custom_fields.rs`**
  - Port handlers: `manage_custom_field_page`, `manage_custom_field_create`, `manage_custom_field_edit_page`, `manage_custom_field_update`, `manage_custom_field_delete`, `manage_custom_field_proj_add`, `manage_custom_field_proj_remove`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 4: Create `src/controllers/filters.rs`**
  - Port handlers: `filter_save`, `filter_delete`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 5: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src/views/tags.rs src/views/custom_fields.rs src/controllers/tags.rs src/controllers/custom_fields.rs src/controllers/filters.rs
  git commit -m "refactor(mvc): migrate tags, custom fields and filters to controllers and views"
  ```

---

### Task 7: Migrate Planning (Roadmap, Changelog, Summary, Billing, News, Email Queue) (`views/planning.rs`, `controllers/planning.rs`)

**Files:**
- Create: `src/views/planning.rs`
- Create: `src/controllers/planning.rs`

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::{News, EmailQueueItem}`, `crate::views::*`
- Produces: `controllers::planning::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/planning.rs`**
  - Move HTML rendering for `/roadmap_page`, `/changelog_page`, `/summary_page`, `/billing_page`, `/news_page`, `/manage_email_queue_page` from `handlers/planning.rs` into `src/views/planning.rs`.

- [ ] **Step 2: Create `src/controllers/planning.rs`**
  - Port handlers: `roadmap_page`, `changelog_page`, `summary_page`, `billing_page`, `news_page`, `news_add`, `news_delete`, `manage_email_queue_page`, `manage_email_queue_flush`, `manage_email_delete`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 3: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/views/planning.rs src/controllers/planning.rs
  git commit -m "refactor(mvc): migrate roadmap, changelog, summary, billing and news to controllers and views"
  ```

---

### Task 8: Migrate REST API, Tokens & Plugins (`controllers/api.rs`)

**Files:**
- Create: `src/views/api.rs` (for HTML pages: `api_tokens_page`, `manage_plugin_page`)
- Create: `src/controllers/api.rs`

**Interfaces:**
- Consumes: `crate::db::Db`, `crate::auth::AuthContext`, `crate::models::{ApiToken, PluginInfo}`, `crate::views::*`
- Produces: `controllers::api::router() -> Router<Db>`

- [ ] **Step 1: Create `src/views/api.rs`**
  - Move HTML rendering for `/api_tokens_page` and `/manage_plugin_page` from `handlers/api.rs` into `src/views/api.rs`.

- [ ] **Step 2: Create `src/controllers/api.rs`**
  - Port HTML token/plugin handlers: `api_tokens_page`, `api_token_create`, `api_token_revoke`, `manage_plugin_page`, `manage_plugin_toggle`.
  - Port REST API endpoints: `rest_users_me`, `rest_list_projects`, `rest_get_project`, `rest_list_issues`, `rest_create_issue`, `rest_get_issue`, `rest_update_issue`, `rest_delete_issue`, `rest_add_issue_note`.
  - Define `pub fn router() -> Router<Db>`.

- [ ] **Step 3: Verify build with cargo check**
  - Run: `cargo check`
  - Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/views/api.rs src/controllers/api.rs
  git commit -m "refactor(mvc): migrate REST API, token and plugin handlers to controllers and views"
  ```

---

### Task 9: Assemble Master Router, Clean Up Legacy Handlers & End-to-End Test Suite

**Files:**
- Create: `src/controllers/mod.rs` (completed master router)
- Modify: `src/lib.rs` (expose controllers, build_router, views, models)
- Delete: `src/handlers/` directory (all files)
- Delete: `src/ui.rs` (obsolete, replaced by `src/views`)

**Interfaces:**
- Produces: `crate::build_router(db: Db) -> Router`, `pub mod controllers`, `pub mod views`, `pub mod models`

- [ ] **Step 1: Implement `src/controllers/mod.rs`**
  - Define `parse_bool(opt: Option<&str>) -> bool`.
  - Define `async fn root_handler(ctx: AuthContext) -> impl IntoResponse`.
  - Implement `pub fn build_router(db: Db) -> Router` merging all controller routers:
    ```rust
    Router::new()
        .route("/", get(root_handler))
        .merge(auth::router())
        .merge(users::router())
        .merge(projects::router())
        .merge(bugs::router())
        .merge(filters::router())
        .merge(custom_fields::router())
        .merge(tags::router())
        .merge(planning::router())
        .merge(api::router())
        .with_state(db)
    ```

- [ ] **Step 2: Update `src/lib.rs`**
  - Expose `pub mod auth;`, `pub mod db;`, `pub mod models;`, `pub mod views;`, `pub mod controllers;`.
  - Define `pub fn build_router(db: db::Db) -> axum::Router { controllers::build_router(db) }`.
  - Provide a deprecated alias `pub use controllers as handlers;` if any test references `handlers`.

- [ ] **Step 3: Remove legacy `src/handlers/` and legacy `src/ui.rs`**
  - Verify all handlers are completely superseded by controllers and views.
  - Delete `src/handlers/` and `src/ui.rs`.

- [ ] **Step 4: Run full verification test suite**
  - Run: `cargo test`
  - Expected: PASS across all 34 unit tests and 13 integration tests (0 failures).

- [ ] **Step 5: Run cargo clippy / check for warnings**
  - Run: `cargo check`
  - Expected: PASS with 0 warnings.

- [ ] **Step 6: Final Commit**
  ```bash
  git add src/
  git commit -m "refactor(mvc): finalize master router and remove legacy handlers and ui modules"
  ```
