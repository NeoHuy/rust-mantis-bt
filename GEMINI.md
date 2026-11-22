# Gemini & Antigravity Rules: rust-mantis-bt

@AGENTS.md

---

## Antigravity & Worktree Notes

- Always verify your active git branch or worktree is based on `origin/develop` before modifying files.
- Use `<type>/<name>` (see branch prefixes in `AGENTS.md` §2.2) and target PRs to `develop`, never `main`.

## Critical Rules (Fallback)

- **Git:** Never commit directly to `main` or `develop`; branch from `origin/develop` and target PRs to `develop`.
- **Architecture & Tests:** Enforce strict MVC separation (`src/models/`, `src/controllers/`, `src/views/`); maintain 1:1 `*_test.rs` unit test files for MVC/core modules and integration tests in `tests/` for integration subsystems (`src/mailer.rs`, `src/mcp/*`).
- **Safety:** Always parameterize SQL with `rusqlite::params![...]`; never use `.unwrap()` or `.expect()` in controller, MCP, or production paths.
- **Verification Gate:** Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` before completion.
