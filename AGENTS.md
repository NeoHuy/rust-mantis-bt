# AI Development Guidelines & Rules: rust-mantis-bt

This document defines the mandatory guidelines, architectural standards, and workflows for any AI assistant (including Antigravity, Claude, Codex, Copilot, and Cursor) contributing to the `rust-mantis-bt` codebase.

---

## 1. Project Overview & Tech Stack

`rust-mantis-bt` is a modern, high-performance Mantis Bug Tracker clone written in Rust.

- **Language & Edition:** Rust 2021 Edition
- **Web Framework:** `axum` (v0.7), `tokio` (v1, multi-threaded runtime)
- **Architecture:** Model-View-Controller (MVC) pattern with strict separation of concerns
- **Database:** `rusqlite` (v0.31, bundled SQLite) with idempotent schemas
- **Serialization:** `serde`, `serde_json`, `serde_urlencoded`
- **Integrations:** Jira Cloud REST API integration (`src/jira.rs`)
- **Security & Cryptography:** `sha2` (password hashing format `sha256$...`, legacy MD5 fallback support), session cookies (`mantis_string`)
- **UI:** Server-side rendered HTML (`src/views/`) using a modern MantisBT 2.x responsive layout
- **Testing:** Dedicated 1:1 companion unit test files (`*_test.rs`), `tower::ServiceExt` for HTTP integration tests in `tests/`

---

## 2. Git Workflow & Branching Strategy (MANDATORY)

Every AI agent working on this repository **MUST** adhere strictly to this branching and PR lifecycle.

### 2.1. Starting a Task (Branch Creation)
- **NEVER** commit directly to `main` or `develop`.
- **Base Branch:** Always branch off the latest remote `origin/develop`.
- **Checkout Procedure:**
  ```bash
  git fetch origin
  git checkout -b <type>/<short-description> origin/develop
  ```
  *(If working within an Antigravity or Git worktree environment, point the worktree to `origin/develop`)*.

### 2.2. Branch Naming Conventions
Use lowercase kebab-case with standard prefixes:
- `feat/<feature-name>`: New user-facing features or capabilities (e.g. `feat/jira-integration`, `feat/tag-filter`)
- `fix/<bug-name>`: Bug fixes (e.g. `fix/cookie-expiration`, `fix/subproject-cycle`)
- `refactor/<module-name>`: Code restructuring without feature addition (e.g. `refactor/controllers-bugs`, `refactor/views-layout`)
- `ci/<workflow-name>`: CI/CD or pipeline changes (e.g. `ci/optimize-github-workflow`)
- `docs/<doc-name>`: Documentation, specs, or guidelines (e.g. `docs/ai-development-guidelines`)
- `hotfix/<critical-fix>`: Emergency fixes for production (branched from `origin/main` only when explicitly instructed)

### 2.3. Commit Conventions
Follow [Conventional Commits](https://www.conventionalcommits.org/):
```text
<type>(<scope>): <concise description in present tense>
```
*Examples:*
- `feat(auth): add api token expiration support`
- `fix(db): prevent cyclic parent-child subproject relationship`
- `refactor(mvc): migrate bugs and notes to controllers and views`
- `test(views): add unit tests for project edit view`

### 2.4. Pull Request (PR) Requirements & Target Branch
- **Target / Base Branch for PR:**
  - For all regular tasks (`feat/*`, `fix/*`, `refactor/*`, `ci/*`, `docs/*`): **Target MUST be `develop`**.
  - **NEVER** open a PR targeting `main` for standard development tasks.
  - Releases from `develop` $\rightarrow$ `main` are handled as dedicated release PRs.
- **Pre-PR Quality Gate:**
  Before pushing and opening a PR, ensure local branch is rebased on `origin/develop` and passes all checks:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```
- **PR Content Structure:**
  - **Summary:** Concise explanation of the problem solved.
  - **Key Changes:** Grouped by architectural layer (Model, View, Controller, DB, Integration).
  - **Verification:** Proof that formatting, clippy, and all tests pass.

---

## 3. Repository Architecture & Code Layout

Code is cleanly organized into specialized MVC layers with dedicated 1:1 unit test companion files:

```text
src/
├── lib.rs                   # Library entry point, router assembly
├── main.rs                  # Binary entry point, TCP listener & environment config
├── auth.rs                  # AuthContext extractor, password hash, session checks
├── auth_test.rs             # Unit tests for authentication
├── db.rs                    # SQLite connection wrapper, schema bootstrap, queries
├── db_test.rs               # Unit tests for database queries and migrations
├── jira.rs                  # Jira Cloud REST API client and issue synchronization
├── jira_test.rs             # Unit tests for Jira integration client
├── models/                  # [MODEL LAYER] Domain entities, enums, type conversions & DTOs
│   ├── mod.rs               # Module exports and re-exports
│   ├── api.rs               # REST API, PAT, and token models
│   ├── bug.rs               # Bug, BugNote, Attachment, History, BugRelationship
│   ├── custom_field.rs      # Custom field definitions and project/bug bindings
│   ├── filter.rs            # Filter criteria, saved filters, sort orders
│   ├── jira.rs              # Jira configuration and issue mapping models
│   ├── planning.rs          # Roadmap, changelog, version summary, time tracking
│   ├── project.rs           # Project, subproject, category, version models
│   ├── tag.rs               # Tag and issue-tag mapping models
│   ├── user.rs              # User, access levels, profile models
│   └── models_test.rs       # Unit tests for domain models
├── controllers/             # [CONTROLLER LAYER] Axum request orchestration & routing
│   ├── mod.rs               # Router assembly, state injection, root dispatch
│   ├── controllers_test.rs  # General router and dispatcher tests
│   ├── api.rs               # REST API endpoints & token management
│   ├── api_test.rs          # Controller tests for API endpoints
│   ├── auth.rs              # Login, logout, account management
│   ├── auth_test.rs         # Controller tests for auth flows
│   ├── bugs.rs              # Issue lifecycle, notes, attachments, workflow
│   ├── bugs_test.rs         # Controller tests for bug operations
│   ├── custom_fields.rs     # Custom fields CRUD and management
│   ├── custom_fields_test.rs# Controller tests for custom fields
│   ├── filters.rs           # Search filters, saved queries, CSV export
│   ├── filters_test.rs      # Controller tests for filters & export
│   ├── jira.rs              # Jira settings and push synchronization handlers
│   ├── jira_test.rs         # Controller tests for Jira integration
│   ├── planning.rs          # Roadmap, changelog, summary, email queue
│   ├── planning_test.rs     # Controller tests for planning handlers
│   ├── projects.rs          # Projects, subprojects, categories, versions
│   ├── projects_test.rs     # Controller tests for project handlers
│   ├── tags.rs              # Tag creation, deletion, association
│   ├── tags_test.rs         # Controller tests for tag handlers
│   ├── users.rs             # User admin CRUD, access level assignments
│   └── users_test.rs        # Controller tests for user admin
└── views/                   # [VIEW LAYER] Server-side HTML renderers & templates
    ├── mod.rs               # View utilities (HTML escape, timestamp formatting, badges)
    ├── views_test.rs        # Unit tests for view utilities
    ├── layout.rs            # Main layout wrapper (navbar, breadcrumbs, sidebar)
    ├── layout_test.rs       # Unit tests for layout rendering
    ├── api.rs               # API documentation & token administration views
    ├── api_test.rs          # View tests for API pages
    ├── auth.rs              # Login and account view templates
    ├── auth_test.rs         # View tests for auth pages
    ├── bugs.rs              # Bug report, view, edit, workflow pages
    ├── bugs_test.rs         # View tests for bug templates
    ├── custom_fields.rs     # Custom field management views
    ├── custom_fields_test.rs# View tests for custom field templates
    ├── jira.rs              # Jira configuration and issue push modal views
    ├── jira_test.rs         # View tests for Jira views
    ├── planning.rs          # Roadmap, changelog, summary, email queue views
    ├── planning_test.rs     # View tests for planning pages
    ├── projects.rs          # Project management, categories, versions views
    ├── projects_test.rs     # View tests for project views
    ├── tags.rs              # Tag list and management views
    ├── tags_test.rs         # View tests for tag views
    ├── users.rs             # User management and creation views
    └── users_test.rs        # View tests for user management views
tests/                       # End-to-end HTTP integration tests (tower::ServiceExt)
docs/                        # Design specifications and implementation plans
```

---

## 4. Feature Development Lifecycle for AI

When tasked with implementing a feature or fixing a bug, follow this execution sequence:

1. **Understand & Spec:**
   - Review existing domain models in `src/models/` and database operations in `src/db.rs`.
   - Ensure the design fits cleanly into the MantisBT MVC domain model.

2. **Database & Schema (`src/db.rs`):**
   - All schema migrations must be idempotent (e.g. `CREATE TABLE IF NOT EXISTS`, column existence checks).
   - Add database methods on `Db` with explicit error handling (`rusqlite::Result`).
   - Add companion unit tests in `src/db_test.rs`.

3. **Domain Models (`src/models/`):**
   - Place structs and enums in the relevant domain file (`src/models/<domain>.rs`) and re-export in `src/models/mod.rs`.
   - Keep models clean with appropriate `serde` attributes.
   - Respect access control constants:
     - Viewer = 10, Reporter = 25, Updater = 40, Developer = 55, Manager = 70, Administrator = 90.
   - Add companion unit tests in `src/models/models_test.rs`.

4. **Controllers & Routing (`src/controllers/` & `src/auth.rs`):**
   - Controllers orchestrate HTTP requests: parse inputs, verify permissions, invoke DB queries, pass data to views, and return responses (`Html`, `Redirect`, `Json`, etc.).
   - Extract `AuthContext` on protected endpoints.
   - Enforce access level guards before performing state mutations:
     ```rust
     if ctx.user.access_level < 70 {
         return Err(...); // Or redirect with error
     }
     ```
   - Keep controllers thin: never build HTML markup directly in controllers; delegate all rendering to `src/views/`.
   - Each controller defines its own sub-router via `pub fn router() -> Router<Db>`, registered and merged in `src/controllers/mod.rs`.
   - Add companion unit tests in `src/controllers/<domain>_test.rs`.

5. **Views & Templates (`src/views/`):**
   - Server-side rendered HTML functions in `src/views/<domain>.rs`.
   - Render page content inside the standard shell using `views::layout::render_layout`.
   - Ensure form methods (`POST`), hidden inputs (e.g., project context, IDs), and action URLs match routes in `controllers/`.
   - Escape all user-generated content with `views::html_escape` to prevent XSS.
   - Add companion unit tests in `src/views/<domain>_test.rs`.

6. **Automated Testing:**
   - **Unit Tests:** Maintain 1:1 companion test files (`<module>_test.rs`) using `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;`.
   - **Integration Tests:** Add or extend integration tests in `tests/` using `tower::ServiceExt`.
   - Test both success and permission-denied / validation-error paths.

---

## 5. Security & Coding Standards

1. **SQL Parameterization (Zero SQL Injection):**
   - **ALWAYS** use parameterized queries with `rusqlite::params![...]`.
   - **NEVER** use string interpolation or formatting (`format!("SELECT ... WHERE id = {}", id)`) for SQL statements.

2. **Output Sanitization (XSS & CSV Injection):**
   - Escape user-generated content before rendering inside HTML templates using `views::html_escape`.
   - When generating CSV exports, sanitize fields starting with `=`, `+`, `-`, or `@` to prevent spreadsheet formula injection.

3. **Robust Error Handling:**
   - Do **NOT** use `.unwrap()` or `.expect()` in production controller paths. Propagate errors with `?` or handle them gracefully with appropriate HTTP error status or user notification.

4. **Architecture & Test Separation:**
   - Maintain strict MVC separation: no DB calls or HTTP logic in views; no HTML markup generation in controllers.
   - Every module must maintain its companion `*_test.rs` file.

5. **Dependencies Policy:**
   - Do **NOT** add external dependencies to `Cargo.toml` without explicit user request or confirmation. Favor the Rust standard library and existing crates.

---

## 6. Verification Checklist (Definition of Done)

Before declaring any task complete or submitting a PR, the AI **MUST** run and pass:

```bash
# 1. Code formatting
cargo fmt --check

# 2. Linter & static analysis with zero warnings
cargo clippy --all-targets --all-features -- -D warnings

# 3. Full test suite
cargo test --verbose
```
