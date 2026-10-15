# AI Development Guidelines & Rules: rust-mantis-bt

This document defines the mandatory guidelines, architectural standards, and workflows for any AI assistant (including Antigravity, Claude, Codex, Copilot, and Cursor) contributing to the `rust-mantis-bt` codebase.

---

## 1. Project Overview & Tech Stack

`rust-mantis-bt` is a modern, high-performance Mantis Bug Tracker clone written in Rust.

- **Language & Edition:** Rust 2021 Edition
- **Web Framework:** `axum` (v0.7), `tokio` (v1, multi-threaded runtime)
- **Database:** `rusqlite` (v0.31, bundled SQLite) with idempotent schemas
- **Serialization:** `serde`, `serde_json`, `serde_urlencoded`
- **Security & Cryptography:** `sha2` (password hashing format `sha256$...`, legacy MD5 fallback support), session cookies (`mantis_string`)
- **UI:** Server-side rendered HTML (`src/ui.rs`) using a modern MantisBT 2.x responsive layout
- **Testing:** `tower::ServiceExt` for HTTP integration tests, standard Rust unit tests

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
- `refactor/<module-name>`: Code restructuring without feature addition (e.g. `refactor/handlers-bugs`)
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
- `test(http): add integration test for issue workflow transitions`

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
  - **Key Changes:** Grouped by architectural layer (DB, Handlers, UI, Models).
  - **Verification:** Proof that formatting, clippy, and all tests pass.

---

## 3. Repository Architecture & Code Layout

Code is cleanly organized into specialized layers:

```text
src/
├── lib.rs              # Library entry point, router assembly
├── main.rs             # Binary entry point, TCP listener & environment config
├── models.rs           # Core domain structs, enums, type conversions
├── db.rs               # SQLite connection wrapper, schema bootstrap, queries
├── auth.rs             # AuthContext extractor, password hash, session checks
├── ui.rs               # SSR HTML layout builders, navigation, templates
└── handlers/           # Axum HTTP request handlers grouped by domain
    ├── mod.rs          # Route definitions and URL dispatching
    ├── auth.rs         # Login, logout, account management
    ├── users.rs        # User administration (CRUD, access levels)
    ├── projects.rs     # Projects, subprojects, categories, versions
    ├── bugs.rs         # Issues/bugs lifecycle, notes, attachments
    ├── filters.rs      # Saved filters, search, CSV export
    ├── custom_fields.rs# Custom fields definitions and bindings
    ├── tags.rs         # Tag management and issue tagging
    ├── planning.rs     # Roadmap, changelog, summary, time tracking
    └── api.rs          # REST API endpoints, personal access tokens
tests/                  # End-to-end HTTP integration tests (subproject*_http_test.rs)
docs/                   # Design specifications and implementation plans
```

---

## 4. Feature Development Lifecycle for AI

When tasked with implementing a feature or fixing a bug, follow this execution sequence:

1. **Understand & Spec:**
   - Review existing models in `src/models.rs` and database operations in `src/db.rs`.
   - Ensure the design fits into the existing MantisBT domain model.

2. **Database & Schema (`src/db.rs`):**
   - All schema migrations must be idempotent (e.g. `CREATE TABLE IF NOT EXISTS`, check column existence).
   - Add database methods on `Db` with explicit error handling (`rusqlite::Result`).

3. **Domain Models (`src/models.rs`):**
   - Keep models clean with appropriate `serde` attributes.
   - Respect access control constants:
     - Viewer = 10, Reporter = 25, Updater = 40, Developer = 55, Manager = 70, Administrator = 90.

4. **Handlers & Security (`src/handlers/` & `src/auth.rs`):**
   - Extract `AuthContext` on protected endpoints.
   - Enforce access level guards before performing state mutations:
     ```rust
     if ctx.user.access_level < 70 {
         return Err(...); // Or redirect with error
     }
     ```
   - Register new routes inside `src/handlers/mod.rs`.

5. **UI & Templates (`src/ui.rs`):**
   - Render server-side HTML maintaining the MantisBT 2.x responsive layout.
   - Ensure form methods (`POST`), hidden inputs (e.g., project context, IDs), and action URLs match `handlers/mod.rs`.

6. **Automated Testing:**
   - Add unit tests in module `tests` submodules or integration tests in `tests/`.
   - Test both success and permission-denied / validation-error paths.

---

## 5. Security & Coding Standards

1. **SQL Parameterization (Zero SQL Injection):**
   - **ALWAYS** use parameterized queries with `rusqlite::params![...]`.
   - **NEVER** use string interpolation/formatting (`format!("SELECT ... WHERE id = {}", id)`) for SQL statements.

2. **Output Sanitization (XSS & CSV Injection):**
   - Escape user-generated content before rendering inside HTML templates.
   - When generating CSV exports, sanitize fields starting with `=`, `+`, `-`, or `@` to prevent spreadsheet formula injection.

3. **Robust Error Handling:**
   - Do **NOT** use `.unwrap()` or `.expect()` in production handler paths. Propagate errors with `?` or handle them gracefully with appropriate HTTP error status or user notification.

4. **Dependencies Policy:**
   - Do **NOT** add external dependencies to `Cargo.toml` without explicit user request or confirmation. Favor Rust standard library and existing crates.

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
