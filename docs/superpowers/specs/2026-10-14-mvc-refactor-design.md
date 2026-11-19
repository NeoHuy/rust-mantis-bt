# Rust MantisBT — MVC Architecture Refactoring Design Spec

**Date:** 2026-10-14  
**Status:** Approved by User  
**Scope:** Architectural Refactoring of `rust-mantis-bt` into Model-View-Controller (MVC) Pattern  

---

## 1. Context & Motivation

The `rust-mantis-bt` codebase is a complete 1:1 port of MantisBT built with Axum, Tokio, and SQLite (`rusqlite`). All 5 sub-projects are functionally complete with 47 unit and integration tests passing.

However, the codebase currently suffers from architectural coupling:
- **`src/handlers/*.rs`** mixes HTTP request handling (form/query parsing, cookies, redirects, error responses), domain/authorization checks, and hundreds of lines of inlined HTML string building (`format!(r#"..."#)`).
- **`src/ui.rs`** contains the outer layout and shared status badge logic, but page-specific UI code is scattered inside the handlers.
- **`src/models.rs`** is a single file containing all domain models, enums, and helpers across all 5 sub-projects.
- **`src/handlers/mod.rs`** registers 50+ routes in a single monolithic `build_router` function.

### Goals
1. **Clean Separation of Concerns**: Enforce strict Model-View-Controller separation.
   - **Models**: Pure domain structs, enums, validation, and data structures.
   - **Views**: Pure rendering functions producing HTML strings wrapped in `axum::response::Html`. No database calls or HTTP state manipulation.
   - **Controllers**: Thin HTTP orchestrators that parse requests, verify permissions, call database/domain operations, invoke views, and return responses.
2. **Encapsulated Routing**: Each controller defines its own `pub fn router() -> Router<Db>`, and `controllers/mod.rs` aggregates them via `.merge()`.
3. **100% Backward Compatibility & Test Parity**:
   - Zero changes to external HTTP behavior: identical URLs, methods, form input field names, status codes, cookies, and JSON formats.
   - Preserve all existing unit and integration tests (34 unit tests + 13 integration tests = 47 passing tests).
   - Zero additional external crate dependencies (no template engine crate added; pure Rust HTML rendering preserved).

---

## 2. Target Directory & Module Structure

```text
src/
├── main.rs                   # Server entrypoint (initializes DB, binds listener, launches Tokio runtime)
├── lib.rs                    # Library root: re-exports build_router, auth, db, models, controllers, views
├── auth.rs                   # Authentication context, password hashing, and cookie management
├── db.rs                     # SQLite Data Access Layer (retains existing queries and transactions)
│
├── controllers/              # [CONTROLLER LAYER] - HTTP Request Orchestration
│   ├── mod.rs                # Merges sub-routers, root redirect, and shared controller utilities (parse_bool)
│   ├── auth.rs               # /login, /logout, /set_project, /account_page, /account_update
│   ├── users.rs              # /manage_user_page, create, edit, update, delete
│   ├── projects.rs           # /manage_proj_page, create, edit, subprojects, categories, versions, users
│   ├── bugs.rs               # /view_all_bug_page, /view_bug_page, report, update, status, notes, files, CSV
│   ├── filters.rs            # /filter_save, /filter_delete
│   ├── custom_fields.rs      # /manage_custom_field_*, project bindings
│   ├── tags.rs               # /manage_tags_page, tag create/update/delete, attach/detach to bugs
│   ├── planning.rs           # /roadmap_page, /changelog_page, /summary_page, /billing_page, /news_*, email queue
│   └── api.rs                # /api_tokens_*, /manage_plugin_*, and /api/rest/* REST endpoints
│
├── views/                    # [VIEW LAYER] - HTML Presentation Logic
│   ├── mod.rs                # Shared view utilities: html_escape, format_timestamp, status_badge, error/forbidden pages
│   ├── layout.rs             # Master layout shell: header navbar, left sidebar, breadcrumb, CSS styling, footer
│   ├── auth.rs               # Login box template, Account settings page template
│   ├── users.rs              # Manage users table, create user form, edit user form
│   ├── projects.rs           # Manage projects table, create/edit project forms, category/version panels
│   ├── bugs.rs               # Issue list table, issue details view, report issue form, edit form, my view grid
│   ├── custom_fields.rs      # Custom field list, create/edit form, project link panel
│   ├── tags.rs               # Tag management table, create/edit modals
│   └── planning.rs           # Roadmap milestones, Changelog release notes, Summary charts, Billing table, News, Email queue
│
└── models/                   # [MODEL LAYER] - Domain Entities & Value Types
    ├── mod.rs                # Re-exports all submodules for complete backward compatibility
    ├── user.rs               # User, UserProfile, AccessLevel enum
    ├── project.rs            # Project, ProjectHierarchy, ProjectStatus, ViewState, Category, Version
    ├── bug.rs                # Bug, BugNote, BugHistory, BugRelationship, BugFile, BugMonitor
    ├── tag.rs                # Tag, BugTag
    ├── custom_field.rs       # CustomField, CustomFieldProject, CustomFieldValue
    ├── filter.rs             # SavedFilter, FilterCriteria
    ├── planning.rs           # News, EmailQueueItem, TimeTrackingSummary
    └── api.rs                # ApiToken, PluginInfo, REST API request/response structs
```

---

## 3. Layer Specifications & Interfaces

### 3.1 Controller Layer (`src/controllers/`)

#### Responsibilities:
1. Extract request state: `State(db): State<Db>`, `ctx: AuthContext`, `Path(...)`, `Query(...)`, `Form(...)`, `Json(...)`.
2. Guard access: Check `ctx.user.access_level` against required permissions (e.g., administrator >= 90, manager >= 70, reporter >= 25) or call `require_admin(&ctx)`.
3. Invoke data queries / mutations via `db.*`.
4. Forward the retrieved domain models and state to the corresponding `views::*` function.
5. Return HTTP responses (`Html<String>`, `Redirect`, `Response`, `Json<Value>`).

#### Sub-router Pattern:
Each controller exposes a `pub fn router() -> Router<Db>`:
```rust
// src/controllers/users.rs
pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_user_page", get(manage_user_page))
        .route("/manage_user_create_page", get(manage_user_create_page))
        .route("/manage_user_create", post(manage_user_create))
        .route("/manage_user_edit_page/:id", get(manage_user_edit_page))
        .route("/manage_user_update/:id", post(manage_user_update))
        .route("/manage_user_delete/:id", post(manage_user_delete))
}
```

The master router in `src/controllers/mod.rs` combines them:
```rust
// src/controllers/mod.rs
pub fn build_router(db: Db) -> Router {
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
}
```

### 3.2 View Layer (`src/views/`)

#### Responsibilities:
1. Accept domain model references (`&[User]`, `&Bug`, `&AuthContext`, form defaults, flash messages).
2. Escape all user-supplied input using `html_escape`.
3. Construct HTML markup using clean Rust formatting functions.
4. Wrap page content in `layout::render_layout(title, ctx, flash, &content)`.
5. Standardize error and forbidden pages:
   - `views::render_error_page(ctx, title, flash, message) -> Html<String>`
   - `views::render_access_denied_page(ctx) -> Response` (returns HTTP 403 Forbidden with standard denial message)
   - `views::render_not_found_page(ctx, item_name) -> Response` (returns HTTP 404 Not Found)

#### Example View Function:
```rust
// src/views/users.rs
pub fn render_manage_users_page(
    ctx: &AuthContext,
    users: &[User],
    search_query: &str,
    enabled_only: bool,
) -> Html<String> {
    let mut rows_html = String::new();
    for (idx, u) in users.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let lvl_str = AccessLevel::from_i32(u.access_level).map(|a| a.as_str()).unwrap_or("unknown");
        let enabled_badge = if u.enabled == 1 { r#"<span class="badge badge-success">enabled</span>"# } else { r#"<span class="badge badge-danger">disabled</span>"# };
        let protected_badge = if u.protected == 1 { r#"<span class="badge badge-warning">protected</span>"# } else { "" };

        rows_html.push_str(&format!(
            r#"<tr class="{row_class}">
                <td><a href="/manage_user_edit_page/{}">{}</a></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{enabled_badge}</td>
                <td>{protected_badge}</td>
                <td>
                    <a href="/manage_user_edit_page/{}" class="btn btn-secondary btn-sm">Edit</a>
                    <form action="/manage_user_delete/{}" method="POST" style="display:inline; margin-left:4px;" onsubmit="return confirm('Are you sure you want to delete this user?');">
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            u.id,
            html_escape(&u.username),
            html_escape(&u.realname),
            html_escape(&u.email),
            html_escape(lvl_str),
            u.id,
            u.id,
        ));
    }

    let search_val = html_escape(search_query);
    let enabled_checked = if enabled_only { "checked" } else { "" };
    let content = format!(
        r#"
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:14px;">
            <h2 class="section-title" style="margin:0; border:none;">Manage Users</h2>
            <a href="/manage_user_create_page" class="btn">Create New Account</a>
        </div>
        <div class="panel" style="margin-bottom:14px;">
            <div class="panel-body" style="padding:10px 14px;">
                <form method="GET" action="/manage_user_page" style="display:flex; align-items:center; gap:12px; flex-wrap:wrap;">
                    <label for="search">Search:</label>
                    <input type="text" name="search" id="search" value="{search_val}" placeholder="Filter by username, real name or email" style="width:250px;" />
                    <label style="display:flex; align-items:center; cursor:pointer;">
                        <input type="checkbox" name="filter_enabled" value="1" {enabled_checked} /> Enabled only
                    </label>
                    <button type="submit" class="btn btn-secondary">Apply Filter</button>
                    <a href="/manage_user_page" class="btn btn-secondary">Reset</a>
                </form>
            </div>
        </div>
        <table class="data-table">
            <thead>
                <tr>
                    <th>Username</th>
                    <th>Real Name</th>
                    <th>Email</th>
                    <th>Access Level</th>
                    <th>Status</th>
                    <th>Protection</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>{rows_html}</tbody>
        </table>
        "#
    );

    layout::render_layout("Manage Users", ctx, None, &content)
}
```

### 3.3 Model Layer (`src/models/`)

#### Responsibilities:
1. Provide strongly typed definitions for all MantisBT domain objects.
2. Group structs into domain submodules:
   - `user.rs`: `User`, `UserProfile`, `AccessLevel` enum (`VIEWER = 10` ... `ADMINISTRATOR = 90`) with unit conversions and string representations.
   - `project.rs`: `Project`, `ProjectHierarchy`, `ProjectStatus`, `ViewState`, `Category`, `Version`.
   - `bug.rs`: `Bug`, `BugNote`, `BugHistory`, `BugRelationship`, `BugFile`, `BugMonitor`.
   - `tag.rs`: `Tag`, `BugTag`.
   - `custom_field.rs`: `CustomField`, `CustomFieldProject`, `CustomFieldValue`.
   - `filter.rs`: `SavedFilter`, `FilterCriteria`.
   - `planning.rs`: `News`, `EmailQueueItem`, `TimeTrackingSummary`.
   - `api.rs`: `ApiToken`, `PluginInfo`, REST API DTOs.
3. `src/models/mod.rs` re-exports all structs and enums (`pub use user::*;`, `pub use project::*;`, etc.) and retains existing model unit tests (`tests::test_access_level_conversion_and_strings`, etc.) to guarantee 100% compatibility with callers.

### 3.4 Data Access Layer (`src/db.rs`)

`src/db.rs` acts as the repository / data layer:
- Manages SQLite schema migrations (`mantis_*_table`), connection pool / mutex, and prepared statement queries.
- Retains all public query and mutation methods (`list_users`, `create_user`, `get_bug`, `insert_bug`, etc.).
- Keeps all 28 DB unit tests passing without modification.

---

## 4. Migration & Transition Strategy

To ensure continuous test validity throughout refactoring, migration will execute in staged increments:

1. **Step 1: Refactor `src/models/`**
   - Create `src/models/` module directory.
   - Split `src/models.rs` into submodules (`user.rs`, `project.rs`, `bug.rs`, etc.).
   - Create `src/models/mod.rs` re-exporting all types and running model unit tests.
   - Verify `cargo test` passes.
2. **Step 2: Create `src/views/` and Migrate Base UI**
   - Create `src/views/` module directory.
   - Move shared helpers (`html_escape`, `format_timestamp`, `render_status_badge`, `status_color`, `render_status_legend`, `render_error_page`) into `views/mod.rs`.
   - Move outer layout shell and CSS into `views/layout.rs`.
   - Create `views/auth.rs`, `views/users.rs`, `views/projects.rs`, `views/bugs.rs`, `views/custom_fields.rs`, `views/tags.rs`, `views/planning.rs`.
   - Keep `src/ui.rs` as a thin re-export adapter during transition if needed, then fully deprecate.
3. **Step 3: Refactor `src/handlers/` into `src/controllers/`**
   - Create `src/controllers/`.
   - Migrate each handler module into its corresponding controller module, extracting inline HTML into its respective `views/*.rs` function and adding `pub fn router() -> Router<Db>`.
   - Implement `src/controllers/mod.rs` with aggregated `.merge()` routers.
   - Update `src/lib.rs` to expose `controllers` and `views` (and provide a fallback alias for `handlers` if any tests reference it).
4. **Step 4: Cleanup & Verification**
   - Remove deprecated `src/handlers/` and legacy `src/ui.rs`.
   - Run `cargo test` across all 34 unit tests and 13 integration tests.
   - Confirm full test suite passes with zero warnings or failures.

---

## 5. Verification & Acceptance Criteria

1. **Architecture Verification**:
   - `src/controllers/` contains only HTTP coordination logic and `router()` definitions; zero inlined HTML strings.
   - `src/views/` contains all HTML generation functions, layout, and CSS.
   - `src/models/` contains well-organized domain structs and enums.
   - `src/db.rs` contains database queries and connection logic.
2. **Quality & Test Verification**:
   - `cargo check` and `cargo test` complete with 0 errors and 0 warnings.
   - All 34 unit tests pass (`models::tests::*`, `auth::tests::*`, `db::tests::*`).
   - All 13 integration HTTP tests pass:
     - `subproject1_http_test.rs` (6 tests)
     - `subproject2_http_test.rs` (4 tests)
     - `subproject3_http_test.rs` (2 tests)
     - `subproject4_http_test.rs` (1 test)
     - `subproject5_http_test.rs` (1 test)
