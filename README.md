# rust-mantis-bt

A fast, lightweight, and modern **Mantis Bug Tracker (MantisBT)** clone implemented in **Rust** using **Axum**, **SQLite (rusqlite)**, and **Tokio**.

---

## Features

- **Core Issue Tracking**: Complete MantisBT lifecycle (reproducibility, severity, priority, status workflow, bugnotes, history, and attachments).
- **Project & Category Management**: Hierarchical subprojects, categories, project versions, and inheritance.
- **Role-Based Access Control (RBAC)**: Strict 6-tier permissions matching MantisBT:
  - `Viewer (10)`: Read-only access to public issues.
  - `Reporter (25)`: Report bugs, update own bugs, add notes, create API tokens.
  - `Updater (40)`: Edit existing issues.
  - `Developer (55)`: Handle issues, assign assignees, view private issues.
  - `Manager (70)`: Delete issues, close issues, manage project settings.
  - `Administrator (90)`: System-wide governance, plugins, and global token oversight.
- **REST API**: JSON REST endpoints compatible with MantisBT API clients.
- **Native Model Context Protocol (MCP) Server**: Built-in Server-Sent Events (SSE) and HTTP protocol support allowing AI models (Antigravity, Claude Desktop, Cursor, Windsurf, etc.) to securely interact with MantisBT.

---

## Quick Start

### Prerequisites
- [Rust toolchain](https://www.rust-lang.org/) (Rust 1.75+ recommended)

### Running the Server
```bash
cargo run
```
By default, the server listens on `http://127.0.0.1:3000` with the SQLite database stored at `./mantis.db`.

You can configure address, base URL, and database path via environment variables:
```bash
export MANTIS_ADDR="0.0.0.0:3000"
export MANTIS_BASE_URL="http://127.0.0.1:3000"
export MANTIS_DB_PATH="/path/to/mantis.db"
cargo run
```

Default administrator credentials:
- **Username**: `administrator`
- **Password**: `root`

---

## Model Context Protocol (MCP) Setup Guide

`rust-mantis-bt` includes an embedded MCP server providing an SSE transport endpoint (`/mcp/sse`) and JSON-RPC 2.0 message handler (`/mcp/messages`).

### 1. Generate an API Token

Every AI connection is **strictly bound to a specific user**. The AI assistant will act under that user's identity and will inherit 100% of their permissions and project restrictions.

You can create an API Token using either of the following methods:

#### Option A: Via Command Line (CLI)
Generate a token directly from terminal without logging into the web browser:
```bash
cargo run -- create-token --user <username> --name "AI Assistant"
```
*(Note: The user must have Reporter level 25 or higher).*

#### Option B: Via Web UI
1. Log in to the MantisBT web interface (`http://127.0.0.1:3000`).
2. Navigate to **API Tokens** (`/api_tokens_page`).
3. Enter a token name (e.g. `Claude-Desktop` or `Cursor-AI`) and click **Create API Token**.
4. Copy the generated 64-hex token string.

---

### 2. Configure Your AI Client

#### A. Antigravity / Cursor / Windsurf / Roo-Code
Add the server to your client's MCP configuration (e.g., in `mcp_servers` or settings) with the `Authorization` header:

```json
{
  "mcpServers": {
    "mantis": {
      "url": "http://127.0.0.1:3000/mcp/sse",
      "headers": {
        "Authorization": "Bearer YOUR_API_TOKEN"
      }
    }
  }
}
```

#### B. Claude Desktop
Add the following to your `claude_desktop_config.json`:
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

Using `mcp-proxy` with the authorization header:
```json
{
  "mcpServers": {
    "mantis": {
      "command": "npx",
      "args": [
        "-y",
        "mcp-proxy",
        "--headers",
        "Authorization: Bearer YOUR_API_TOKEN",
        "http://127.0.0.1:3000/mcp/sse"
      ]
    }
  }
}
```

> **Security Note:** Passing raw API tokens in the URL query parameter (`?token=...`) is disabled by default to prevent tokens from leaking into web proxy logs and server access logs. If needed for local testing, it can be enabled by setting `MANTIS_ALLOW_QUERY_TOKEN=1`.

---

### 3. Available MCP Tools

Once connected, your AI assistant will automatically discover the following 8 tools:

| Tool Name | Description | Key Parameters | Required Role |
| :--- | :--- | :--- | :--- |
| **`whoami`** | Returns the authenticated user's ID, username, and role. | None | Reporter (25+) |
| **`list_projects`** | Lists all projects accessible to the user. | None | Viewer (10+) |
| **`get_project_metadata`**| Returns project categories and version definitions. | `project_id` | Viewer (10+) |
| **`search_issues`** | Search/filter issues by project, status, keyword, handler. | `project_id`, `search`, `status`, `limit`, `page` | Viewer (10+) |
| **`get_issue`** | Retrieve full details, notes, and reproduction steps for a bug. | `issue_id` | Viewer (10+); Dev (55+) for private bugs |
| **`create_issue`** | Report a new issue. Auto-assigns `reporter_id = user.id`. | `project_id`, `category_id`, `summary`, `description` | Reporter (25+) |
| **`update_issue`** | Update status, handler, priority, severity, or summary. | `issue_id`, `status`, `handler_id`, `priority` | Updater (40+) / Dev (55+) / Mgr (70+) |
| **`add_issue_note`** | Add a comment/bugnote. Auto-records user as author. | `issue_id`, `text`, `view_state`, `time_tracking` | Reporter (25+) |

---

### 4. RBAC & Security Principles in MCP

- **Identity Attribution**: All issues and bugnotes created by the AI have `reporter_id` automatically set to the user ID owning the token.
- **Least Privilege**: If an AI assistant tries to perform an action unauthorized for the user's role (such as a Reporter trying to close a ticket or view another team's private project), the MCP server rejects the call gracefully with `isError: true` and an explanatory message.
- **Admin Oversight**: Administrators (`access_level = 90`) can view all tokens across the system at `/api_tokens_page` and revoke any token immediately.

---

## Testing

Run the automated test suite:
```bash
cargo test
```
All unit tests and integration tests (including REST API, tokens, and MCP SSE) will execute against an in-memory SQLite database.

---

## License

GNU General Public License v2.0 (matching MantisBT license).
