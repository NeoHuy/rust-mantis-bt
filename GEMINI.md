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

2. **MVC Architecture & 1:1 Test Layout:**
   - Adhere strictly to the MVC layer separation:
     - Domain entities in `src/models/`
     - Request orchestration & routing in `src/controllers/`
     - HTML presentation in `src/views/`
   - Maintain 1:1 companion unit test files (`*_test.rs`) alongside each module using `#[cfg(test)] #[path = "<name>_test.rs"] mod tests;`.

3. **Quality Verification Before Completion:**
   - Run the mandatory trio before finalizing any task:
     ```bash
     cargo fmt --check
     cargo clippy --all-targets --all-features -- -D warnings
     cargo test
     ```

4. **Code Safety & Parameterization:**
   - Parameterize all SQL queries using `rusqlite::params!`.
   - Never construct raw SQL query strings using formatting.
   - Never use `.unwrap()` or `.expect()` in route controllers or production code.
