# Claude Development Guidelines: rust-mantis-bt

This file instructs Claude Code and Anthropic models on project conventions, workflows, and standards for `rust-mantis-bt`.

@AGENTS.md

---

## Quick Reference Commands

- **Run all tests:** `cargo test`
- **Run single test:** `cargo test <test_name>`
- **Format code:** `cargo fmt`
- **Check format:** `cargo fmt --check`
- **Lint code:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Run dev server:** `cargo run` (Listens on `127.0.0.1:3000` by default)

---

## Critical Rules for Claude

1. **Git Checkout Rule:**
   - **NEVER** edit or commit directly on `main` or `develop`.
   - When starting any task, fetch and checkout a new branch from `origin/develop`:
     ```bash
     git fetch origin
     git checkout -b <feat|fix|refactor|ci|docs>/<task-name> origin/develop
     ```

2. **Pull Request Target Rule:**
   - All PRs for features, fixes, refactorings, or docs **MUST target `develop`**.
   - Do **NOT** target `main` unless executing an explicit production release.

3. **Database & SQL Rule:**
   - Use `rusqlite::params![...]` for all queries. Never construct raw SQL strings with formatting.
   - Keep migrations idempotent in `src/db.rs`.

4. **Verification Gate:**
   - Always run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` before claiming a task is done.
