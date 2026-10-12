# Rust MantisBT — Sub-project 1: Core Foundation, Auth & Projects Design Spec

**Date:** 2026-10-12  
**Status:** Draft for Review  
**Scope:** Sub-project 1 of 5 (Full 1:1 MantisBT Port to Rust + SQLite)

---

## 1. Context & Roadmap Overview

The goal of `rust-mantis-bt` is a complete 1:1 functional port of MantisBT into an ultra-lightweight single-binary Rust web application backed by SQLite, preserving MantisBT's database table naming (`mantis_*_table`) and integer enum constants (`10, 25, 40, 55, 70, 90`, etc.) so data migration and workflow parity with PHP MantisBT remain seamless.

Because MantisBT spans ~32 database tables and multiple domains, the port is decomposed into 5 sequential sub-projects:

1. **Sub-project 1 (This Spec): Core Foundation, Auth & Projects** — Single-binary server setup, SQLite schema & migrations, configuration store, user management, authentication & cookie sessions, effective project-level authorization, project/sub-project hierarchy, categories, and versions.
2. **Sub-project 2: Core Issue Tracking & Workflow** — Issues (`mantis_bug_table`, `mantis_bug_text_table`), status lifecycle (`10=New` to `90=Closed`), severity/priority/reproducibility, bug notes, bug history/audit log, relationships, and attachments.
3. **Sub-project 3: Filters, Search, View Issues, My View & Summary** — Multi-criteria filtering (`mantis_filters_table`), SQLite FTS5 search, My View dashboard boxes, Summary statistics, Changelog, and Roadmap.
4. **Sub-project 4: Custom Fields, Tags, Time Tracking & System Profiles** — Dynamic custom fields (`mantis_custom_field_*`), tags (`mantis_tag_table`), time tracking/billing, and OS/Platform profiles.
5. **Sub-project 5: Email Notifications, REST API & Plugin/Webhook System** — Email queue (`mantis_email_table`), MantisBT-compatible REST API (`/api/rest/...` with `mantis_api_token_table`), and event hooks/plugins.

---

## 2. Technical Stack (Option A — Ultra-lightweight Single Binary)

- **HTTP Server:** `axum` + `tokio` (minimal features: `rt-multi-thread`, `macros`, `net`, `signal`).
- **Database:** `rusqlite` with `bundled` feature (statically compiles modern SQLite with FTS5 and STRICT table support; zero external C library dependency). Connection access is synchronized via a lightweight mutex/pool helper with `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;`.
- **HTML Rendering:** Server-side rendered HTML written directly in Rust (`maud` or typed HTML builder functions) + embedded static CSS & HTMX (`include_str!`), producing a single self-contained binary with `< 15 MB` RAM usage and zero Node.js dependency.
- **Password Hashing & Crypto:** `sha2` + `rand` (supports both salted SHA-256 for new passwords and legacy MantisBT MD5/bcrypt verification if imported from an existing MantisBT DB).

---

## 3. Database Schema (Sub-project 1 Tables)

All tables use SQLite `STRICT` mode where applicable and retain exact MantisBT table and column names:

### 3.1 `mantis_config_table`
Stores dynamic system/project/user configuration overrides:
- `config_id TEXT NOT NULL`
- `project_id INTEGER NOT NULL DEFAULT 0`
- `user_id INTEGER NOT NULL DEFAULT 0`
- `access_reqd INTEGER NOT NULL DEFAULT 0`
- `type INTEGER NOT NULL DEFAULT 90` (`1=int`, `2=string`, `3=complex/json`)
- `value TEXT NOT NULL`
- `PRIMARY KEY (config_id, project_id, user_id)`

### 3.2 `mantis_user_table`
Stores user accounts and session cookie tokens:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `username TEXT NOT NULL UNIQUE`
- `realname TEXT NOT NULL DEFAULT ''`
- `email TEXT NOT NULL DEFAULT ''`
- `password TEXT NOT NULL`
- `enabled INTEGER NOT NULL DEFAULT 1`
- `protected INTEGER NOT NULL DEFAULT 0`
- `access_level INTEGER NOT NULL DEFAULT 25` (`10=Viewer`, `25=Reporter`, `40=Updater`, `55=Developer`, `70=Manager`, `90=Administrator`)
- `login_count INTEGER NOT NULL DEFAULT 0`
- `lost_password_request_count INTEGER NOT NULL DEFAULT 0`
- `failed_login_count INTEGER NOT NULL DEFAULT 0`
- `cookie_string TEXT NOT NULL UNIQUE`
- `last_visit INTEGER NOT NULL DEFAULT 0` (Unix timestamp)
- `date_created INTEGER NOT NULL DEFAULT 0` (Unix timestamp)

*Default bootstrap seed:* If `mantis_user_table` is empty on startup, create `administrator` (password `root`, matching MantisBT's default initial install, with `access_level = 90`).

### 3.3 `mantis_project_table`
Stores projects:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `name TEXT NOT NULL UNIQUE`
- `status INTEGER NOT NULL DEFAULT 10` (`10=development`, `30=release`, `50=stable`, `70=obsolete`)
- `enabled INTEGER NOT NULL DEFAULT 1`
- `view_state INTEGER NOT NULL DEFAULT 10` (`10=public`, `50=private`)
- `access_min INTEGER NOT NULL DEFAULT 10`
- `description TEXT NOT NULL DEFAULT ''`
- `category_id INTEGER NOT NULL DEFAULT 1`
- `inherit_global INTEGER NOT NULL DEFAULT 1`

### 3.4 `mantis_project_hierarchy_table`
Stores parent-child sub-project relationships:
- `child_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `parent_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `inherit_parent INTEGER NOT NULL DEFAULT 1`
- `PRIMARY KEY (child_id, parent_id)`

### 3.5 `mantis_project_user_list_table`
Stores per-project user access level overrides and private project memberships:
- `project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `access_level INTEGER NOT NULL DEFAULT 25`
- `PRIMARY KEY (project_id, user_id)`

### 3.6 `mantis_category_table`
Stores global (`project_id = 0`) and project-specific categories:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `project_id INTEGER NOT NULL DEFAULT 0`
- `user_id INTEGER NOT NULL DEFAULT 0` (optional default assignee user ID)
- `name TEXT NOT NULL`
- `status INTEGER NOT NULL DEFAULT 0`
- `UNIQUE (project_id, name)`

*Default bootstrap seed:* Ensure a default global category `[All Projects] General` (`project_id = 0, name = 'General'`) exists on first boot, matching MantisBT.

### 3.7 `mantis_project_version_table`
Stores project versions for reporting/target version/fixed-in-version and roadmap/changelog:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `version TEXT NOT NULL`
- `description TEXT NOT NULL DEFAULT ''`
- `released INTEGER NOT NULL DEFAULT 1`
- `obsolete INTEGER NOT NULL DEFAULT 0`
- `date_order INTEGER NOT NULL DEFAULT 0` (Unix timestamp)
- `UNIQUE (project_id, version)`

---

## 4. Domain Logic & Authorization Rules

### 4.1 MantisBT Enum Constants (`src/models.rs`)
- **AccessLevel:** `Viewer = 10`, `Reporter = 25`, `Updater = 40`, `Developer = 55`, `Manager = 70`, `Administrator = 90`, `Nobody = 100`.
- **ProjectStatus:** `Development = 10`, `Release = 30`, `Stable = 50`, `Obsolete = 70`.
- **ViewState:** `Public = 10`, `Private = 50`.

### 4.2 Effective Access Level Resolution (`src/auth.rs`)
Given a `User` and `project_id`:
1. If `project_id == 0` (All Projects context), return `user.access_level`.
2. If `user.access_level >= 90` (`Administrator`), return `90`.
3. Look up `(project_id, user.id)` in `mantis_project_user_list_table`. If present, return that project-specific `access_level`.
4. Otherwise, fetch `project.view_state` from `mantis_project_table`:
   - If `project.view_state == 10` (`Public`), return `user.access_level`.
   - If `project.view_state == 50` (`Private`), return `user.access_level` only if `user.access_level >= 70` (default `private_project_threshold`), else return `0` (no access).

### 4.3 Category & Version Inheritance
When listing categories for `project_id`:
- Include categories directly owned by `project_id`.
- If `project.inherit_global == 1`, include global categories (`project_id = 0`).
- If `project_id` is a child in `mantis_project_hierarchy_table` with `inherit_parent = 1`, recursively include ancestor project categories (preventing cycles).

### 4.4 Active Project Switcher (`MANTIS_PROJECT_COOKIE`)
- Like classic MantisBT, the top navigation bar includes a project selector dropdown showing `All Projects` (`0`) and all projects visible to the logged-in user (including indented sub-projects).
- Selecting a project updates the `MANTIS_PROJECT_COOKIE` cookie and redirects back to the current page.

---

## 5. Module Structure & HTTP Routes

### 5.1 File Layout
- `Cargo.toml`: Minimal dependencies (`axum`, `tokio`, `rusqlite`, `sha2`, `rand`, `serde`, `tower-Service` / form handling).
- `src/main.rs`: Server bootstrap, CLI/env config (`MANTIS_DB_PATH`, `MANTIS_ADDR`), router wiring.
- `src/db.rs`: SQLite connection wrapper, schema migration runner, seed data, and data access methods.
- `src/models.rs`: Domain structs and MantisBT integer enum types.
- `src/auth.rs`: Password hashing/verification, cookie parsing (`MANTIS_STRING_COOKIE`, `MANTIS_PROJECT_COOKIE`), Axum request extractors (`AuthSession`), and `effective_access_level`.
- `src/ui.rs`: MantisBT layout shell (top bar with user info & project selector, left navigation sidebar, flash alerts, embedded CSS).
- `src/handlers/auth.rs`:
  - `GET /login`, `POST /login`
  - `POST /logout`
  - `POST /set_project`
  - `GET /account_page`, `POST /account_update` (update realname, email, password)
- `src/handlers/users.rs` (Requires `Administrator = 90`):
  - `GET /manage_user_page` (list users, filter by enabled/search)
  - `GET /manage_user_create_page`, `POST /manage_user_create`
  - `GET /manage_user_edit_page/:id`, `POST /manage_user_update/:id`, `POST /manage_user_delete/:id`
- `src/handlers/projects.rs` (Requires `Manager = 70` or `Administrator = 90`):
  - `GET /manage_proj_page` (list projects with hierarchy and global categories)
  - `GET /manage_proj_create_page`, `POST /manage_proj_create`
  - `GET /manage_proj_edit_page/:id`, `POST /manage_proj_update/:id`, `POST /manage_proj_delete/:id`
  - `POST /manage_proj_subproj_add/:id`, `POST /manage_proj_subproj_delete/:id/:child_id`
  - `POST /manage_proj_cat_add`, `POST /manage_proj_cat_update/:cat_id`, `POST /manage_proj_cat_delete/:cat_id`
  - `POST /manage_proj_ver_add/:id`, `POST /manage_proj_ver_update/:ver_id`, `POST /manage_proj_ver_delete/:ver_id`
  - `POST /manage_proj_user_add/:id`, `POST /manage_proj_user_remove/:id/:user_id`

---

## 6. Error Handling & Security

- **SQLite Constraints:** Foreign keys and `UNIQUE` constraints (`username`, `project.name`, `(project_id, category_name)`, `(project_id, version)`) return user-friendly HTTP 400/422 HTML pages instead of panicking.
- **Account Lockout & Protection:**
  - Users with `enabled = 0` cannot log in or use existing session cookies.
  - Users with `protected = 1` cannot have their password or access level modified by non-self or deleted, and the last active `Administrator` cannot be deleted or demoted.
  - Increment `failed_login_count` on bad password and reset to `0` on successful login (updating `login_count` and `last_visit`).
- **XSS Prevention:** All dynamic strings rendered in HTML templates are HTML-escaped.

---

## 7. Verification & Testing Strategy

Integration tests (`tests/subproject1_test.rs` or unit test module in `src/`) using in-memory SQLite (`:memory:`):
1. **Bootstrap & Default Admin Seed:** Verify schema creation, default `administrator` user (`access_level = 90`), and default `General` global category (`project_id = 0`).
2. **Authentication & Protected Account Rules:** Verify login, cookie session resolution, failed login counter, disabled account rejection, and protection against deleting the last administrator.
3. **Effective Access Level & Project Visibility:** Verify public vs. private projects, explicit `mantis_project_user_list_table` overrides, and `Manager`/`Administrator` access rules.
4. **Hierarchy, Categories & Versions:** Verify parent-child sub-project linkage, cycle prevention, category inheritance (`inherit_global` + `inherit_parent`), and project version CRUD.
5. **HTTP Handler End-to-End Flow:** Verify login → create project → add category/version → assign user to private project → switch active project cookie.
