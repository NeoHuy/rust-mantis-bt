# Sub-project 3: Advanced Views, Search, Saved Filters, Custom Fields, Tags & Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Tags, Custom Fields, Saved Filters, Full-Text Issue Search, Issue Monitoring, "My View" Dashboard, and CSV Export to `rust-mantis-bt`.

**Architecture:** 7 new SQLite tables added to `Db::migrate()`. New models in `src/models.rs`. New DB methods in `src/db.rs`. New handlers in `src/handlers/{tags,custom_fields,filters}.rs` and enhancements to `src/handlers/bugs.rs`, `src/handlers/mod.rs`, and `src/ui.rs`.

**Spec:** `docs/superpowers/specs/2026-10-12-subproject-3-search-filters-custom-fields-tags-design.md`

---

### Task 1: Schema Migrations & Models (Tags, Custom Fields, Filters, Monitors)

- [ ] Add `CustomFieldType` enum (`String=0`, `Numeric=1`, `Float=2`, `Enum=3`, `Email=4`, `Checkbox=5`, `List=6`, `Multilist=7`, `Date=8`, `Radio=9`, `Textarea=10`) and structs `Tag`, `BugTag`, `CustomField`, `CustomFieldValue`, `SavedFilter`, `BugFilter` to `src/models.rs`.
- [ ] Add 7 tables (`mantis_tag_table`, `mantis_bug_tag_table`, `mantis_custom_field_table`, `mantis_custom_field_project_table`, `mantis_custom_field_string_table`, `mantis_filters_table`, `mantis_bug_monitor_table`) to `Db::migrate()` in `src/db.rs`.

### Task 2: Database Methods — Tags, Custom Fields, Saved Filters, Search & Monitors

- [ ] Implement Tag CRUD & bug attachment methods (`create_tag`, `update_tag`, `delete_tag`, `list_tags`, `attach_tag_to_bug`, `detach_tag_from_bug`, `list_bug_tags`) with history logging (`tag_attached` / `tag_detached`).
- [ ] Implement Custom Field CRUD, project linking, and bug value read/write (`create_custom_field`, `update_custom_field`, `delete_custom_field`, `list_custom_fields`, `link_custom_field_to_project`, `unlink_custom_field_from_project`, `list_custom_fields_for_project`, `set_custom_field_value`, `get_custom_field_values_for_bug`) with length/required validation and history logging.
- [ ] Implement Saved Filters (`save_filter`, `get_filter`, `list_visible_filters`, `delete_filter`), Issue Monitors (`add_bug_monitor`, `remove_bug_monitor`, `list_bug_monitors`), and advanced `filter_bugs(&self, filter: &BugFilter, user: &User)` supporting keyword search across summary/description/steps/additional_info/bugnotes, tag filter, status/hide_status, priority, severity, category, reporter, handler, and monitored_by.
- [ ] Write unit tests in `src/db.rs` verifying Tags, Custom Fields, Saved Filters, Full-text Search, and Monitors.

### Task 3: HTTP Handlers & Integration Tests (My View, Tags, Custom Fields, Saved Filters, CSV Export)

- [ ] Implement `src/handlers/tags.rs`, `src/handlers/custom_fields.rs`, `src/handlers/filters.rs`.
- [ ] Update `src/handlers/bugs.rs` with `my_view_page`, `csv_export`, `bug_monitor_add`, `bug_monitor_delete`, enhanced `view_all_bug_page` (search/saved filters/CSV link), and custom field + tag + monitor integration in `view_bug_page`, `bug_report_page`, `bug_report`, `update_bug_page`, `update_bug`.
- [ ] Wire routes in `src/handlers/mod.rs` and navigation links in `src/ui.rs`.
- [ ] Write integration tests in `tests/subproject3_http_test.rs`.
