# MantisBT Model Context Protocol (MCP) Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a native Model Context Protocol (MCP) Server over SSE/HTTP (`/mcp/sse`, `/mcp/messages`) within `rust-mantis-bt`, strictly binding AI operations to authenticated users and enforcing role-based access control (RBAC).

**Architecture:** Embedded Axum sub-router in `src/mcp/` handling JSON-RPC 2.0 over Server-Sent Events (SSE) and HTTP POST. Sessions are authenticated via API tokens and tied to MantisBT `User` entities. A tool dispatcher executes 8 core issue-tracking tools with fine-grained access checks (`effective_access_level`). Token creation is restricted to Reporter+ (25+) and administrators (90) gain system-wide token governance.

**Tech Stack:** Rust 2021, Axum 0.7 (`axum::response::sse`), Tokio 1, rusqlite 0.31, serde / serde_json, SHA-256.

**Spec:** `docs/superpowers/specs/2026-11-18-mcp-server-design.md`

## Global Constraints

- Axum 0.7 compatibility using built-in `axum::response::sse::Sse`.
- Zero unstable/external MCP SDK crates; native JSON-RPC 2.0 serialization via `serde_json`.
- Strict user identity: `reporter_id` for created issues and bugnotes must always equal the authenticated session user's ID.
- RBAC levels: Viewer (10), Reporter (25), Updater (40), Developer (55), Manager (70), Administrator (90).
- Token creation requires `access_level >= 25`.

## Review Focus

1. **Unauthenticated or Expired SSE Requests**: Handshake with invalid or missing token returns HTTP 401 without hanging the SSE stream.
2. **Access Exceeded in Tool Call**: Reporter calling `update_issue` on another user's issue or assigning a handler returns `{ isError: true }` with clear explanation instead of panicking or 500.
3. **Private Issue Confidentiality**: `search_issues` and `get_issue` never leak issues with `view_state = 50` to non-developers unless they are the reporter.
4. **Admin vs Non-Admin Token Revocation**: Normal users can only revoke their own tokens; Administrators can revoke any user's token.
5. **SSE Session Cleanup on Disconnect**: Closed SSE connections or inactive sessions (>30 min) do not leak memory in `SessionManager`.

---

### Task 1: Token Creation RBAC Gate & Admin Oversight in UI and DB

**Files:**
- Modify: `src/db.rs`
- Modify: `src/handlers/api.rs`
- Test: `tests/mcp_test.rs`

**Interfaces:**
- Consumes: `Db::authenticate_api_token`, `Db::create_api_token`, `Db::revoke_api_token`.
- Produces: `Db::list_all_api_tokens(&self) -> rusqlite::Result<Vec<(ApiToken, String, String)>>`.

- [ ] **Step 1: Write the failing test for token governance**
In `tests/mcp_test.rs`, add tests verifying:
- Viewer (access level 10) gets 403 on `GET /api_tokens_page` and `POST /api_token_create`.
- Reporter (access level 25) successfully creates an API token.
- Administrator (access level 90) sees tokens from all users via `db.list_all_api_tokens()` and can revoke them.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --test mcp_test`
Expected: FAIL (method `list_all_api_tokens` not found or 403 checks missing).

- [ ] **Step 3: Implement `list_all_api_tokens` in `src/db.rs`**
Add query joining `mantis_api_token_table` and `mantis_user_table` returning token plus `username` and `real_name`.

- [ ] **Step 4: Update `src/handlers/api.rs` with role checks & admin view**
- In `api_tokens_page` and `api_token_create`: check `ctx.user.access_level >= 25`; return 403 if lower.
- In `api_tokens_page`: when `ctx.user.access_level >= 90`, render the "System-Wide API Tokens" table.
- In `api_token_revoke`: check if `token.user_id == ctx.user.id || ctx.user.access_level >= 90`; return 403 if unauthorized.

- [ ] **Step 5: Run tests to verify they pass**
Run: `cargo test --test mcp_test`
Expected: PASS.

- [ ] **Step 6: Commit**
```bash
git add src/db.rs src/handlers/api.rs tests/mcp_test.rs
git commit -m "feat(auth): enforce Reporter role on API tokens and add admin token oversight"
```

---

### Task 2: CLI Token Creation Command

**Files:**
- Modify: `src/main.rs`
- Test: `tests/mcp_test.rs`

**Interfaces:**
- Consumes: `Db::get_user_by_username`, `Db::create_api_token`.
- Produces: CLI subcommand `create-token --user <username> --name <name>`.

- [ ] **Step 1: Write test for CLI token creation logic**
Add test in `tests/mcp_test.rs` testing user lookup, role verification (access_level >= 25), and token creation.

- [ ] **Step 2: Run test to verify it passes or fails**
Run: `cargo test --test mcp_test test_cli_token_creation`

- [ ] **Step 3: Implement argument handling in `src/main.rs`**
Parse `std::env::args()` for `create-token`:
- If arguments `--user <username>` and `--name <name>` provided:
  - Open DB, lookup user.
  - Verify user exists and `access_level >= 25`.
  - Call `db.create_api_token(user.id, name)`.
  - Print generated token and exit `0`.

- [ ] **Step 4: Verify CLI functionality**
Run: `cargo test --test mcp_test`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add src/main.rs tests/mcp_test.rs
git commit -m "feat(cli): add create-token CLI command"
```

---

### Task 3: MCP Protocol Data Types & JSON-RPC 2.0 Models

**Files:**
- Create: `src/mcp/mod.rs`
- Create: `src/mcp/protocol.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:
  - `JsonRpcRequest { jsonrpc, id, method, params }`
  - `JsonRpcResponse { jsonrpc, id, result, error }`
  - `JsonRpcError { code, message, data }`
  - `ToolDefinition { name, description, input_schema }`
  - `CallToolResult { content, is_error }`

- [ ] **Step 1: Write unit tests for JSON-RPC serialization & deserialization**
In `src/mcp/protocol.rs` `#[cfg(test)]`, test serialization of initialize response, tools/list response, tools/call request, and error formats.

- [ ] **Step 2: Run unit test to verify it fails**
Run: `cargo test protocol::tests`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement data structures in `src/mcp/protocol.rs`**
Implement Serde structs for JSON-RPC 2.0 messages, MCP standard error codes (`-32700`, `-32600`, `-32601`, `-32602`, `-32603`), and tool schema types.

- [ ] **Step 4: Expose `pub mod mcp;` in `src/lib.rs`**

- [ ] **Step 5: Run unit tests to verify they pass**
Run: `cargo test protocol::tests`
Expected: PASS.

- [ ] **Step 6: Commit**
```bash
git add src/mcp/mod.rs src/mcp/protocol.rs src/lib.rs
git commit -m "feat(mcp): add JSON-RPC 2.0 and MCP protocol data models"
```

---

### Task 4: MCP Session Management (`src/mcp/session.rs`)

**Files:**
- Create: `src/mcp/session.rs`
- Modify: `src/mcp/mod.rs`

**Interfaces:**
- Produces:
  - `McpSession`: holds `id`, `user: User`, `sender: mpsc::Sender<Event>`, `last_active: Instant`.
  - `SessionManager`: thread-safe session map with `create_session`, `get_session`, `remove_session`, `cleanup_stale`.

- [ ] **Step 1: Write unit tests for `SessionManager`**
In `src/mcp/session.rs` `#[cfg(test)]`, test session registration, message dispatch to channel, user retrieval, and eviction of sessions older than threshold.

- [ ] **Step 2: Run unit test to verify it fails**
Run: `cargo test session::tests`
Expected: FAIL.

- [ ] **Step 3: Implement `McpSession` and `SessionManager` in `src/mcp/session.rs`**
Use `Arc<RwLock<HashMap<String, McpSession>>>` and Tokio `mpsc::channel(32)`.

- [ ] **Step 4: Run unit tests to verify they pass**
Run: `cargo test session::tests`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add src/mcp/session.rs src/mcp/mod.rs
git commit -m "feat(mcp): add MCP session manager and SSE event channels"
```

---

### Task 5: Core 8 MCP Tools & RBAC Dispatcher (`src/mcp/tools.rs`)

**Files:**
- Create: `src/mcp/tools.rs`
- Modify: `src/mcp/mod.rs`
- Test: `tests/mcp_test.rs`

**Interfaces:**
- Consumes: `Db`, `User`, `McpSession`.
- Produces:
  - `all_tool_definitions() -> Vec<ToolDefinition>`
  - `dispatch_tool_call(db: &Db, user: &User, name: &str, args: Value) -> CallToolResult`

- [ ] **Step 1: Write failing tests for 8 tools and RBAC checks**
In `tests/mcp_test.rs`:
- Test `whoami` returns user ID and role name.
- Test `list_projects` filters out inaccessible private projects.
- Test `get_project_metadata` requires Viewer (10+).
- Test `create_issue` enforces `reporter_id = user.id` and requires Reporter (25+).
- Test `update_issue` enforces Updater (40+) or Reporter on own issue.
- Test `add_issue_note` enforces `reporter_id = user.id`.
- Test `get_issue` and `search_issues` hide private issues from unauthorized users.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --test mcp_test test_mcp_tools`
Expected: FAIL.

- [ ] **Step 3: Implement `all_tool_definitions` with JSON Schemas**
Define schemas for `whoami`, `list_projects`, `get_project_metadata`, `search_issues`, `get_issue`, `create_issue`, `update_issue`, `add_issue_note`.

- [ ] **Step 4: Implement `dispatch_tool_call` with strict RBAC**
Execute operations against `Db` checking `db.effective_access_level(user, project_id)`. Return `CallToolResult { is_error: true, content: [...] }` upon permission failure or not found.

- [ ] **Step 5: Run tests to verify they pass**
Run: `cargo test --test mcp_test test_mcp_tools`
Expected: PASS.

- [ ] **Step 6: Commit**
```bash
git add src/mcp/tools.rs src/mcp/mod.rs tests/mcp_test.rs
git commit -m "feat(mcp): implement core 8 tools and RBAC dispatcher"
```

---

### Task 6: MCP Axum Handlers, SSE Transport & Full Integration

**Files:**
- Create: `src/mcp/handlers.rs`
- Modify: `src/mcp/mod.rs`
- Modify: `src/handlers/mod.rs`
- Test: `tests/mcp_test.rs`

**Interfaces:**
- Consumes: `SessionManager`, `protocol`, `tools`.
- Produces:
  - `GET /mcp/sse` -> SSE Stream.
  - `POST /mcp/messages` -> JSON-RPC handler.

- [ ] **Step 1: Write end-to-end integration test in `tests/mcp_test.rs`**
Using `tower::ServiceExt::oneshot`:
- Request `GET /mcp/sse?token=INVALID` -> HTTP 401.
- Request `GET /mcp/sse?token=VALID` -> HTTP 200, Content-Type `text/event-stream`, event `endpoint` with session ID.
- Request `POST /mcp/messages?session_id=<UUID>` with `initialize` -> returns capabilities.
- Request `POST /mcp/messages?session_id=<UUID>` with `tools/list` -> returns 8 tools.
- Request `POST /mcp/messages?session_id=<UUID>` with `tools/call` for `whoami` -> returns identity.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --test mcp_test test_mcp_e2e`
Expected: FAIL (endpoints 404).

- [ ] **Step 3: Implement `src/mcp/handlers.rs`**
- `sse_handler`: extract token from query `?token=` or header `Authorization: Bearer`, authenticate via `db.authenticate_api_token`, create `McpSession`, return SSE response with initial `endpoint` event.
- `messages_handler`: validate `session_id`, parse JSON-RPC, dispatch method (`initialize`, `ping`, `tools/list`, `tools/call`), return JSON response.

- [ ] **Step 4: Mount MCP sub-router in `src/handlers/mod.rs`**
Add `.nest("/mcp", mcp::router(db, session_manager))` or add routes directly to the main Axum router.

- [ ] **Step 5: Run all tests to verify green**
Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**
```bash
git add src/mcp/handlers.rs src/handlers/mod.rs tests/mcp_test.rs
git commit -m "feat(mcp): mount Axum SSE and message handlers for MCP integration"
```
