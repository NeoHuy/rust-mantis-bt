# Sub-project: MantisBT - Atlassian Jira Integration Design Spec

**Date:** 2026-10-13  
**Status:** Approved  
**Author:** AI Pair Programmer & User  

---

## 1. Overview & Goals

This specification defines the integration between **Rust MantisBT** and **Atlassian Jira Cloud**.

### User Story
As a Developer or Triage engineer, when reviewing an issue submitted by users in MantisBT, if I determine that this issue is a valid bug or task to be implemented by engineering, I can push it directly to Jira with a single click to create an issue ticket automatically in Jira.

### Key Objectives
1. **Direct Triage Action**: Provide an explicit "Push to Jira" / "Create Jira Issue" action on the MantisBT bug view page (`/view_bug_page/:id`) for users with Developer or higher access level (`access_level >= 55`).
2. **Issue Creation via Jira REST API**: Create issues on Jira Cloud via HTTP Basic Auth (Email + API Token) using Jira REST API v2 (`/rest/api/2/issue`).
3. **Traceability & Linking**:
   - Store Jira Issue Key (`PROJ-123`) and URL in a dedicated table `mantis_jira_links_table`.
   - Render a visible Jira badge/link on the bug detail page and issue list (`/view_all_bug_page`).
   - Automatically insert a system Bugnote in Mantis documenting the creation of the Jira issue.
   - Automatically update the Mantis bug status to `acknowledged` (status = `30`) or `assigned` (status = `50`).
4. **Flexible Configuration & Project Mapping**:
   - Provide an Admin configuration page (`/manage_jira_page`) for Global Jira settings (Base URL, Account Email, API Token, Default Issue Type, Default Project Key).
   - Allow per-Mantis-project mapping to Jira Project Keys in `mantis_config_table`.
   - Provide a "Test Connection" button to verify Jira credentials against the Jira API `/rest/api/3/myself`.

---

## 2. Architecture & Components

```
+-------------------------------------------------------------+
|                        MantisBT Web UI                      |
|  - /manage_jira_page (Admin Configuration & Mapping)        |
|  - /view_bug_page/:id (Triage action & Jira issue badge)    |
|  - /view_all_bug_page (Jira issue badge indicator)          |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                    Axum Route Handlers                      |
|  - GET  /manage_jira_page                                   |
|  - POST /manage_jira_config                                 |
|  - POST /manage_jira_test                                   |
|  - POST /view_bug_page/:id/jira_push                        |
+------------------------------+------------------------------+
                               |
               +---------------+---------------+
               |                               |
               v                               v
+-------------------------------+ +---------------------------+
|          Mantis Db            | |      Jira API Client      |
|  - mantis_jira_links_table    | |  src/jira.rs              |
|  - mantis_config_table        | |  - test_connection()      |
|  - mantis_bug_table           | |  - create_issue()         |
|  - mantis_bugnote_table       | +-------------+-------------+
|  - mantis_bug_history_table   |               |
+-------------------------------+               | HTTPS REST
                                                v
                                  +---------------------------+
                                  |    Atlassian Jira Cloud   |
                                  |    /rest/api/2/issue      |
                                  |    /rest/api/3/myself     |
                                  +---------------------------+
```

---

## 3. Database Schema

### 3.1. `mantis_jira_links_table`
Stores link records between Mantis bugs and Jira tickets (1-to-1 relationship per bug):

```sql
CREATE TABLE IF NOT EXISTS mantis_jira_links_table (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bug_id INTEGER NOT NULL UNIQUE,
    jira_key TEXT NOT NULL,
    jira_url TEXT NOT NULL,
    issue_type TEXT NOT NULL DEFAULT 'Bug',
    created_by INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(bug_id) REFERENCES mantis_bug_table(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_jira_links_bug_id ON mantis_jira_links_table(bug_id);
CREATE INDEX IF NOT EXISTS idx_jira_links_key ON mantis_jira_links_table(jira_key);
```

### 3.2. Configuration Keys in `mantis_config_table`
Stored using existing hierarchical `get_config` and `set_config` functions:
- Global configs (`project_id = 0, user_id = 0`):
  - `jira_enabled`: `"1"` or `"0"`
  - `jira_base_url`: e.g. `https://mycompany.atlassian.net`
  - `jira_email`: e.g. `dev@mycompany.com`
  - `jira_api_token`: Atlassian API Token
  - `jira_default_issue_type`: e.g. `"Bug"` or `"Task"`
  - `jira_default_project_key`: e.g. `"PROJ"`
- Per-project configs (`project_id = X, user_id = 0`):
  - `jira_project_key`: specific Jira Project Key for Mantis Project X

---

## 4. Jira REST API Client (`src/jira.rs`)

### 4.1. Dependencies
Add to `Cargo.toml`:
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

### 4.2. Credentials & Authentication
- HTTP Basic Auth Header: `Authorization: Basic base64(email:api_token)`
- Headers:
  - `Accept: application/json`
  - `Content-Type: application/json`

### 4.3. Methods
1. `test_connection(config: &JiraConfig) -> Result<JiraUserInfo, JiraError>`:
   - Request: `GET {base_url}/rest/api/3/myself`
   - Returns user display name or email on success (HTTP 200).
2. `create_issue(config: &JiraConfig, req: JiraCreateIssueRequest) -> Result<JiraCreateIssueResponse, JiraError>`:
   - Request: `POST {base_url}/rest/api/2/issue`
   - Payload:
     ```json
     {
       "fields": {
         "project": { "key": "PROJ" },
         "summary": "[Mantis #0001234] Title of the bug",
         "description": "Full description...\n\n---\nReported in MantisBT: {base_url}/view_bug_page/1234\nReporter: username\nSeverity: major",
         "issuetype": { "name": "Bug" }
       }
     }
     ```
   - Response: HTTP 201 Created with JSON `{ "id": "10001", "key": "PROJ-123", "self": "..." }`.
   - Resolves Jira browse URL: `{base_url}/browse/{key}`.

---

## 5. User Interface & Endpoints

### 5.1. Admin Configuration (`/manage_jira_page`)
- Route: `GET /manage_jira_page`, `POST /manage_jira_config`, `POST /manage_jira_test`
- Requires `access_level >= 90` (ADMINISTRATOR).
- Displays:
  - Global configuration form with masked token.
  - "Test Connection" button that triggers verification and displays success/error banner.
  - Per-project mapping table listing all Mantis projects with inputs for Jira Project Key.

### 5.2. Bug Detail View (`/view_bug_page/:id`)
- Route: `POST /view_bug_page/:id/jira_push`
- Permissions: requires `access_level >= 55` (DEVELOPER).
- If Bug already linked:
  - Displays Jira Badge: `<a href="{jira_url}" target="_blank" class="badge badge-primary">Jira: {jira_key} ↗</a>`.
  - Push button is hidden/disabled.
- If Bug is not linked and `jira_enabled == "1"`:
  - Shows button **"Push to Jira"** with a confirmation modal/form.
  - Modal form allows reviewing Jira Project Key (prefilled with project mapping), Issue Type (`Bug`/`Task`), Summary, and target Mantis status (default `acknowledged`).
- Action handler:
  1. Validates user permission and bug status.
  2. Ensures idempotency (checks `mantis_jira_links_table`).
  3. Sends request to Jira.
  4. On 201 Created:
     - Inserts record into `mantis_jira_links_table`.
     - Adds a Bugnote: `"Created Jira issue [{jira_key}]({jira_url}) by {username}."`
     - Updates Mantis bug status to `acknowledged` (30).
     - Records history event in `mantis_bug_history_table`.
     - Redirects with `?msg=Created+Jira+issue+{jira_key}`.
  5. On error:
     - Keeps Mantis bug state untouched.
     - Redirects with `?err=Jira+error:+{escaped_error}`.

### 5.3. Bug Listing View (`/view_all_bug_page`)
- Shows a concise Jira badge link next to the bug ID or summary if linked.

---

## 6. Error Handling & Edge Cases

1. **Jira Offline / Unreachable / Timeout**:
   - HTTP client configured with a 10-second timeout.
   - Clean error notification presented to user without corrupted database state.
2. **Invalid Jira Credentials (HTTP 401/403)**:
   - Clear message indicating authentication failure or insufficient permissions on Jira project.
3. **Concurrent Submissions (Race Condition)**:
   - Enforced by `UNIQUE(bug_id)` constraint in SQLite.
4. **Deleted Bugs**:
   - Cascading foreign key deletes the corresponding link record.

---

## 7. Testing Plan

1. **Unit Tests**:
   - Serialization and deserialization of Jira REST requests and responses.
   - URL parsing and basic auth encoding.
2. **Database Integration Tests**:
   - CRUD operations on `mantis_jira_links_table`.
   - Hierarchical config reading and updating for Jira configs.
3. **HTTP Integration Tests**:
   - Mock HTTP server (e.g., using Axum mock or wiremock) to test successful issue creation, connection test, 401 error, and 400 validation error scenarios.
   - End-to-end bug push flow: verify Jira link table, bugnote insertion, and bug status update.
