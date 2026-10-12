# Rust MantisBT — Sub-project 3: Advanced Views, Search, Saved Filters, Custom Fields, Tags & Export Design Spec

**Date:** 2026-10-12  
**Status:** Approved  
**Scope:** Sub-project 3 of 5 (Full 1:1 MantisBT Port to Rust + SQLite)

---

## 1. Scope & Goals

Sub-project 3 builds on Sub-projects 1 & 2 to deliver:
1. **Tags (`mantis_tag_table`, `mantis_bug_tag_table`):** Global tag library, attach/detach tags on issues, filter issues by tag, manage tags page.
2. **Custom Fields (`mantis_custom_field_table`, `mantis_custom_field_project_table`, `mantis_custom_field_string_table`):** Admin-defined custom fields (types: `0=String`, `1=Numeric`, `2=Float`, `3=Enumeration`, `4=Email`, `5=Checkbox`, `6=List`, `7=Multiselection list`, `8=Date`, `9=Radio`, `10=Textarea`), linked per project, displayed and edited on bugs, recorded in `mantis_bug_history_table`.
3. **Saved Filters & Full-Text Search (`mantis_filters_table`):** Keyword search across `summary`, `description`, `steps_to_reproduce`, `additional_information`, and `bugnote` text; combined with status, severity, priority, category, reporter, handler, hide_status, and tag filters; save/apply/delete named filters (personal or public).
4. **My View Dashboard (`/my_view_page`):** Classic MantisBT multi-box dashboard showing:
   - *Assigned to Me (Unresolved)*
   - *Reported by Me*
   - *Resolved*
   - *Unassigned*
   - *Recently Modified*
   - *Monitored by Me* (table created now for dashboard integration)
5. **CSV Export (`/csv_export`):** Export current filtered bug list as RFC 4180 CSV with formula-injection protection.

---

## 2. New Database Tables (Sub-project 3)

### 2.1 `mantis_tag_table`
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `user_id INTEGER NOT NULL DEFAULT 0`
- `name TEXT NOT NULL UNIQUE`
- `description TEXT NOT NULL DEFAULT ''`
- `date_created INTEGER NOT NULL DEFAULT 0`
- `date_updated INTEGER NOT NULL DEFAULT 0`

### 2.2 `mantis_bug_tag_table`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `tag_id INTEGER NOT NULL REFERENCES mantis_tag_table(id) ON DELETE CASCADE`
- `user_id INTEGER NOT NULL DEFAULT 0`
- `date_attached INTEGER NOT NULL DEFAULT 0`
- `PRIMARY KEY (bug_id, tag_id)`

### 2.3 `mantis_custom_field_table`
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `name TEXT NOT NULL UNIQUE`
- `type INTEGER NOT NULL DEFAULT 0` (`0=String`, `1=Numeric`, `2=Float`, `3=Enum`, `4=Email`, `5=Checkbox`, `6=List`, `7=Multilist`, `8=Date`, `9=Radio`, `10=Textarea`)
- `possible_values TEXT NOT NULL DEFAULT ''` (pipe-separated `|` values for Enum/List/Checkbox/Radio)
- `default_value TEXT NOT NULL DEFAULT ''`
- `valid_regexp TEXT NOT NULL DEFAULT ''`
- `access_level_r INTEGER NOT NULL DEFAULT 10`
- `access_level_rw INTEGER NOT NULL DEFAULT 25`
- `length_min INTEGER NOT NULL DEFAULT 0`
- `length_max INTEGER NOT NULL DEFAULT 0`
- `require_report INTEGER NOT NULL DEFAULT 0`
- `require_update INTEGER NOT NULL DEFAULT 0`
- `display_report INTEGER NOT NULL DEFAULT 1`
- `display_update INTEGER NOT NULL DEFAULT 1`
- `require_resolved INTEGER NOT NULL DEFAULT 0`
- `display_resolved INTEGER NOT NULL DEFAULT 0`
- `display_closed INTEGER NOT NULL DEFAULT 0`
- `require_closed INTEGER NOT NULL DEFAULT 0`
- `filter_by INTEGER NOT NULL DEFAULT 1`

### 2.4 `mantis_custom_field_project_table`
- `field_id INTEGER NOT NULL REFERENCES mantis_custom_field_table(id) ON DELETE CASCADE`
- `project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `sequence INTEGER NOT NULL DEFAULT 0`
- `PRIMARY KEY (field_id, project_id)`

### 2.5 `mantis_custom_field_string_table`
- `field_id INTEGER NOT NULL REFERENCES mantis_custom_field_table(id) ON DELETE CASCADE`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `value TEXT NOT NULL DEFAULT ''`
- `text TEXT NOT NULL DEFAULT ''`
- `PRIMARY KEY (field_id, bug_id)`

### 2.6 `mantis_filters_table`
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `project_id INTEGER NOT NULL DEFAULT 0`
- `is_public INTEGER NOT NULL DEFAULT 0`
- `name TEXT NOT NULL DEFAULT ''`
- `filter_string TEXT NOT NULL DEFAULT ''` (JSON-serialized `BugFilter`)

### 2.7 `mantis_bug_monitor_table`
- `user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `PRIMARY KEY (user_id, bug_id)`

---

## 3. HTTP Routes & Handlers

### 3.1 My View & Advanced Search / Export (`src/handlers/bugs.rs`)
- `GET /my_view_page` — MantisBT "My View" dashboard with 6 boxes (`Assigned to Me`, `Unassigned`, `Reported by Me`, `Resolved`, `Recently Modified`, `Monitored by Me`). Also becomes the default `/` redirect for logged-in users!
- `GET /view_all_bug_page` — Enhanced with `q` (search keyword), `category_id`, `reporter_id`, `tag_id`, `hide_status`, `filter_id` (apply saved filter), plus custom field display and tag display on `/view_bug_page/:id`.
- `GET /csv_export` — Exports filtered bugs as `text/csv` attachment (`issues.csv`), sanitizing cells starting with `=`, `+`, `-`, `@` to prevent spreadsheet formula injection.
- `POST /bug_monitor_add/:bug_id` & `POST /bug_monitor_delete/:bug_id` — Monitor/unmonitor issue.

### 3.2 Tags (`src/handlers/tags.rs`)
- `GET /manage_tags_page` — List all tags, create tag form (`>= Reporter (25)` to attach/create, `>= Manager (70)` to edit/delete).
- `POST /tag_create` — Create a new tag.
- `POST /tag_update/:id` — Update tag name/description (`>= Manager (70)`).
- `POST /tag_delete/:id` — Delete tag (`>= Manager (70)`).
- `POST /bug_tag_attach/:bug_id` — Attach existing or new comma-separated tags to a bug (`>= Reporter (25)`).
- `POST /bug_tag_detach/:bug_id/:tag_id` — Detach tag from bug (`>= Developer (55)` or user who attached it).

### 3.3 Custom Fields (`src/handlers/custom_fields.rs`)
- `GET /manage_custom_field_page` — List & create custom fields (`Administrator (90)`).
- `POST /manage_custom_field_create` — Create custom field (`Administrator (90)`).
- `GET /manage_custom_field_edit_page/:id` — Edit custom field definitions and project links (`Administrator (90)`).
- `POST /manage_custom_field_update/:id` — Update custom field (`Administrator (90)`).
- `POST /manage_custom_field_delete/:id` — Delete custom field (`Administrator (90)`).
- `POST /manage_custom_field_proj_add/:id` — Link custom field to project (`Administrator (90)`).
- `POST /manage_custom_field_proj_remove/:id/:project_id` — Unlink custom field from project (`Administrator (90)`).
- Integration into `/bug_report_page`, `/bug_report`, `/update_bug_page/:id`, `/update_bug/:id`, and `/view_bug_page/:id`: custom fields linked to the bug's project are rendered, validated (`require_report` / `require_update`, `length_min` / `length_max`, `valid_regexp`), saved to `mantis_custom_field_string_table`, and logged to `mantis_bug_history_table`.

### 3.4 Saved Filters (`src/handlers/filters.rs`)
- `POST /filter_save` — Save current filter criteria (`BugFilter`) into `mantis_filters_table` (`>= Reporter (25)` for personal filter; `>= Manager (70)` for public filter `is_public=1`).
- `POST /filter_delete/:id` — Delete saved filter (owner or `>= Manager (70)`).
