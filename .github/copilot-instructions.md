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

### 2. Architecture & Patterns
- **Web Layer:** Axum 0.7 routes defined in `src/handlers/mod.rs` and implemented in domain files (`src/handlers/*.rs`).
- **Data Layer:** SQLite with `rusqlite` bundled in `src/db.rs`. Queries must **always** use `rusqlite::params![...]` (no SQL string formatting).
- **Security:** Access levels in `src/auth.rs` and `src/models.rs` (Viewer=10, Reporter=25, Updater=40, Developer=55, Manager=70, Administrator=90). Guard handlers with `ctx.user.access_level`.
- **UI:** Server-side rendered HTML in `src/ui.rs` (MantisBT 2.x responsive theme). Sanitize user input to prevent XSS.

### 3. Rust Code Standards
- Avoid `.unwrap()` / `.expect()` in handler code; return `Result` or handle errors cleanly.
- Keep dependencies lean. Do not add new crates to `Cargo.toml` without explicit instruction.

### 4. Verification Gate
Before finishing any change, verify that:
1. `cargo fmt --check` succeeds.
2. `cargo clippy --all-targets --all-features -- -D warnings` has 0 warnings.
3. `cargo test` passes 100%.
