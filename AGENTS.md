# AI Development Guidelines & Rules: rust-mantis-bt

This document defines the mandatory guidelines, architectural standards, and workflows for any AI assistant (including Antigravity, Claude, Codex, Copilot, and Cursor) contributing to the `rust-mantis-bt` codebase.

---

## 1. Project Overview & Tech Stack

`rust-mantis-bt` is a modern, high-performance Mantis Bug Tracker clone written in Rust.

- **Language & Edition:** Rust 2021 Edition
- **Web Framework:** `axum` (v0.7), `tokio` (v1, multi-threaded runtime), `tokio-stream` (v0.1, MCP SSE streaming)
- **Architecture:** Model-View-Controller (MVC) pattern with strict separation of concerns
- **Database:** `rusqlite` (v0.31, bundled SQLite) with idempotent schemas
- **Serialization & HTTP Client:** `serde`, `serde_json`, `reqwest` (v0.12 with `rustls-tls` for Jira and SendGrid integrations)
- **Integrations:** Jira Cloud REST API (`src/jira.rs`), SendGrid email queue dispatcher (`src/mailer.rs`), and Model Context Protocol (MCP) SSE server (`src/mcp/`, `/mcp/sse` & `/mcp/messages`)
- **Security & Auth:** `sha2` (password hashing format `sha256$...`, legacy MD5 fallback support), `rand` (v0.8), `AuthContext` web session/project cookies (`MANTIS_STRING_COOKIE`, `MANTIS_PROJECT_COOKIE`), `ApiAuth` Bearer PAT for `/api/rest/*`, and Bearer PAT authentication in `src/mcp/handlers.rs`
- **UI:** Server-side rendered HTML (`src/views/`) using a modern MantisBT 2.x responsive layout
- **Testing:** Dedicated 1:1 companion unit test files (`*_test.rs`), `tower::ServiceExt` and `http-body-util` for integration tests in `tests/`

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
  - Releases from `develop` → `main` are handled as dedicated release PRs.
- **Pre-PR Quality Gate:**
  Before pushing and opening a PR, ensure local branch is rebased on `origin/develop` and passes all checks (note: `.github/workflows/rust.yml` ignores `docs/**`, `**.md`, and `.gitignore`, so docs-only PRs skip GitHub Actions CI):
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

Code is organized into specialized MVC layers with 1:1 companion unit test files (`<module>.rs` + `<module>_test.rs`) across MVC and core modules:

```text
src/
├── lib.rs                      # Library entry point, build_router & CLI commands (create-token)
├── main.rs                     # Binary entry point, TCP listener, env config & SendGrid worker
├── auth.rs (+ auth_test.rs)    # AuthContext (cookies), ApiAuth (Bearer PAT), password hashing
├── db.rs (+ db_test.rs)        # SQLite connection wrapper, schema bootstrap, queries
├── jira.rs (+ jira_test.rs)    # Jira Cloud REST API client & issue synchronization
├── mailer.rs                   # SendGrid email queue background dispatcher
├── mcp/                        # Model Context Protocol (MCP) SSE server & tool dispatcher
│   ├── mod.rs                  # McpState & /mcp/sse, /mcp/messages router
│   ├── handlers.rs             # SSE & JSON-RPC HTTP handlers + PAT token auth
│   ├── protocol.rs             # JSON-RPC / MCP protocol types & error codes
│   ├── session.rs              # SessionManager for active SSE client streams
│   └── tools.rs                # Tool definitions & permission-checked DB dispatch
├── models/                     # [MODEL LAYER] Domain entities, enums, validation & DTOs
│   ├── mod.rs                  # Submodule exports & re-exports
│   ├── <domain>.rs             # Domain types (api, bug, custom_field, filter, jira, planning, project, tag, user)
│   └── models_test.rs          # Unit tests for domain models
├── controllers/                # [CONTROLLER LAYER] Axum request orchestration & routing
│   ├── mod.rs                  # Master router merging sub-routers & MCP router (+ controllers_test.rs)
│   └── <domain>.rs             # Handlers + `pub fn router() -> Router<Db>` (+ <domain>_test.rs)
└── views/                      # [VIEW LAYER] Server-side HTML renderers & templates
    ├── mod.rs                  # Shared helpers: html_escape, format_timestamp, status badges, error pages (+ views_test.rs)
    ├── layout.rs               # Main layout wrapper: navbar, breadcrumbs, sidebar (+ layout_test.rs)
    └── <domain>.rs             # SSR page templates (+ <domain>_test.rs)
tests/                          # HTTP/MCP/Jira/SendGrid integration tests (tower::ServiceExt) & manual_test.sh
docs/                           # Design specifications and implementation plans
```

### Runtime Environment Variables
- **Core & Storage:**
  - `MANTIS_DB_PATH`: SQLite database file path (default: `mantis.db`).
  - `MANTIS_ADDR`: Bind address and port (default: `127.0.0.1:3000`).
  - `MANTIS_MAX_FILE_SIZE`: Maximum upload size in bytes for bug attachments.
  - `MANTIS_UPLOAD_DIR`: Optional directory path for attachment storage.
- **MCP Server:**
  - `MANTIS_ALLOW_QUERY_TOKEN`: Set to `1` or `true` to allow `?token=` query auth on `/mcp/sse` for local dev/testing (disabled by default; prefer `Authorization: Bearer <token>`).
- **SendGrid Email Worker:**
  - `SENDGRID_API_KEY`: Enables the background email queue worker when set.
  - `SENDGRID_FROM_EMAIL`, `SENDGRID_FROM_NAME`, `SENDGRID_ENDPOINT`: Optional sender identity and API endpoint overrides.

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

4. **Controllers, Routing & Integrations (`src/controllers/`, `src/auth.rs`, `src/mcp/`):**
   - Controllers orchestrate HTTP requests: parse inputs, verify permissions, invoke DB queries, pass data to views, and return responses (`Html`, `Redirect`, `Response`, `Json`, etc.).
   - Extract `AuthContext` on protected web endpoints and `ApiAuth` (Bearer PAT) on `/api/rest/*` endpoints.
   - Enforce access level guards before performing state mutations:
     - For global admin/manager actions, check `ctx.user.access_level` and return `views::render_access_denied_page(&ctx)`:
       ```rust
       if ctx.user.access_level < 70 {
           return views::render_access_denied_page(&ctx);
       }
       ```
     - For project-scoped actions (issues, notes, attachments, Jira push, REST API, MCP tools), always check `db.effective_access_level(&ctx.user, project_id)` (or `&auth.user` / `&user`) so project-specific overrides and private projects are respected.
   - Keep controllers thin: never build HTML markup directly in controllers; delegate all rendering to `src/views/`.
   - Each controller defines its own sub-router via `pub fn router() -> Router<Db>`, registered and merged in `src/controllers/mod.rs`.
   - Add companion unit tests in `src/controllers/<domain>_test.rs`.

5. **Views & Templates (`src/views/`):**
   - Server-side rendered HTML functions in `src/views/<domain>.rs`.
   - Render page content inside the standard shell using `views::layout::render_layout`.
   - Reuse shared helpers in `src/views/mod.rs` (`render_error_page`, `render_access_denied_page`, `render_not_found_page`, `html_escape`, `format_timestamp`, `render_status_badge`).
   - Ensure form methods (`POST`), hidden inputs (e.g., project context, IDs), and action URLs match routes in `controllers/`.
   - Escape all user-generated content with `views::html_escape` to prevent XSS.
   - Add companion unit tests in `src/views/<domain>_test.rs`.

6. **Automated Testing:**
   - **Unit Tests:** Maintain 1:1 companion test files (`<module>_test.rs`) for MVC and core modules using `#[cfg(test)] #[path = "<module>_test.rs"] mod tests;`.
   - **Integration Tests:** Add or extend integration tests in `tests/` using `tower::ServiceExt` (covering HTTP flows, MCP SSE/tools, Jira, and SendGrid mailer).
   - Test both success and permission-denied / validation-error paths.

---

## 5. Security & Coding Standards

1. **SQL Parameterization (Zero SQL Injection):**
   - **ALWAYS** use parameterized queries with `rusqlite::params![...]`.
   - **NEVER** use string interpolation or formatting (`format!("SELECT ... WHERE id = {}", id)`) for SQL statements.

2. **Output Sanitization & HTTP/HTML Safety:**
   - Escape user-generated content before rendering inside HTML templates using `views::html_escape`.
   - When generating CSV exports, sanitize fields starting with `=`, `+`, `-`, or `@` to prevent spreadsheet formula injection.
   - Never nest `<form>` tags inside edit pages (verified by `test_no_nested_forms_in_edit_pages`).
   - Strip `\r` and `\n` characters from error messages placed into redirect `Location` headers or query strings to prevent header injection and `HeaderValue` panics.

3. **Robust Error Handling:**
   - Do **NOT** use `.unwrap()` or `.expect()` in production controller or MCP paths. Propagate errors with `?` or handle them gracefully with appropriate HTTP status, JSON-RPC error, or user notification.

4. **Architecture & Test Separation:**
   - Maintain strict MVC separation: no DB calls or HTTP logic in views; no HTML markup generation in controllers.
   - Every new MVC or core module must maintain its companion `*_test.rs` file (or dedicated integration tests in `tests/` for integration subsystems).

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
