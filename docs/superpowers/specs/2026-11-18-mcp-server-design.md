# Sub-project: MantisBT Model Context Protocol (MCP) Server Integration — Design Spec

**Date:** 2026-11-18  
**Status:** Approved  
**Author:** AI Pair Programmer & User  

---

## 1. Overview & Goals

This specification defines the architecture, protocol, identity binding, and tool suite for integrating a **Model Context Protocol (MCP) Server** directly into **Rust MantisBT** (`rust-mantis-bt`).

The MCP Server enables AI assistants (Claude Desktop, Antigravity, Cursor, Windsurf, Roo-Code, etc.) to discover and execute issue-tracking operations in MantisBT via a standard protocol.

### Key Objectives & Principles
1. **Direct Native Integration**: Embedded into the Axum web application (`src/mcp/`) over Server-Sent Events (SSE) and HTTP POST (`/mcp/sse` and `/mcp/messages`), requiring no separate daemon or external unstable dependencies.
2. **Strict User Representation & Identity Binding**:
   - The AI assistant **must always act on behalf of a specific authenticated MantisBT user**. There are no anonymous or super-privileged bot actions.
   - Every issue reported by the AI sets `reporter_id = user.id`.
   - Every bugnote added by the AI sets `reporter_id = user.id`.
   - Every action follows the user's audit history.
3. **Role-Based Access Control (RBAC) Enforcement**:
   - The AI assistant is constrained by 100% of the permissions assigned to that user's role on each project (Viewer: 10, Reporter: 25, Updater: 40, Developer: 55, Manager: 70, Administrator: 90).
   - If an AI tool call exceeds the user's role, the server rejects it gracefully with a descriptive error (`isError: true`).
4. **Token Management & Access Governance**:
   - Only users with **Reporter (25+)** or higher access levels are allowed to generate API tokens on `/api_tokens_page`.
   - **Administrators (90)** can view all system-wide tokens across all users and revoke any token.
   - Provide a convenient CLI command (`cargo run -- create-token --user <username> --name <name>`) for terminal/script token provisioning.
5. **Core Issue Tracking Toolset (Phase 1)**:
   - Expose 8 essential tools: `whoami`, `list_projects`, `get_project_metadata`, `search_issues`, `get_issue`, `create_issue`, `update_issue`, `add_issue_note`.

---

## 2. Architecture & Components

```
+---------------------------------------------------------------------------------+
|                                AI Client / Host                                 |
|            (Antigravity / Claude Desktop / Cursor / Windsurf / etc.)            |
+---------------------------------------------------------------------------------+
        │ (1) GET /mcp/sse?token=<API_TOKEN> (or Authorization: Bearer <TOKEN>)
        ▼
+---------------------------------------------------------------------------------+
|                              Rust MantisBT (Axum)                               |
|                                                                                 |
|  ┌───────────────────────────────────────────────────────────────────────────┐  |
|  │                        src/mcp/handlers.rs                                │  |
|  │  - GET /mcp/sse        -> Authenticate Token, Register McpSession,       │  |
|  │                           Send initial 'endpoint' SSE event               │  |
|  │  - POST /mcp/messages  -> Validate session_id, Dispatch JSON-RPC 2.0      │  |
|  └─────────────────────────────────────┬─────────────────────────────────────┘  |
|                                        │                                        |
|  ┌─────────────────────────────────────▼─────────────────────────────────────┐  |
|  │                        src/mcp/session.rs                                 │  |
|  │  - McpSession: session_id (UUID), User, mpsc::Sender<Event>, last_active  │  |
|  │  - SessionManager: Arc<RwLock<HashMap<String, McpSession>>>               │  |
|  └─────────────────────────────────────┬─────────────────────────────────────┘  |
|                                        │                                        |
|  ┌─────────────────────────────────────▼─────────────────────────────────────┐  |
|  │                        src/mcp/tools.rs                                   │  |
|  │  - 8 Tool Definitions with JSON Schemas                                   │  |
|  │  - RBAC checks against session user via db.effective_access_level()       │  |
|  │  - Execution of Issue/Project operations via Db                           │  |
|  └─────────────────────────────────────┬─────────────────────────────────────┘  |
|                                        │                                        |
|  ┌─────────────────────────────────────▼─────────────────────────────────────┐  |
|  │                   src/db.rs (SQLite / mantis.db)                          │  |
|  │  - mantis_user_table, mantis_api_token_table, mantis_bug_table, etc.      │  |
|  └───────────────────────────────────────────────────────────────────────────┘  |
+---------------------------------------------------------------------------------+
```

### Module Layout
```text
src/mcp/
├── mod.rs          # Module declarations, router builder, McpState
├── protocol.rs     # JSON-RPC 2.0 data models (Request, Response, Error, Tool schema)
├── session.rs      # Session tracking (UUID, User, mpsc channel, timeout sweep)
├── tools.rs        # Tool definitions, JSON Schemas, input parsing, RBAC validation
└── handlers.rs     # Axum endpoint handlers (GET /mcp/sse, POST /mcp/messages)
```

---

## 3. Token Management & Role Governance

### 3.1 Token Creation Gate
- In `src/handlers/api.rs`:
  - `GET /api_tokens_page`: Check `ctx.user.access_level >= 25`. If `< 25`, return `403 Forbidden` (`forbidden_html(&ctx, "Reporter (25) or higher access level required to generate API tokens")`).
  - `POST /api_token_create`: Check `ctx.user.access_level >= 25`. If `< 25`, return `403 Forbidden`.

### 3.2 Administrator Token Oversight
- In `src/db.rs`:
  - Add `pub fn list_all_api_tokens(&self) -> rusqlite::Result<Vec<(ApiToken, String /* username */, String /* real_name */)>>`
- In `src/handlers/api.rs` (`api_tokens_page`):
  - When `ctx.user.access_level >= 90` (Administrator), render an additional table: **System-Wide API Tokens (Administrator View)**.
  - Lists: Token ID, Owner Username/Real Name, Token Name, Created Date, Last Used Date, and Revoke action.
- In `POST /api_token_revoke/:id`:
  - Allow revocation if `ctx.user.id == token.user_id` OR `ctx.user.access_level >= 90`.

### 3.3 CLI Token Generation
- Add CLI flag / command in `src/main.rs`:
  ```bash
  cargo run -- create-token --user <username> --name <token_name>
  ```
  - Looks up the user in SQLite.
  - Verifies user exists and `access_level >= 25`.
  - Generates token and prints the raw 64-hex string to stdout.

---

## 4. MCP Protocol & Session Lifecycle

### 4.1 Handshake (`GET /mcp/sse`)
- **Authentication**:
  1. Header: `Authorization: Bearer <raw_token>` or `Authorization: <raw_token>`
  2. Query Param: `/mcp/sse?token=<raw_token>`
- **Validation**:
  - `db.authenticate_api_token(raw_token)`
  - If invalid or missing, respond with `401 Unauthorized` (`{"error": "Unauthorized"}`).
- **Connection Setup**:
  - Generate a secure UUID v4 `session_id`.
  - Create a Tokio `mpsc::channel::<axum::response::sse::Event>(32)`.
  - Register session in `SessionManager`:
    ```rust
    pub struct McpSession {
        pub id: String,
        pub user: crate::models::User,
        pub sender: tokio::sync::mpsc::Sender<axum::response::sse::Event>,
        pub last_active: std::time::Instant,
    }
    ```
  - Send the initial SSE event:
    - Event name: `endpoint`
    - Data: `/mcp/messages?session_id=<UUID>`
  - Return `axum::response::sse::Sse` stream to client.

### 4.2 Message Dispatch (`POST /mcp/messages?session_id=<UUID>`)
- Look up `session_id` in `SessionManager`. If not found, return `404 Not Found` (or `401 Unauthorized`).
- Update `last_active = Instant::now()`.
- Parse body as JSON-RPC 2.0 Request:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": { ... }
  }
  ```
- **Supported Methods**:
  1. `initialize`: Returns server capabilities, protocol version (`2024-11-05`), and server metadata (`name: "rust-mantis-bt", version: "0.1.0"`).
  2. `notifications/initialized`: Acknowledged, no-op response.
  3. `ping`: Responds with empty object `{}`.
  4. `tools/list`: Returns the list of 8 tools with their JSON Schema parameter specifications.
  5. `tools/call`: Dispatches to `src/mcp/tools.rs` passing `(db, session.user, tool_name, arguments)`.
- The JSON-RPC response is returned directly in the HTTP POST response body and/or routed through the SSE event channel.

---

## 5. Specification of Core 8 MCP Tools

### 5.1 `whoami`
- **Description**: Returns the authenticated user profile under whose identity this session executes.
- **Parameters**: None `{}`.
- **RBAC**: Permitted for any valid authenticated user token.
- **Response**:
  ```json
  {
    "user_id": 1,
    "username": "administrator",
    "real_name": "Root Admin",
    "email": "admin@example.com",
    "access_level": 90,
    "access_level_name": "administrator"
  }
  ```

### 5.2 `list_projects`
- **Description**: Lists all projects visible to the current user.
- **Parameters**: None `{}`.
- **RBAC**: Uses `db.list_visible_projects(&user)`. Projects that are private where the user is not a member are filtered out.
- **Response**: Array of project summaries `[{ id, name, status, view_state, description }]`.

### 5.3 `get_project_metadata`
- **Description**: Retrieves categories, versions, and metadata for a specific project.
- **Parameters**:
  - `project_id` (integer, required)
- **RBAC**: Requires `effective_access_level(&user, project_id) >= 10` (Viewer).
- **Response**: `{ project_id, categories: [...], versions: [...] }`.

### 5.4 `search_issues`
- **Description**: Search and filter issues across visible projects.
- **Parameters**:
  - `project_id` (integer, optional)
  - `status` (integer, optional)
  - `handler_id` (integer, optional)
  - `reporter_id` (integer, optional)
  - `search` (string, optional - keyword search in summary/description)
  - `limit` (integer, optional - default 25, max 100)
  - `page` (integer, optional - default 1)
- **RBAC**:
  - Only returns issues from projects visible to the user.
  - Private issues (`view_state == 50`) are omitted unless the user has `effective_access_level >= 55` (Developer) on that project or is the reporter of the issue.

### 5.5 `get_issue`
- **Description**: Get full issue details including description, steps to reproduce, bugnotes, and tags.
- **Parameters**:
  - `issue_id` (integer, required)
- **RBAC**:
  - Requires `effective_access_level(&user, bug.project_id) >= 10`.
  - If `bug.view_state == 50` (Private): requires `effective_access_level >= 55` or `bug.reporter_id == user.id`.

### 5.6 `create_issue`
- **Description**: Create/report a new issue in a project.
- **Parameters**:
  - `project_id` (integer, required)
  - `category_id` (integer, required)
  - `summary` (string, required)
  - `description` (string, required)
  - `priority` (integer, optional - default 30: normal)
  - `severity` (integer, optional - default 50: minor)
  - `reproducibility` (integer, optional - default 10: always)
  - `steps_to_reproduce` (string, optional)
  - `additional_information` (string, optional)
  - `view_state` (integer, optional - default 10: public)
- **RBAC**:
  - Requires `effective_access_level(&user, project_id) >= 25` (Reporter).
  - Enforces `reporter_id = user.id`.

### 5.7 `update_issue`
- **Description**: Update an issue's status, resolution, handler, summary, priority, or severity.
- **Parameters**:
  - `issue_id` (integer, required)
  - `status` (integer, optional)
  - `resolution` (integer, optional)
  - `handler_id` (integer, optional)
  - `priority` (integer, optional)
  - `severity` (integer, optional)
  - `summary` (string, optional)
- **RBAC**:
  - Requires `effective_access_level >= 40` (Updater) OR (`effective_access_level >= 25` AND `bug.reporter_id == user.id`).
  - Assigning `handler_id` requires `effective_access_level >= 55` (Developer).
  - Closes/Resolves require appropriate workflow threshold.

### 5.8 `add_issue_note`
- **Description**: Add a comment/bugnote to an existing issue.
- **Parameters**:
  - `issue_id` (integer, required)
  - `text` (string, required)
  - `view_state` (integer, optional - 10: public, 50: private)
  - `time_tracking` (integer, optional - minutes spent)
- **RBAC**:
  - Requires `effective_access_level >= 25` (Reporter).
  - If issue is private: requires `effective_access_level >= 55` or user is reporter.
  - Setting note `view_state = 50` (Private) requires `effective_access_level >= 55`.
  - Enforces `reporter_id = user.id`.

---

## 6. Error Handling Strategy

1. **Protocol Errors (JSON-RPC 2.0)**:
   - `-32700` (Parse error): Invalid JSON.
   - `-32600` (Invalid Request): Missing `jsonrpc: "2.0"` or invalid payload structure.
   - `-32601` (Method not found): Any method other than `initialize`, `notifications/*`, `ping`, `tools/list`, `tools/call`.
   - `-32602` (Invalid params): Argument schema violation (missing required parameters or wrong type).
   - `-32603` (Internal error): Database connection failure or unhandled internal error.

2. **Tool Execution Errors**:
   - Returned via standard MCP tool result with `isError: true`:
     ```json
     {
       "content": [
         {
           "type": "text",
           "text": "Permission Denied: User 'reporter1' (access level 25) cannot assign handler on project 1. Developer (55) required."
         }
       ],
       "isError": true
     }
     ```

---

## 7. Testing Strategy

Create comprehensive integration tests in `tests/mcp_test.rs`:
1. **Token Permissions & Role Governance**:
   - Verify Viewer (10) receives HTTP 403 on `/api_tokens_page` and `POST /api_token_create`.
   - Verify Reporter (25) successfully creates an API token.
   - Verify Administrator (90) sees all system tokens and can revoke tokens owned by other users.
2. **MCP SSE Handshake & Authentication**:
   - Unauthenticated `GET /mcp/sse` -> HTTP 401.
   - Valid token via query param and via Bearer header -> HTTP 200 with `text/event-stream` and `endpoint` event.
3. **MCP Core Operations**:
   - `initialize` returns protocol version `2024-11-05` and server info.
   - `tools/list` returns all 8 tool schemas.
   - `whoami` returns authenticated user identity.
   - `list_projects` filters out private projects where the user is not a member.
4. **Issue Management & RBAC**:
   - Reporter creates issue -> issue exists with `reporter_id = user.id`.
   - Reporter updates own issue -> success.
   - Reporter updates another user's issue -> rejected with `isError: true` (Permission Denied).
   - Developer updates issue & assigns handler -> success.
   - Private issue visibility test: Reporter cannot see private issue; Developer can view private issue.
   - Add note test: Note author matches authenticated token user.
