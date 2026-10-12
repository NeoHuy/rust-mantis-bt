# Rust MantisBT — Sub-project 4: Roadmap, Changelog, Summary Statistics, Time Tracking, News & Email Queue Design Spec

**Date:** 2026-10-12  
**Status:** Approved  
**Scope:** Sub-project 4 of 5 (Full 1:1 MantisBT Port to Rust + SQLite)

---

## 1. Scope & Goals

Sub-project 4 delivers the release planning, analytics, time tracking, project announcements, and email notification subsystem of MantisBT:
1. **Roadmap (`/roadmap_page`):** Groups issues by `target_version` for each visible project, calculates completion percentage (`resolved/closed` vs total issues targeted for that version), and lists each issue with status, handler, and strike-through for resolved/closed issues.
2. **Changelog (`/changelog_page`):** Groups resolved/closed issues (`status >= 80`) by `fixed_in_version` for released or active project versions.
3. **Summary / Statistics (`/summary_page`):** Project & global statistics breakdown by Status, Severity, Priority, Category, Reporter, and Developer (Handler) (`>= Manager (70)` or configurable summary threshold).
4. **Time Tracking (`time_tracking` on Bugnotes + `/billing_page`):**
   - `POST /bugnote_add/:bug_id` and `Db::add_bugnote_with_time` accept `time_tracking` (minutes spent, e.g. `45` or `1:30` parsed as minutes).
   - Displays time spent per bugnote, total time spent on `/view_bug_page/:id`, and a Time Tracking / Billing report page (`GET /billing_page`) summarizing minutes/hours per user and per bug (`>= Developer (55)`).
5. **Project News / Announcements (`mantis_news_table`, `/news_page`):** Manager+ (`>= 70`) can post announcements (global or per-project, headline + body, sticky/announcement flag, public/private view_state), visible to users on `/news_page`.
6. **Email Notification Queue (`mantis_email_table`, `/manage_email_queue_page`):**
   - Automatic queuing of notification emails (`enqueue_email`) into `mantis_email_table` when a bug is reported, assigned, status-changed, or commented on (notifying reporter, assigned handler, and all users monitoring the bug in `mantis_bug_monitor_table`).
   - Admin view (`GET /manage_email_queue_page`) and manual/batch flush (`POST /manage_email_queue_flush`, `POST /manage_email_delete/:id`).

---

## 2. New Database Tables (Sub-project 4)

### 2.1 `mantis_email_table`
- `email_id INTEGER PRIMARY KEY AUTOINCREMENT`
- `email TEXT NOT NULL DEFAULT ''`
- `subject TEXT NOT NULL DEFAULT ''`
- `metadata TEXT NOT NULL DEFAULT ''`
- `body TEXT NOT NULL DEFAULT ''`
- `submitted INTEGER NOT NULL DEFAULT 0`

### 2.2 `mantis_news_table`
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `project_id INTEGER NOT NULL DEFAULT 0`
- `poster_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `date_posted INTEGER NOT NULL DEFAULT 0`
- `last_modified INTEGER NOT NULL DEFAULT 0`
- `view_state INTEGER NOT NULL DEFAULT 10`
- `announcement INTEGER NOT NULL DEFAULT 0`
- `headline TEXT NOT NULL DEFAULT ''`
- `body TEXT NOT NULL DEFAULT ''`

---

## 3. HTTP Routes & Handlers

### 3.1 Planning & Analytics (`src/handlers/planning.rs`)
- `GET /roadmap_page` — Release roadmap by `target_version` with progress bar & completion percentage.
- `GET /changelog_page` — Release changelog by `fixed_in_version`.
- `GET /summary_page` — Issue statistics by Status, Severity, Priority, Category, Reporter, and Handler.
- `GET /billing_page` — Time tracking summary report by bug and user (`>= Developer (55)`).

### 3.2 News & Email Queue (`src/handlers/planning.rs`)
- `GET /news_page` — View project/global news & announcements (plus post form for `>= Manager (70)`).
- `POST /news_add` — Create news item (`>= Manager (70)`).
- `POST /news_delete/:id` — Delete news item (`>= Manager (70)`).
- `GET /manage_email_queue_page` — View pending emails in `mantis_email_table` (`Administrator (90)`).
- `POST /manage_email_queue_flush` — Flush/send queued emails (`Administrator (90)`).
- `POST /manage_email_delete/:id` — Delete queued email (`Administrator (90)`).
