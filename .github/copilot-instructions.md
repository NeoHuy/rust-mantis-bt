# GitHub Copilot & Codex Instructions: rust-mantis-bt

This file instructs GitHub Copilot, Copilot Workspace, and OpenAI Codex on project rules, architecture, and coding standards.

See [AGENTS.md](../AGENTS.md) for full project rules and architecture.

---

## Key Directives for Copilot & Codex

### 1. Git Workflow & Branching
- **Base Branch:** All new branches must be checked out from `origin/develop` (after running `git fetch origin`).
- **Never commit directly** to `main` or `develop`.
- **Branch Naming:** `feat/<name>`, `fix/<name>`, `refactor/<name>`, `ci/<name>`, `docs/<name>`.
- **PR Target:** All Pull Requests must target `develop` (never `main`).

### 2. Architecture & Patterns (MVC)
- **Controllers (`src/controllers/`):** Axum 0.7 request orchestration and routing. Sub-routers in domain files (`src/controllers/*.rs`) merged in `src/controllers/mod.rs`. Handlers parse requests, verify permissions, call DB methods, invoke views, and return responses.
- **Views (`src/views/`):** Server-side rendered HTML templates and layout (`src/views/layout.rs`). Render page content and escape user input with `views::html_escape` to prevent XSS.
- **Models (`src/models/`):** Pure domain structs, enums, validation, and DTOs partitioned by domain (`src/models/*.rs`) and re-exported in `src/models/mod.rs`.
- **Data Layer (`src/db.rs`):** SQLite with bundled `rusqlite`. Queries must **always** use parameterized `rusqlite::params![...]` (no SQL string formatting). Migrations must be idempotent.
- **Security & Permissions:** Access levels in `src/auth.rs` and `src/models/user.rs` (Viewer=10, Reporter=25, Updater=40, Developer=55, Manager=70, Administrator=90). Guard controllers with `ctx.user.access_level`.
- **Integrations:** Jira Cloud REST API client in `src/jira.rs`, with dedicated controllers and views.
- **Testing Layout:** Maintain 1:1 companion unit test files (`*_test.rs`) alongside each module using `#[cfg(test)] #[path = "<name>_test.rs"] mod tests;`. HTTP integration tests live in `tests/`.

### 3. Rust Code Standards
- Avoid `.unwrap()` / `.expect()` in controller code or production paths; propagate errors with `?` or return clean error responses.
- Keep dependencies lean. Do not add new crates to `Cargo.toml` without explicit instruction.

### 4. Verification Gate
Before finishing any change, verify that:
1. `cargo fmt --check` succeeds.
2. `cargo clippy --all-targets --all-features -- -D warnings` has 0 warnings.
3. `cargo test` passes 100%.
