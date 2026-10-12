# Sub-project 4: Roadmap, Changelog, Summary Statistics, Time Tracking, News & Email Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement Roadmap, Changelog, Summary Statistics, Time Tracking (`time_tracking` on Bugnotes + `/billing_page`), Project News (`mantis_news_table`), and Email Notification Queue (`mantis_email_table`) for `rust-mantis-bt`.

**Architecture:** 2 new SQLite tables (`mantis_email_table`, `mantis_news_table`) in `Db::migrate()`. New models `EmailMessage`, `NewsItem`, `TimeTrackingEntry` in `src/models.rs`. New DB methods in `src/db.rs` with automatic email notification queuing on bug lifecycle events. New HTTP handlers in `src/handlers/planning.rs` and updates to `src/handlers/bugs.rs`, `src/handlers/mod.rs`, and `src/ui.rs`.

**Spec:** `docs/superpowers/specs/2026-10-12-subproject-4-roadmap-changelog-stats-timetracking-email-design.md`

---

### Task 1: Schema, Models & DB Methods (Email Queue, News, Time Tracking, Roadmap & Changelog Queries)

- [ ] Add `EmailMessage`, `NewsItem`, `TimeTrackingEntry` structs to `src/models.rs`.
- [ ] Add `mantis_email_table` and `mantis_news_table` to `Db::migrate()` in `src/db.rs`.
- [ ] Implement `enqueue_email`, `notify_bug_event`, `list_queued_emails`, `delete_queued_email`, `flush_email_queue` in `src/db.rs` and hook `notify_bug_event` into `create_bug`, `assign_bug`, `change_bug_status`, and `add_bugnote_with_time`.
- [ ] Implement `add_bugnote_with_time` (storing `time_tracking` minutes in `mantis_bugnote_table` and recording history if `time_tracking > 0`) and `list_time_tracking(project_id)`.
- [ ] Implement `create_news`, `delete_news`, `list_news(project_id, user_access)` in `src/db.rs`.
- [ ] Write unit tests in `src/db.rs` covering email notifications on bug events, time tracking aggregation, and news visibility.

### Task 2: HTTP Handlers & Integration Tests (Roadmap, Changelog, Summary, Billing, News, Email Queue)

- [ ] Create `src/handlers/planning.rs` implementing:
  - `GET /roadmap_page`
  - `GET /changelog_page`
  - `GET /summary_page`
  - `GET /billing_page`
  - `GET /news_page`, `POST /news_add`, `POST /news_delete/:id`
  - `GET /manage_email_queue_page`, `POST /manage_email_queue_flush`, `POST /manage_email_delete/:id`
- [ ] Update `src/handlers/bugs.rs` (`bugnote_add` and `view_bug_page`) to support `time_tracking` input (minutes or `HH:MM`) and display time tracking per note + total time on bug detail.
- [ ] Wire routes in `src/handlers/mod.rs` and navigation links in `src/ui.rs`.
- [ ] Write end-to-end HTTP integration tests in `tests/subproject4_http_test.rs`.
