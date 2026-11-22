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
- **Security & Auth:** In-house PBKDF2-HMAC-SHA256 built on `sha2` (`pbkdf2_sha256$<iters>$<salt>$<hex>`, OWASP default 600,000 iterations in release, 10,000 in debug, 1,000 in test; configurable via `MANTIS_PBKDF2_ITERATIONS`), legacy hash support (`sha256$...`, bare `salt$hash`, in-house 32-hex MD5, optional plaintext via `MANTIS_ALLOW_LEGACY_PLAINTEXT_PASSWORD`), transparent rehash on login (`needs_rehash`), constant-time hash comparisons and timing equalization against timing attacks, `rand` (v0.8), `AuthContext` web session/project cookies (`MANTIS_STRING_COOKIE`, `MANTIS_PROJECT_COOKIE`), `ApiAuth` Bearer PAT for `/api/rest/*`, and Bearer PAT authentication in `src/mcp/handlers.rs`
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

Code is organized into specialized MVC layers with 1:1 companion unit test files (`<module>.rs` + `<module>_test.rs`) across MVC and core modules; integration subsystems (`src/mailer.rs`, `src/mcp/*`) are covered by dedicated integration tests in `tests/` (`tests/test_sendgrid_mailer.rs`, `tests/mcp_test.rs`).

```text
src/
├── lib.rs                      # Library entry point, router delegator (controllers::build_router) & CLI commands (create-token)
├── main.rs                     # Binary entry point, TCP listener, env config & SendGrid worker
├── auth.rs (+ auth_test.rs)    # AuthContext (cookies), ApiAuth (Bearer PAT), PBKDF2/legacy password hashing
├── db.rs (+ db_test.rs)        # SQLite connection wrapper, schema bootstrap, queries
├── jira.rs (+ jira_test.rs)    # Jira Cloud REST API client & issue synchronization
├── mailer.rs                   # SendGrid email queue background dispatcher
├── mcp/                        # Model Context Protocol (MCP) SSE server & tool dispatcher
│   ├── mod.rs                  # /mcp/sse & /mcp/messages router (re-exports McpState & SessionManager)
│   ├── handlers.rs             # McpState, SSE & JSON-RPC HTTP handlers + PAT token auth
│   ├── protocol.rs             # JSON-RPC / MCP protocol types & error codes
│   ├── session.rs              # SessionManager for active SSE client streams
│   └── tools.rs                # Tool definitions & permission-checked DB dispatch
├── models/                     # [MODEL LAYER] Domain entities, enums, validation & DTOs
│   ├── mod.rs                  # Submodule exports & re-exports
│   ├── <domain>.rs             # Domain types (api, bug, custom_field, filter, jira, planning, project, tag, user)
│   └── models_test.rs          # Unit tests for domain models
├── controllers/                # [CONTROLLER LAYER] Axum request orchestration & routing
│   ├── mod.rs                  # Master router, CSRF middleware, security headers middleware & root dispatch (+ controllers_test.rs)
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
  - `MANTIS_ADDR`: Bind address and port (default: `127.0.0.1:3000`; also used as the fallback host for `MANTIS_BASE_URL`).
  - `MANTIS_BASE_URL`: Base URL for issue links in notification emails (`Db::resolve_base_url` in `src/db.rs`; defaults to `http://<MANTIS_ADDR>`).
  - `MANTIS_MAX_FILE_SIZE`: Maximum upload size in bytes for bug attachments (default: `5242880` = 5MB).
  - `MANTIS_UPLOAD_DIR`: Optional directory path for attachment disk storage (default: empty; attachments stored directly in database).
- **Security, Auth & Sessions:**
  - `MANTIS_COOKIE_SECURE`: When set to `1` or `true`, marks cookies as `Secure` and enables `Strict-Transport-Security` (HSTS: `max-age=31536000; includeSubDomains`).
  - `MANTIS_PUBLIC_ORIGIN`: Whitelisted public origin URL for CSRF validation behind reverse proxies / SSL termination (e.g. `https://mantis.example.com`).
  - `MANTIS_SESSION_TTL_SECS`: Inactivity timeout in seconds before session expires (default: `604800` = 7 days, minimum `60`).
  - `MANTIS_PBKDF2_ITERATIONS`: Custom iteration count for PBKDF2 password hashing (minimum `1000`; defaults to `600000` in release, `10000` in debug, `1000` in test).
  - `MANTIS_MAX_FAILED_LOGINS`: Consecutive failed login attempts before account lockout (default: `5`).
  - `MANTIS_LOCKOUT_SECS`: Account lockout duration in seconds after max failed attempts (default: `900` = 15 minutes).
  - `MANTIS_ALLOW_LEGACY_PLAINTEXT_PASSWORD`: Opt-in flag (`1` or `true`) to allow authentication against legacy plaintext passwords (default: false).
- **Integrations (Jira, SendGrid, MCP):**
  - `MANTIS_JIRA_API_TOKEN`: Optional server-side environment override for Jira API token (takes precedence over token stored in database).
  - `MANTIS_ALLOW_LOCAL_JIRA`: Opt-in flag (`1` or `true`) to allow Jira integration to connect to local/private IP addresses (SSRF guard, default: false).
  - `MANTIS_ALLOW_QUERY_TOKEN`: Set to `1` or `true` to allow `?token=` query auth on `/mcp/sse` for local dev/testing (disabled by default; prefer `Authorization: Bearer <token>`).
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
     - For global admin actions (custom fields, Jira settings, user admin), check `< 90` (Administrator); for manager actions (project management), check `< 70` (Manager). Return `views::render_access_denied_page(&ctx)` or the per-controller `forbidden(&ctx, msg)` / `forbidden_html` / `require_manager` helper returning HTTP 403 Forbidden:
       ```rust
       if ctx.user.access_level < 90 {
           return forbidden(&ctx, "Administrator access required");
       }
       ```
     - For project-scoped actions (issues, notes, attachments, Jira push, REST API, MCP tools), always check `db.effective_access_level(&ctx.user, project_id)` (or `&auth.user` / `&user`) so project-specific overrides and private projects are respected.
   - Keep controllers thin: never build HTML markup directly in controllers; delegate all rendering to `src/views/` (note: controller-level `forbidden*` flash error helpers are accepted minor exceptions).
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
   - Replace `\r`, `\n`, and `\t` characters with spaces and truncate error messages placed into redirect `Location` headers or query strings to prevent header injection and `HeaderValue` panics (see `sanitize_error_message` in `src/controllers/jira.rs`; reuse or extract into a shared helper when other controllers need it).

3. **Robust Error Handling:**
   - Do **NOT** use `.unwrap()` or `.expect()` in production controller or MCP paths. Propagate errors with `?` or handle them gracefully with appropriate HTTP status, JSON-RPC error, or user notification.

4. **Architecture & Test Separation:**
   - Maintain strict MVC separation: no DB calls or HTTP logic in views; no HTML markup generation in controllers (minor exceptions: controller-level `forbidden*` flash-error helpers).
   - Every new MVC or core module must maintain its companion `*_test.rs` file (or dedicated integration tests in `tests/` for integration subsystems).

5. **Dependencies Policy:**
   - Do **NOT** add external dependencies to `Cargo.toml` without explicit user request or confirmation. Favor the Rust standard library and existing crates.

6. **Security Middleware & Protections (CSRF, Headers, Sessions, Lockout):**
   - **CSRF Protection:** Handled by `csrf_protection_middleware` in `src/controllers/mod.rs` on state-changing methods (`POST`, `PUT`, `PATCH`, `DELETE`). Validates `Sec-Fetch-Site` (`same-origin`/`none`), and checks `Origin` / `Referer` against `X-Forwarded-Host` (taking precedence when present), `Host`, and `MANTIS_PUBLIC_ORIGIN`. For AJAX / JS `fetch` calls, include the `X-CSRF-Token` header (read from `<meta name="csrf-token" content="...">` rendered by `views/layout.rs`). Note that CSRF checks run only when origin/referer/token headers are supplied, and `/api/rest/*` and `/mcp*` routes are exempt.
   - **Security Headers & CSP:** `security_headers_middleware` in `src/controllers/mod.rs` sets `X-Frame-Options: SAMEORIGIN`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, Content Security Policy (restricts scripts and styles to `'self'` and `'unsafe-inline'`, preventing external CDN dependencies), and HSTS when `MANTIS_COOKIE_SECURE=1`.
   - **Session Hardening & Lifecycle:** Session cookies (`MANTIS_STRING_COOKIE` / `mantis_string`) are marked `HttpOnly`, `SameSite=Lax`, and `Secure` (when configured). Sessions expire after `MANTIS_SESSION_TTL_SECS` of inactivity. The server-side cookie string is rotated on login (and upon logout, invalidating the active session while browser cookies are cleared with `Max-Age=0`).
   - **Brute-Force & SSRF Guards:** Account lockout prevents brute-force password guessing via `MANTIS_MAX_FAILED_LOGINS` and `MANTIS_LOCKOUT_SECS`. Jira outbound HTTP client validates resolved IP addresses to block SSRF against private/loopback networks unless `MANTIS_ALLOW_LOCAL_JIRA=1`.

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
