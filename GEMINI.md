# Gemini & Antigravity Rules: rust-mantis-bt

@[AGENTS.md](AGENTS.md)

---

## Directives for Gemini & Antigravity Agents

1. **Git Isolation:**
   - Always verify current branch / worktree before modifying files.
   - For new tasks, base the branch or worktree on `origin/develop`:
     ```bash
     git fetch origin
     git checkout -b <type>/<task-name> origin/develop
     ```
   - Target PRs to `develop`, never `main`.

2. **Quality Verification Before Completion:**
   - Run the mandatory trio before finalizing any task:
     ```bash
     cargo fmt --check
     cargo clippy --all-targets --all-features -- -D warnings
     cargo test
     ```

3. **Code Safety:**
   - Parameterize all SQL queries using `rusqlite::params!`.
   - Never use `.unwrap()` in route handlers.
