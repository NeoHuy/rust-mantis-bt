# Sub-project 5: REST API, API Tokens & Plugin/Hook System — Design Spec

## 1. Scope & Goals
Complete the 1:1 Rust + SQLite port of MantisBT (`rust-mantis-bt`) with:
1. **API Tokens (`mantis_api_token_table`):**
   - Users can generate and revoke personal API tokens from a web UI (`/api_tokens_page`, `POST /api_token_create`, `POST /api_token_revoke/:id`).
   - Raw token is generated as a 64-hex CSPRNG string, shown once to the user upon creation, while only its SHA-256 hex digest (`sha256_hex`) is stored in `mantis_api_token_table.hash`.
   - Authenticating with `Authorization: <token>` or `Authorization: Bearer <token>` looks up the token hash, verifies the owning user is `enabled == 1`, and updates `date_used`.
2. **MantisBT-Compatible JSON REST API (`/api/rest/...`):**
   - `GET /api/rest/users/me` — returns the authenticated user profile (`id`, `name`, `real_name`, `email`, `access_level`).
   - `GET /api/rest/projects` and `GET /api/rest/projects/:id` — returns visible projects (with categories and versions).
   - `GET /api/rest/issues` (supports `?project_id=`, `?page=`, `?page_size=`) and `GET /api/rest/issues/:id` — returns issues with full text, bugnotes, attachments metadata, tags, custom fields, and history.
   - `POST /api/rest/issues` — creates a new issue via JSON payload (`summary`, `description`, `project: { id }`, `category: { id }`, optional `priority`, `severity`, `target_version`, etc.).
   - `PATCH /api/rest/issues/:id` — updates issue fields, status, resolution, or handler via JSON payload.
   - `DELETE /api/rest/issues/:id` — deletes an issue (requires `Manager >= 70` on the project).
   - `POST /api/rest/issues/:id/notes` — adds a bugnote (with optional `view_state` and `time_tracking` minutes) via JSON payload.
3. **Plugin & Event Hook System (`mantis_plugin_table` + `WebhookNotification` / `AutoTagging` built-in plugins):**
   - Database table `mantis_plugin_table` (`basename TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 1, protected INTEGER NOT NULL DEFAULT 0, priority INTEGER NOT NULL DEFAULT 3`).
   - Built-in event hooks dispatched on issue creation, status change, and bugnote addition:
     - `AutoTitleNormalizer` (`basename = "AutoTitleNormalizer"`): when enabled, trims and normalizes whitespace in issue summaries or prefixes/tags according to config.
     - `AuditWebhookLog` (`basename = "AuditWebhookLog"`): when enabled, records structured JSON event payloads into `mantis_config_table` / plugin event log (`plugin_event_log` or `mantis_email_table` / dedicated queryable hook log) whenever `EVENT_REPORT_BUG`, `EVENT_UPDATE_BUG_STATUS`, or `EVENT_BUGNOTE_ADD` fires.
   - Admin UI (`/manage_plugin_page`, `POST /manage_plugin_toggle/:basename`) for `Administrator (90)` to view installed plugins and enable/disable them.

## 2. Database Schema Additions (25 Tables Total)
```sql
CREATE TABLE IF NOT EXISTS mantis_api_token_table (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    hash TEXT NOT NULL UNIQUE,
    date_created INTEGER NOT NULL DEFAULT 0,
    date_used INTEGER NOT NULL DEFAULT 0,
    UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS mantis_plugin_table (
    basename TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    protected INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 3
);
```
Default seed in `Db::migrate()`:
- `MantisCoreFormatting` (`enabled = 1, protected = 1, priority = 3`)
- `EventAuditLog` (`enabled = 1, protected = 0, priority = 3`)
