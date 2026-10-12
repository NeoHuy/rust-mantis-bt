# Sub-project 5: REST API, API Tokens & Plugin System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement API Tokens (`mantis_api_token_table`), MantisBT-compatible JSON REST API (`/api/rest/...`), and the Plugin & Event Hook System (`mantis_plugin_table`) for `rust-mantis-bt`.

**Architecture:** 2 new SQLite tables (`mantis_api_token_table`, `mantis_plugin_table`) in `Db::migrate()`. New models `ApiToken`, `PluginItem` in `src/models.rs`. Token hashing (`sha256_hex`) in `src/auth.rs` + `ApiAuth` extractor for `/api/rest/...`. New HTTP & REST handlers in `src/handlers/api.rs`.

**Spec:** `docs/superpowers/specs/2026-10-13-subproject-5-rest-api-tokens-plugins-design.md`

---

### Task 1: Schema, Models, Token Auth & Plugin DB Methods
- [ ] Add `ApiToken` and `PluginItem` structs to `src/models.rs`.
- [ ] Add `hash_api_token(raw: &str) -> String` and `generate_api_token() -> String` to `src/auth.rs`.
- [ ] Add `mantis_api_token_table` and `mantis_plugin_table` (with default plugin seeds `MantisCoreFormatting` and `EventAuditLog`) to `Db::migrate()` in `src/db.rs`.
- [ ] Implement DB methods in `src/db.rs`:
  - `create_api_token(user_id, name) -> Result<(i64, String), String>`
  - `list_api_tokens(user_id) -> rusqlite::Result<Vec<ApiToken>>`
  - `revoke_api_token(id, user_id) -> rusqlite::Result<bool>`
  - `authenticate_api_token(raw_token) -> rusqlite::Result<Option<User>>` (verifies SHA-256 hash, checks `user.enabled == 1`, updates `date_used`)
  - `list_plugins() -> rusqlite::Result<Vec<PluginItem>>`, `set_plugin_enabled(basename, enabled) -> Result<(), String>`, `is_plugin_enabled(basename) -> bool`
  - Hook `EventAuditLog` plugin into `create_bug`, `change_bug_status`, and `insert_bugnote_inner` (when `EventAuditLog` is enabled, records event entry in `mantis_bug_history_table` or `mantis_config_table` counter `plugin_event_audit_count`).
- [ ] Write unit tests in `src/db.rs` for API token creation, authentication, disabled-user rejection, revocation, and plugin toggling.

### Task 2: REST API Endpoints, Token/Plugin Web UI & Integration Tests
- [ ] Create `src/handlers/api.rs` with:
  - Web UI routes: `GET /api_tokens_page`, `POST /api_token_create`, `POST /api_token_revoke/:id`, `GET /manage_plugin_page`, `POST /manage_plugin_toggle/:basename`.
  - JSON REST routes (authenticated via `Authorization` header with raw token or `Bearer <token>`):
    - `GET /api/rest/users/me`
    - `GET /api/rest/projects`
    - `GET /api/rest/projects/:id`
    - `GET /api/rest/issues`
    - `GET /api/rest/issues/:id`
    - `POST /api/rest/issues`
    - `PATCH /api/rest/issues/:id`
    - `DELETE /api/rest/issues/:id`
    - `POST /api/rest/issues/:id/notes`
- [ ] Wire routes in `src/handlers/mod.rs` and navigation links (`API Tokens`, `Manage Plugins`) in `src/ui.rs`.
- [ ] Write end-to-end HTTP + REST integration tests in `tests/subproject5_http_test.rs`.
