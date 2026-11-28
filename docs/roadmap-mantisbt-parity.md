# MantisBT 2.28.4 Feature Parity Roadmap (`rust-mantis-bt`)

This document summarizes the feature comparison between `rust-mantis-bt` and upstream **MantisBT 2.28.4** (schema v213), and defines a 5-phase implementation roadmap (**Phase 1–5**) to bring `rust-mantis-bt` up to feature parity with MantisBT.

> **Scope:** This document **excludes** Jira integration (`src/jira.rs`, `src/controllers/jira.rs`, `src/views/jira.rs`, `src/models/jira.rs`), which is a custom extension specific to `rust-mantis-bt`.

---

## 1. High-Level Feature Comparison

| Feature Area | MantisBT 2.28.4 (Upstream) | `rust-mantis-bt` | Status |
| :--- | :--- | :--- | :--- |
| Issue CRUD, Notes, History, Attachments, Monitors, Relationships | Full lifecycle + Revisions, Clone, Move, Bulk actions, Sticky, Reminders, Sponsorship | Full lifecycle: CRUD, Notes edit/visibility, Revisions, Optimistic Locking, Clone, Move, Bulk (`bug_actiongroup`), Sticky, Monitors, Reminders, Multi-file upload/preview (no Sponsorships) | **Completed (Phase 2)** |
| Workflow & Permissions UI | Workflow Transitions matrix, Thresholds, Graphviz, DB Config editor | Hardcoded in application logic (no configuration UI) | **Not implemented** |
| Dashboard (`My View`), Filtering & Export | 9 configurable boxes + Timeline; multi-select/ANY-ALL/Custom Field filters + Permalinks; CSV, Excel, Print, Word, RSS | 9 configurable boxes + Activity Timeline; multi-select `ALL`/`ANY` + Custom Field filters + Permalinks; CSV, Excel XML, Print, Word, token-authenticated RSS | **Completed (Phase 3)** |
| Planning & Reporting | Roadmap, Changelog, Summary (15+ tables) + MantisGraph (Chart.js), Billing (date range, rate, export) | Roadmap, Changelog, extended Summary (14 tables) + SVG Status Donut & Issues Trend charts, date-filtered Billing with rates & exports | **Completed (Phase 3)** |
| Projects, Subprojects, Categories, Versions, Docs | Full CRUD + cross-project copy + Project Documentation (`proj_doc_*`) | Basic CRUD + subproject cycle check; missing copy actions & Project Docs | **Partial (~65%)** |
| Custom Fields & Tags | 11 types with typed rendering/validation/regex/required/dynamic values + Tag detail/merge | All 11 types declared, UI renders text input only; basic Tag CRUD | **Partial (~40%)** |
| Users, Auth, Preferences & Profiles | LDAP/HTTP/MD5, Signup verification, Lost password, CAPTCHA, Anonymous, Impersonation, Pruning, Prefs, Profiles | Local auth (PBKDF2-SHA256), direct signup, lockout; remaining flows missing | **Partial (~35%)** |
| Plugin System | 83 event hooks + 4 bundled plugins (`MantisCoreFormatting`, `MantisGraph`, `XmlImportExport`, `Gravatar`) | Toggle table + active `MantisCoreFormatting` (Markdown, `#123`/`~123` links, `@mention`), built-in SVG charts, `EventAuditLog` | **Partial (Core Formatting & Charts)** |
| API (REST & SOAP) | Full REST API (~40+ routes, OpenAPI, ETag) + SOAP API (72 operations) | Basic REST API (9 routes), no SOAP | **Partial (~20%)** |
| Email, i18n, Admin & Other Integrations | SMTP/PHPMailer + DKIM/S-MIME, 66 languages, Timezones, Wiki, OpenSearch, 16 Admin Checks | SendGrid API only (plain text), English & UTC only, no Admin Checks/Wiki | **Partial (~20%)** |
| **MCP Server (AI Agent)** | Not available | Built-in SSE MCP Server with 8 tools | **Rust-only capability** |

---

## 2. Capabilities Unique to `rust-mantis-bt` (Excluding Jira Sync)

1. **Built-in MCP Server ([`src/mcp/`](../src/mcp/mod.rs)):**
   - Implements Model Context Protocol (`2024-11-05`, JSON-RPC 2.0 over SSE at `/mcp/sse` and `/mcp/messages`) authenticated via API tokens, exposing 8 tools for AI Agents (`whoami`, `list_projects`, `get_project_metadata`, `search_issues`, `get_issue`, `create_issue`, `update_issue`, `add_issue_note`).
2. **Stronger Default Password Security & CSV Protection:**
   - Hashes passwords by default with **PBKDF2-SHA256** (600,000 iterations in release builds in [`src/auth.rs`](../src/auth.rs)) with transparent legacy hash upgrades (`MD5`/`SHA256`) and constant-time/timing-equalized verification (upstream still defaults to `MD5`).
   - Neutralizes leading `=`, `+`, `-`, and `@` characters in CSV exports to prevent spreadsheet formula injection.
3. **Operational Simplicity & Built-in Tooling:**
   - Single-binary deployment backed by SQLite (WAL mode), embedded CSS/assets, a built-in periodic email queue worker, and the `create-token` CLI subcommand ([`src/lib.rs`](../src/lib.rs)).

---

## 3. Implementation Roadmap Overview (5 Phases)

| Phase | Focus | Primary Objectives | Status |
| :--- | :--- | :--- | :--- |
| **Phase 1** | **Quick Wins & Closing Half-Built Features** | Wire existing DB columns/routes, fix filter pagination, complete Custom Fields rendering & validation | **Completed** (Milestones 1–4, `valid_regexp`, Roadmap subprojects/scoping, `tests/backward_compat_test.rs`) |
| **Phase 2** | **Daily Issue & Note Workflows** | Bulk actions, Clone/Move, Bugnote editing + Revisions, Markdown & `#123`/`@mention`, multi-file upload & preview | **Completed** (Milestones 1–4 + polish: bugnote editing/revisions/optimistic locking, clone/move/bulk actions, `MantisCoreFormatting` Markdown/mentions, monitors/reminders/multi-file uploads) |
| **Phase 3** | **Advanced Filtering, Reporting & Exports** | Multi-select filters, column customization, Print/Excel/RSS, Activity Timeline, extended Summary & Charts | **Completed** (Milestones 1–3: multi-select filters & custom columns, Print/Word/Excel/RSS exports, Activity Timeline & 9 My View boxes, extended Summary/Billing/SVG charts) |
| **Phase 4** | **Configuration UI, Auth & SMTP** | Web UI for Workflow/Thresholds/Email Matrix, User Prefs & Profiles, Lost Password/Verification, SMTP | Pending |
| **Phase 5** | **API Ecosystem, MCP Parity, Migration & i18n** | REST API parity with MantisBT 2.28, enhanced MCP tools, XML Import/Export, localization & Timezones | Pending |

---

## 4. Detailed Phase Breakdown

### Phase 1 — Closing Half-Built Features & Quick Wins (Completed)

*Goal: Complete features that already exist partially in the schema or routes without UI/logic wiring, establishing a clean baseline before adding larger subsystems. All milestones (1–4), `valid_regexp` ReDoS-safe compilation, Roadmap subproject roll-up & per-project obsolete scoping, and the 13-scenario `tests/backward_compat_test.rs` regression suite are fully implemented and verified.*

1. **Fix `View Issues` Pagination, Sorting & Saved Filter Deletion** ([`src/models/filter.rs`](../src/models/filter.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs), [`src/controllers/filters.rs`](../src/controllers/filters.rs), [`src/views/bugs.rs`](../src/views/bugs.rs))
   - Preserve active filter query parameters across pagination links.
   - Render a Saved Filter delete button on the UI wired to the existing `POST /filter_delete/:id` route.
   - Add `per_page` selection and column sorting (`id`, `priority`, `severity`, `status`, `last_updated`, `summary`).
2. **Wire Existing Issue Columns & Project Access Rules** ([`src/models/bug.rs`](../src/models/bug.rs), [`src/db.rs`](../src/db.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs))
   - Expose UI/controller handling for `sticky` (pinning issues), `due_date` (with overdue highlighting), and `build` (product build); expose `os`/`os_build`/`platform` on `update_bug_page`.
   - Roll up **subproject** issues when filtering by a parent project in `list_bugs` / `filter_bugs` while enforcing per-subproject access checks.
   - Auto-assign `handler_id` from `category.user_id` (default assignee) on issue creation.
   - Enforce `Project.access_min` in `query_effective_access_level`.
   - Display human-readable status/resolution/priority/severity/user names in the **Issue History** UI table instead of raw numeric codes.
3. **Complete Custom Fields** ([`src/models/custom_field.rs`](../src/models/custom_field.rs), [`src/views/bugs.rs`](../src/views/bugs.rs), [`src/views/custom_fields.rs`](../src/views/custom_fields.rs), [`src/db.rs`](../src/db.rs))
   - Render appropriate HTML controls for all 11 `CustomFieldType` variants (`select`, `select multiple`, `checkbox`, `radio`, `date`, `textarea`, `email`, `number`) instead of only `<input type="text">`; move all HTML rendering out of controllers into `src/views/`.
   - Enforce `display_*`, `require_*`, `access_level_r`/`rw`, type validation, dynamic `possible_values` (`=versions`, `=released_versions`, `=future_versions`, `=categories`), and `valid_regexp` compiled with 1 MiB size cap for ReDoS safety.
4. **Complete Missing CRUD UIs & Small Parity Gaps** ([`src/controllers/projects.rs`](../src/controllers/projects.rs), [`src/views/projects.rs`](../src/views/projects.rs), [`src/controllers/planning.rs`](../src/controllers/planning.rs), [`src/views/planning.rs`](../src/views/planning.rs), [`src/controllers/tags.rs`](../src/controllers/tags.rs), [`src/views/tags.rs`](../src/views/tags.rs), [`src/controllers/auth.rs`](../src/controllers/auth.rs), [`src/views/auth.rs`](../src/views/auth.rs))
   - Add edit forms for Categories (`POST /manage_proj_cat_update/:cat_id`), Versions (`POST /manage_proj_ver_update/:ver_id` + persist `date_order`), Tags (`POST /tag_update/:id` + `tag_view_page` & issue counts), and News (`news_edit_page` / `news_update`).
   - Order **Roadmap** versions by `ProjectVersion.date_order`, roll up subproject versions with private subproject isolation, scope obsolete versions per-project, and honor `released`/`obsolete` flags.
   - Wire the `perm_login` ("Remember Me") checkbox in [`src/controllers/auth.rs`](../src/controllers/auth.rs).

---

### Phase 2 — Upgrading Daily Issue & Note Workflows (Completed)

*Goal: Deliver the full set of daily operations expected by Developers, QA engineers, and Managers in MantisBT.*

1. **Full Bugnotes, Revisions & Optimistic Locking** ([`src/db.rs`](../src/db.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs))
   - Support editing bugnote text (`bugnote_edit_page`) and toggling public/private visibility after creation (`bugnote_set_view_state`).
   - Add `mantis_bug_revision_table` to track edit history for `description`, `steps_to_reproduce`, `additional_information`, and `bugnote`, with view and drop UI.
   - Enforce `last_updated` checks on issue updates (Optimistic Locking) to prevent conflicting overwrites.
2. **Clone, Move & Bulk Operations (`bug_actiongroup`)**
   - Support **cloning issues** (with optional relationship and attachment copying) and **moving issues** across projects.
   - Add multi-select checkboxes on `View Issues` with bulk actions: Close, Resolve, Assign, Delete, Move, Copy, Update Priority/Severity/Status/Category/Version/Due Date/View State/Sticky, Add Note, and Attach Tags.
3. **Working Rich Formatting (`MantisCoreFormatting`)** ([`src/views/mod.rs`](../src/views/mod.rs))
   - Turn `MantisCoreFormatting` from a stub into an active formatter controlled by `mantis_plugin_table`:
     - Auto-link `#123` (issues, with status tooltip and strikethrough when resolved/closed), `~123` (bugnotes), and external URLs (`rel="noopener noreferrer nofollow"`).
     - Support `@username` mentions with email notifications.
     - Support safe Markdown rendering (escaping raw HTML before rendering to prevent XSS) and code block syntax highlighting.
4. **Enhanced Monitors, Reminders & Attachments**
   - List monitoring users on `view_bug_page` and allow Developers/Managers to add or remove monitors for other users.
   - Add **Send Reminder** (`bug_reminder_page`) to project members (optionally saved as a note and auto-subscribing recipients as monitors).
   - Support multi-file uploads (including during initial issue reporting), extension whitelist/blacklist filtering (blocking inline `.svg`/HTML), and inline image/text/diff previews.

---

### Phase 3 — Advanced Filtering, Reporting & Exports (Completed)

*Goal: Support complex issue queries, progress tracking, and management reporting.*

1. **Advanced Filters & Column Customization** ([`src/models/filter.rs`](../src/models/filter.rs), [`src/controllers/filters.rs`](../src/controllers/filters.rs))
   - Support multi-select filter criteria and `MATCH ALL / ANY` combinators.
   - Add filter fields for date ranges (`date_submitted`, `last_updated`), `version`/`target_version`/`fixed_in_version`, `resolution`, `reproducibility`, `view_state`, `sticky`, `relationship`, `note_user`, and **Custom Fields**.
   - Add Saved Filters management (`manage_filter_page`) and shareable filter **Permalinks**.
   - Support per-context customizable column sets (`View`, `Print`, `CSV`, `Excel`).
2. **Extended Exports & Syndication**
   - Add **HTML Print View** (`print_all_bug_page`), **Word export**, and **Excel XML export** (preserving formula-injection sanitization).
   - Add token-authenticated **RSS Feeds** (`issues_rss`, `news_rss`).
3. **My View Timeline, Extended Summary, Billing & Charts** ([`src/controllers/planning.rs`](../src/controllers/planning.rs), [`src/views/planning.rs`](../src/views/planning.rs))
   - Add the **Activity Timeline** and support configuring all 9 boxes on `My View`.
   - Expand `Summary` with breakdowns for By Project, By Category, By Resolution, By Date (opened/resolved/balance), Longest Open, Most Active, Developer×Resolution, and Reporter Effectiveness.
   - Upgrade `Billing` (Time Tracking) with date range filtering, subproject roll-up, hourly rate calculation, and CSV/Excel exports.
   - Add statistical charts & **Issues Trend** (equivalent to `MantisGraph`, preferring lightweight server-side SVG rendering or bundled local assets without external CDNs).

---

### Phase 4 — Configuration Management, Advanced Auth & Email SMTP

*Goal: Make the system enterprise-ready with runtime configuration without code edits.*

1. **Configuration Management Web UI (`Manage Configuration`)**
   - Leverage `mantis_config_table` scoped by `(project_id, user_id)` across 5 admin screens:
     - **Permissions Report** (`adm_permissions_report`)
     - **Workflow Thresholds** (`manage_config_work_threshold`)
     - **Workflow Transitions Matrix** (`manage_config_workflow` — replacing the hardcoded `90 -> 20` rule and adding dependency-blocking checks on resolve)
     - **Email Notifications Matrix** (`manage_config_email`)
     - **Configuration Report Editor** (`adm_config_report`)
2. **User Preferences, Profiles & Advanced User/Project Administration**
   - Add `mantis_user_pref_table` for granular per-event/severity email preferences, bugnote sort order, `default_project`, `refresh_delay`, and `timezone`.
   - Add `mantis_user_profile_table` for personal and global environment profiles (Platform / OS / OS Build).
   - Add **Lost Password / Email Reset**, **Email Verification** on signup and email changes, **Admin Re-authentication** (5-minute password confirmation for admin routes), **Impersonate User**, **Prune unused accounts**, project assignment on user edit pages, **Copy categories/versions/users/custom fields** across projects, and **Project Documentation** (`proj_doc_*`).
   - *(Optional based on deployment needs)*: Anonymous/Guest access, LDAP, or OIDC/SSO.
3. **SMTP Transport & System Utilities** ([`src/mailer.rs`](../src/mailer.rs))
   - Support **SMTP transport** (TLS/STARTTLS, auth) alongside the existing SendGrid API *(requires explicit approval to add a crate such as `lettre` to `Cargo.toml`)*.
   - Apply the Email Notifications matrix and `user_pref` rules when queuing emails.
   - Add **Manage Overview / System Info**, basic System Checks, DB row statistics, Database ↔ Disk attachment migration tools, and Offline maintenance mode.

---

### Phase 5 — REST API & MCP Parity, Data Migration & i18n

*Goal: Achieve full ecosystem parity for external integrations, AI agents, and migrating data from PHP MantisBT.*

1. **REST API Parity with MantisBT 2.28** ([`src/controllers/api.rs`](../src/controllers/api.rs))
   - Expand `/api/rest/issues/:id`: add `files` (upload/download/delete), `tags`, `monitors`, `relationships`, `DELETE /notes/:note_id`; support `custom_fields`, `tags`, and `handler` on create/patch, plus `filter_id` and `select` on queries.
   - Add CRUD endpoints for `/api/rest/projects` (including `versions`, `subprojects`, `users`, `handlers`), `/api/rest/users` (including `reset`, `tokens`), `/api/rest/filters`, and `/api/rest/config`.
   - Add `ETag` / `If-None-Match` support and publish an OpenAPI specification.
2. **MCP Server Enhancements** ([`src/mcp/tools.rs`](../src/mcp/tools.rs))
   - Include `tags`, `relationships`, `custom_fields`, and `attachments` in `get_issue` responses (currently `tags` is mentioned in the tool description but omitted from the JSON payload).
   - Add MCP tools for tags, relationships, filters, and attachment metadata.
3. **XML Import/Export & Localization (i18n)**
   - Support XML issue import/export compatible with upstream `XmlImportExport` (`mantis.dtd`) to ease data migration.
   - Add lightweight i18n support (starting with English + Vietnamese) and render timestamps in the user's configured `timezone` instead of fixed UTC.

---

## 5. Out of Scope / Non-Goals

- **SOAP API (`mantisconnect.php`):** Legacy protocol superseded by the REST API and MCP Server.
- **Dynamic PHP-Style Plugin System:** Poor fit for a single-binary Rust architecture; equivalent functionality should be built as compiled modules toggled via internal trait hooks (Formatting, Graphs, XML Import/Export, Gravatar).
- **Sponsorships & Legacy Wiki Adapters (TWiki, WackoWiki, WikkaWiki, etc.):** Very low usage relative to maintenance overhead.
- **Multi-Database Backends (Oracle / MS SQL / MySQL via ADOdb):** Only consider adding PostgreSQL later if deployment scale exceeds SQLite.

---

## 6. Regression Testing & Legacy Data Compatibility Strategy

### 6.1. Current Test Coverage & Gaps

- **Current baseline (~207 automated tests prior to Phase 1):**
  - ~167 unit tests organized 1:1 alongside modules (`src/*_test.rs`, `src/models/models_test.rs`, `src/controllers/*_test.rs`, `src/views/*_test.rs`).
  - 40 HTTP/integration tests under [`tests/`](../tests) (`subproject1_http_test.rs` – `subproject5_http_test.rs`, `mcp_test.rs`, `test_sendgrid_mailer.rs`, `test_jira_*.rs`), plus the manual test script [`tests/manual_test.sh`](../tests/manual_test.sh).
  - Every PR passes the mandatory quality gate (`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`), catching regressions in core flows.
- **Identified gaps to close:**
  - **No upgrade / backward-compatibility tests against legacy data:** Existing tests initialize fresh in-memory databases and do not validate reopening an existing `mantis.db` across schema or struct evolutions.
  - **Thin HTTP integration coverage for Filters, Planning, and REST API:** [`tests/subproject3_http_test.rs`](../tests/subproject3_http_test.rs) (2 tests), [`tests/subproject4_http_test.rs`](../tests/subproject4_http_test.rs) (1 test), and [`tests/subproject5_http_test.rs`](../tests/subproject5_http_test.rs) (1 test) cover minimal happy paths only.

### 6.2. Backward-Compatibility Test Suite (`tests/backward_compat_test.rs`)

Maintain a dedicated integration test suite in `tests/backward_compat_test.rs` (kept independent of fresh-DB fixtures) to verify legacy data safety across Phases 1–5:

1. **Legacy Saved Filter JSON Deserialization (`mantis_filters_table.filter_string`):**
   - Seed raw legacy `BugFilter` JSON payloads (containing only initial fields, omitting newer optional fields such as `sort`, `dir`, `per_page`, `match_type`, etc.).
   - Verify `serde_json::from_str::<BugFilter>` succeeds via `#[serde(default)]` and `GET /view_all_bug_page?filter_id=...` returns `200 OK` with expected results.
2. **Idempotent `Db::migrate()` on Pre-Populated Legacy SQLite Files:**
   - Seed a file-backed SQLite database with legacy records (issues with `sticky = 0`, `due_date = 0`, `build = ""`, projects with default `access_min = 10`, legacy versions, raw numeric `mantis_bug_history_table` rows) and older schema shapes.
   - Re-open and run `Db::migrate()` repeatedly, asserting zero lost/corrupted rows and clean execution of all read queries.
3. **Legacy Custom Field Values (`mantis_custom_field_string_table`):**
   - Seed legacy free-text values inside `Numeric`, `Float`, `Email`, `Date`, and `Enum` custom fields via raw SQL.
   - Verify `GET /view_bug_page/:id` and `GET /update_bug_page/:id` render cleanly (`200 OK`) without panics or errors, while subsequent saves enforce validation clearly (allowing empty non-required values).
4. **Access Boundaries & Legacy Visibility Semantics:**
   - Verify subproject roll-up continues isolating private subprojects from unauthorized users.
   - Verify `Project.access_min` enforces thresholds while preserving default `access_min = 10` behavior for existing public projects and explicit `mantis_project_user_list_table` overrides.
   - Verify `403 Forbidden` guards on Manager/Admin actions (`bug_stick`, Category/Version/Tag/News edits, deleting other users' filters).

### 6.3. Per-Phase Regression Test Checklist

- **Phase 1:**
  - Preserve all active filter query parameters across `/view_all_bug_page` pagination, verify URL encoding, sorting (with `sticky` pinned first), and `per_page` clamping.
  - Cover CRUD and role guards for Category, Version (`date_order`), Tag (detail page + usage counts), News edit, `bug_stick`, `due_date`/`build`/`os` updates, subproject roll-up, category default assignee, `perm_login` cookie `Max-Age`, rendering/validation of all 11 `CustomFieldType`s (including multi-value `Checkbox`/`Multilist`), and `test_no_nested_forms_in_edit_pages`.
- **Phase 2:**
  - Cover all `bug_actiongroup` bulk operations (Close/Resolve/Assign/Move/Copy/Tags) including per-project permission enforcement.
  - Cover `mantis_bug_revision_table` history, Optimistic Locking (`last_updated` conflict), Reminders, multi-user Monitor management, and XSS-safe `MantisCoreFormatting` output for Markdown / `#123` / `~123` / `@mention`.
- **Phase 3:**
  - Expand `subproject3_http_test.rs` and `subproject4_http_test.rs` for multi-select filters (`MATCH ALL` / `MATCH ANY`), column customization, Print/Excel XML exports (with formula-injection protection), authenticated RSS feeds, Activity Timeline, extended Summary tables, and date-filtered Billing.
- **Phase 4:**
  - Cover hierarchical `(project_id, user_id)` configuration in `mantis_config_table`, Workflow Transitions matrix (with default fallbacks), `mantis_user_pref_table`, `mantis_user_profile_table`, Lost Password / Email Verification / Admin Re-auth flows, and SMTP integration tests alongside SendGrid.
- **Phase 5:**
  - Expand `subproject5_http_test.rs` and [`tests/mcp_test.rs`](../tests/mcp_test.rs) to assert backward-compatible JSON payloads across REST API and MCP tools while adding new fields/endpoints (`files`, `tags`, `monitors`, `relationships`), plus `mantis.dtd` XML Import/Export round-trip tests.

### 6.4. Verification Gate & Safe Upgrade Procedure (SQLite WAL)

1. **Mandatory Pre-Merge Quality Gate:**
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --verbose
   ```
2. **Pre-Upgrade Database Backup:** Because [`src/db.rs`](../src/db.rs) enables `PRAGMA journal_mode = WAL`, uncommitted checkpoint data may reside in the `-wal` file; always back up live databases with SQLite's online backup command rather than raw file copy:
   ```bash
   sqlite3 mantis.db ".backup mantis.db.bak"
   ```
