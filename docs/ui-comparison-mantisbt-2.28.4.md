# Side-by-Side Live UI Comparison: MantisBT 2.28.4 vs. `rust-mantis-bt`

This document captures a direct side-by-side browser comparison between upstream **MantisBT 2.28.4** and **`rust-mantis-bt`** (on `develop` commit `6a341ba`), combined with source-level inspection of [`src/views/layout.rs`](../src/views/layout.rs), [`src/views/bugs.rs`](../src/views/bugs.rs), [`src/views/projects.rs`](../src/views/projects.rs), [`src/views/planning.rs`](../src/views/planning.rs), and upstream MantisBT templates/CSS.

---

## 1. Executive Summary & Test Setup

### Live Comparison Environment
- **Upstream MantisBT 2.28.4:** Executed via temporary MariaDB 11.4 + PHP 8.3 container stack (`http://127.0.0.1:8088`, schema v213 installed cleanly via `admin/install.php`).
- **`rust-mantis-bt`:** Executed from the local `develop` build (`http://127.0.0.1:3003`, clean SQLite WAL DB).
- **Seeded Test Dataset (matching on both instances):** `Demo Project`, Category `Backend`, Versions `v1.0` (Released) and `v2.0` (Planned), Issue #1 (`Login form alignment issue`, note + `ui-audit` tag + `target_version = v2.0`), Issue #2 (`Roadmap progress check`, resolved `fixed` in `v1.0`), and News item `Welcome to Demo Project` (enabled on `rust-mantis-bt`; disabled by default in upstream MantisBT 2.28.4).
- **Viewports Checked:** Desktop (`1280x900`) and Mobile (`390x844`).

### High-Level Verdict
`rust-mantis-bt` faithfully reproduces the **Ace Admin / MantisBT 2.x visual identity** (top navbar, left sidebar, breadcrumbs, 7-color status palette, and blue panel headers) inside a single self-contained binary with zero external CDN dependencies. However, live side-by-side testing uncovered **two critical usability gaps** (broken mobile layout at `390x844` and non-functional sidebar collapse) and **multiple CSS contrast, duplicate-flash, and form-UX defects** compared with MantisBT 2.28.4.

---

## 2. Screen-by-Screen Comparison (9 Areas)

### 2.1. Login & Authentication Screens
| Dimension | MantisBT 2.28.4 (`login_page.php`, `login_password_page.php`) | `rust-mantis-bt` (`/login`, `/signup_page`) |
| :--- | :--- | :--- |
| **Visual Style** | White page canvas, centered MantisBT logo, light-gray card (`#f2f2f2`), FontAwesome field icons (`fa-user`, `fa-lock`), orange focus ring, right-aligned charcoal `Login` button. | Dark background, centered compact card with Unicode lock icon, full-width blue `Login` button, `Signup for a new account` link below. |
| **Login Flow** | **2-step login** (username first, then password + `"Keep me logged in"` unchecked + `"Only allow your session to be used from this IP address"` checked). | **Single-step login** (username + password in one form, `"Remember my login on this browser"` checked by default) + direct `/signup_page`. |
| **Security Prompts** | Displays yellow warning box if `admin/` directory exists, default `administrator` password is unchanged, or `$g_path` is unset; prompts re-auth when opening Manage pages. | No default-password / deployment warning banner and no admin re-authentication gate. |
| **UX Note** | Better for per-user SSO/LDAP discovery and admin hardening. | Faster daily login for small teams; `/signup_page` works out of the box when `allow_signup` is enabled. |

---

### 2.2. Global Shell & Mobile Responsiveness (`1280x900` & `390x844`)
| Dimension | MantisBT 2.28.4 (`core/layout_api.php`) | `rust-mantis-bt` ([`src/views/layout.rs`](../src/views/layout.rs)) |
| :--- | :--- | :--- |
| **Frontend Stack** | Bootstrap 3.4.1 + Ace Admin (`ace.css`, `ace-mantis.css`) + Font Awesome 4.7 + jQuery/Dropzone/List.js/Datetimepicker. | Custom hand-written CSS (~34 KB inline in `layout.rs`), Unicode glyphs (`▦`, `☰`, `✎`, `⚑`, `⚙`), minimal inline JS. |
| **Top Navbar** | Dark charcoal/blue navbar with `Report Issue`, `Invite Users`, styled Project dropdown menu, and User dropdown (`My Account`, `RSS`, `Logout`). | Blue header with green `+ Report Issue`, native `<select>` `Project:` switcher, static user pill, and standalone red `Logout` button. |
| **Left Sidebar** | `125px` width with stacked icon-above-label items; **`«` button collapses cleanly to `43px` icon-only (`.menu-min`)**. | `190px` width (`min-width: 190px`) with horizontal icon+label items; **`«` button has no click handler or collapsed state**. |
| **Tall Page Behavior** | Sidebar stays aligned without awkward dead space. | On tall scrolled pages, sidebar menu items scroll out of view leaving a blank vertical column with `«` pinned at the bottom. |
| **Breadcrumbs & Footer** | Breadcrumb bar + **`Recently Visited: 0000002, 0000001`** + `Issue #` input; footer with copyright & MantisBT logo. | Breadcrumb bar + `Issue #` input (no `Recently Visited` strip); footer with `MantisBT 2.x (Rust + SQLite Single-Binary Edition)`. |
| **Mobile (`390x844`)** | Collapses navbar into a `45px` bar with a hamburger menu drawer and wraps tables in `.table-responsive` horizontal scrollers (`scrollWidth = 375`). | **Broken layout (no `@media` queries):** `190px` sidebar stays permanently open, crushing main content into `~200px`, wrapping `+ Report Issue` across 3 lines, and clipping `Logout` & tables off-screen. |

---

### 2.3. My View Dashboard
| Dimension | MantisBT 2.28.4 (`my_view_page.php`) | `rust-mantis-bt` (`/my_view_page`) |
| :--- | :--- | :--- |
| **Layout** | **2-column split:** 6 collapsible issue boxes stacked on the left + **Activity Timeline** card on the right (`Prev` pagination + timestamps + event icons). | **2x3 grid** of 6 non-collapsible boxes (`Assigned to Me (Unresolved)`, `Unassigned`, `Reported by Me`, `Resolved`, `Recently Modified`, `Monitored by Me`). |
| **Issue Row Details** | Status square swatch, edit pencil shortcut, priority icon, category, timestamp, and **strikethrough ID** on resolved/closed issues (`0000002`). | Left status color border, ID, status text, summary, and `Updated:` timestamp (no edit icon, priority icon, or strikethrough ID). |
| **Status Legend** | Status colors shown inline per box. | Renders a horizontal **7-color status legend bar** (`new` through `closed`) at the bottom. |

---

### 2.4. View Issues & Filters
| Dimension | MantisBT 2.28.4 (`view_all_bug_page.php`) | `rust-mantis-bt` (`/view_all_bug_page`) |
| :--- | :--- | :--- |
| **Filter Panel** | **Collapsed by default** into a slim bar with `Reset`, inline `Search`, filter menu icon, and expand chevron; expands to **26+ criteria** (versions, dates, OS/platform, relationships, sticky, note author, match type `ALL`/`ANY`, highlight changed hours). | **Always expanded** 9-dropdown filter panel (`Reporter`, `Assigned To`, `Monitored By`, `Category`, `Severity`, `Priority`, `Status`, `Hide Status`, `Tag`) + `Search` + `Save Current Filter`. |
| **Issues Table** | **11 columns** (`Checkbox`, `Edit`, `P`, `ID`, `Notes`, `Attachment`, `Category`, `Severity`, `Status` + assignee, `Updated`, `Summary`) inside responsive container. | **8 columns** (`ID`, `Project`, `Category`, `Severity`, `Priority`, `Status`, `Summary`, `Updated ▼`), slightly overflows horizontally at `1280x900`. |
| **Actions & Exports** | `Select All` + bulk actions dropdown (`Close`, `Resolve`, `Assign`, `Move`, `Copy`, `Set Sticky`, `Add Note`, `Attach Tags`, `Delete`, etc.) + `Print Reports`, `CSV Export`, `Excel Export`, `Summary`. | No bulk-selection checkboxes/actiongroup; exports limited to `CSV Export` + `Report Issue` button; `Per page: 10 \| 25 \| 50 \| 100`. |
| **Contrast Issues** | Clean high-contrast Ace buttons. | `Save Current Filter` and unselected `Per page` buttons use `.btn-secondary` (**white text on `#abbac3`**, low contrast). |

---

### 2.5. Report Issue
| Dimension | MantisBT 2.28.4 (`bug_report_page.php`) | `rust-mantis-bt` (`/bug_report_page`) |
| :--- | :--- | :--- |
| **Form Layout** | 16-row table with left-aligned labels and red `*` required markers. | 14-row table with right-aligned blue labels in shaded label cells and blue `*` markers. |
| **Available Inputs** | Includes `Select Profile` (`Or Fill In +`), `Assign To`, **`Target Version`** (unreleased versions), `Attach Tags` + `Existing tags` dropdown, **Dropzone file upload**, `View Status` radios, and `Report Stay` checkbox. | Missing `Select Profile`, `Assign To`, **`Target Version`** on report, `Tags`, initial file upload, and `Report Stay`. |
| **Field Quirks** | Clean single `OS Build` field in profile expansion + `Product Build`. | Renders a `Platform / OS / Build` row (containing `OS Build`) **and** a separate `Build` row right below it, which can confuse users. |

---

### 2.6. View Issue Detail, Toolbar, Notes & History
| Dimension | MantisBT 2.28.4 (`view.php?id=1`) | `rust-mantis-bt` (`/view_bug_page/1`) |
| :--- | :--- | :--- |
| **Sections & Collapsibility** | Collapsible panels (`View Issue Details`, `Relationships`, `Users monitoring this issue`, `Activities`, `Add Note`, `Issue History`) + top `Send a Reminder` / `Jump to Notes` / `Jump to History` links. | Non-collapsible `.panel` cards (`View Issue Details`, `Tags`, `Relationships`, `Attached Files`, `Activities / Bugnotes`, `Issue History`). |
| **Action Toolbar** | Bottom action bar with outline buttons: `Edit`, `Assign To`, `Change Status To`, `Monitor`, `Stick`, `Clone`, `Close`, `Delete`. | Inline action bar: `Edit`, `Monitor`, `Status` + `Resolution` + inline `Optional status note` + `Change Status`, `Assign To`, `Stick`, `Delete Issue`. Missing `Clone`, `Close`, `Send a Reminder`, and `Users monitoring` list. |
| **Notes & Formatting** | Card/timeline activities with user avatar icon, `~0000001` anchor, edit/private-toggle actions, Dropzone attachment upload, and Markdown / `#123` formatting. | Tabular bugnotes (`Author \| Note \| Time Spent \| Visibility \| Actions`) with **inline `Time Tracking (HH:MM or mins)`** input; only `Delete` action on notes; plain-text body rendering. |
| **UI Defects Observed** | Single clean flash feedback. | 1) **Duplicate flash banner** after status/issue updates (`render_view_bug_page` renders `.flash-success` inside body while `render_layout` already renders it above). 2) Low-contrast white-on-`#abbac3` `.btn-secondary` buttons (`Monitor`, `Change Status`, `Assign To`, `Stick`, `Attach Tags`, `Add Relationship`, `Upload File`). |

---

### 2.7. Update Issue
| Dimension | MantisBT 2.28.4 (`bug_update_page.php`) | `rust-mantis-bt` (`/update_bug_page/1`) |
| :--- | :--- | :--- |
| **Layout & Controls** | Multi-column metadata grid matching View Issue, including `Status`, `Resolution`, `Assigned To`, `<select>` dropdowns for `Product Version` / `Target Version` / `Fixed in Version`, and inline `Add Note` + `Time Tracking`. | Vertical 2-column table (`Category` to `Additional Information`) with `Update Information` button. Omits `Status`, `Resolution`, `Assigned To`, and `Add Note` (handled on `/view_bug_page/:id` instead). |
| **Version Input Defect** | Versions are constrained `<select>` dropdowns populated from the project's version list. | `Product Version`, `Fixed in Version`, and `Target Version` are **free-text `<input type="text">` fields** ([`src/views/bugs.rs#L1472-L1480`](../src/views/bugs.rs#L1472-L1480)), risking typos and mismatched Roadmap/Changelog strings. |

---

### 2.8. Planning & Reports (`Roadmap`, `Changelog`, `Summary`, `News`)
| Dimension | MantisBT 2.28.4 | `rust-mantis-bt` |
| :--- | :--- | :--- |
| **Roadmap** | `roadmap_page.php` renders **only unreleased versions** (`v2.0`) with `Scheduled For Release` date, `[Category]` badges, and `View Issues` filter links. | `/roadmap_page` renders **both unreleased (`v2.0 Planned`) and released (`v1.0 Released`)** versions; lacks category tags, scheduled release dates, and `View Issues` links. |
| **Changelog** | `changelog_page.php` renders released versions with release date and category badges. | `/changelog_page` cleanly groups resolved/closed issues (`status >= 80`) by `fixed_in_version`. |
| **Summary** | **14 detailed tables** (`By Project`, `By Status`, `By Severity`, `By Category`, `Time Stats`, `Developer Stats`, `By Date`, `Most Active`, `Longest Open`, `By Resolution`, `By Priority`, `Reporter Stats`, `Reporter Effectiveness`, `Reporter By Resolution`) with ratios. | **5 simplified cards** (`Overview`, `By Status`, `By Severity`, `By Priority`, `By Reporter / Developer`) showing raw totals only. |
| **News** | Disabled by default (`news_list_page.php` returns `403 Access Denied`). | Enabled in sidebar (`/news_page`) with card list and `Post News Item` form; **critical styling bug** on card header `Edit` button (`.panel-header a` specificity overrides `.btn-secondary` with `rgba(255,255,255,0.18)` and overflows below the blue header onto white background, rendering white text on white). |

---

### 2.9. Admin / Manage Pages
| Dimension | MantisBT 2.28.4 | `rust-mantis-bt` |
| :--- | :--- | :--- |
| **Manage Tabs** | 8 tabs: `[i]` (`manage_overview_page.php`), `Users`, `Projects`, `Tags`, `Custom Fields`, `Global Profiles`, `Plugins`, `Configuration`. | 7 tabs: `Manage Users`, `Manage Projects`, `Manage Tags`, `Manage Custom Fields`, `Manage Plugins`, `Manage Jira`, `Email Queue` (`/manage_overview_page` returns `404`; `Global Profiles` & `Configuration` absent). |
| **Manage Projects** | Sortable project table, `Copy From/To`, `Exclude inherited categories`, separate Add vs Add & Edit Version (`Released` **unchecked** by default). | Combines Projects + `Global Categories`; `/manage_proj_create_page` shows raw status numbers (`development (10)`); **`Released` checkbox is checked by default** on Add Version ([`src/views/projects.rs#L535`](../src/views/projects.rs#L535)); `/manage_proj_edit_page/:id` lists parent project inside subproject dropdown (blocked server-side on submit). |
| **Manage Custom Fields** | Compact table + inline `Name` + `Add` form; full field config on edit page. | Full 11-option `New Custom Field` form rendered inline on `/manage_custom_field_page`; `/manage_custom_field_edit_page/:id` displays numeric type code (`0`) instead of `String`. |
| **Manage Users** | `A–Z / 0–9 / Unused / New` quick filters, `Hide Inactive` / `Show Disabled`, 9 sortable columns (`Date Created`, `Last Visit`, `Failed Logins`). | `Search` + `Enabled only` + 7 non-sortable columns; low-contrast `.btn-secondary` on `Reset` and `Edit` buttons. |

---

## 3. Prioritized List of Observed UI Defects in `rust-mantis-bt`

### P1 — High Impact (Usability & Visual Accessibility)
1. **Broken Mobile Responsive Layout (`390x844`):** [`src/views/layout.rs`](../src/views/layout.rs) has zero `@media` rules; `aside.sidebar` stays fixed at `190px`, squeezing main content to `~200px` and clipping top navbar controls and tables.
2. **Low-Contrast `.btn-secondary` & Invisible News `Edit` Link:**
   - `.btn-secondary` ([`src/views/layout.rs#L403-L408`](../src/views/layout.rs#L403-L408)) sets `color: #fff !important` on `#abbac3` (~1.98:1 contrast ratio — fails WCAG AA).
   - On `/news_page`, `<a class="btn btn-secondary btn-sm">Edit</a>` ([`src/views/planning.rs#L406`](../src/views/planning.rs#L406)) inside `.panel-header` is overridden by `.panel-header a` (`background: rgba(255,255,255,0.18); color: #fff !important;` at [`src/views/layout.rs#L292-L298`](../src/views/layout.rs#L292-L298)) and drops onto the white card body, making `"Edit"` nearly invisible.
3. **Non-Functional Sidebar Collapse (`«`):** `<div class="sidebar-collapse">&laquo;</div>` ([`src/views/layout.rs#L759`](../src/views/layout.rs#L759)) has no click event handler or collapsed CSS state.

### P2 — Medium Impact (Workflow Friction & Data Consistency)
4. **Duplicate Flash Banner on `/view_bug_page/:id`:** [`src/views/bugs.rs#L1145`](../src/views/bugs.rs#L1145) injects `.flash-success` inside page body while `render_layout` already renders the flash banner at the top.
5. **Version Inputs as Free-Text on `/update_bug_page/:id`:** [`src/views/bugs.rs#L1472-L1480`](../src/views/bugs.rs#L1472-L1480) uses `<input type="text">` for `version`, `fixed_in_version`, and `target_version` instead of `<select>` populated from project versions.
6. **`Released` Checked by Default on Add Version:** [`src/views/projects.rs#L535`](../src/views/projects.rs#L535) defaults new versions to `Released = checked`, causing newly added future milestones to appear released immediately.
7. **Horizontal Table Overflow on `/view_all_bug_page`:** Issues table lacks an `overflow-x: auto` wrapper, causing slight horizontal overflow at `1280x900`.
8. **Missing `Target Version` on `/bug_report_page`:** Users cannot set `Target Version` during initial bug creation.

### P3 — Low Impact (Polish & Consistency)
9. **Custom Field Edit Type Label:** `/manage_custom_field_edit_page/:id` displays the raw numeric type code (`0`) instead of its label (`String`).
10. **Project Status Dropdown on `/manage_proj_create_page`:** Displays raw numeric suffix (`development (10)`).
11. **Redundant `OS Build` vs `Build` Labels on Issue Forms:** Placing `OS Build` inside `Platform / OS / Build` immediately above `Build` creates visual ambiguity.
12. **Parent Project Shown in Subproject Dropdown:** `/manage_proj_edit_page/:id` includes ancestor projects in the subproject `<select>` (rejected on submit, but preventable in UI).
13. **Roadmap Includes Already-Released Versions:** `/roadmap_page` lists `Released` versions alongside `Planned` versions, whereas upstream MantisBT reserves `roadmap_page.php` strictly for unreleased targets.

---

## 4. Recommended UI Improvement Plan

### Track A — Quick CSS & Template Polish PR (`fix/ui-qa-polish`)
*(Zero backend schema changes; low-risk visual & form fixes)*
1. Fix `.btn-secondary` contrast (darker slate `#6c757d` or dark text `#393939` on `#e7ebee` with `#c5cdd4` border) and fix `.panel-header` flex alignment + `.panel-header a` override for `Edit`/`Delete` action buttons on `/news_page`.
2. Add `@media (max-width: 768px)` responsive rules in [`src/views/layout.rs`](../src/views/layout.rs) (hamburger sidebar toggle, wrapping navbar controls cleanly, `.table-responsive { overflow-x: auto; }` around wide data tables).
3. Wire `<div class="sidebar-collapse">&laquo;</div>` with lightweight CSP-safe inline JS + `.sidebar-min` CSS rules (persisted in `localStorage` or cookie).
4. Remove the duplicate `.flash-success` block inside `render_view_bug_page` ([`src/views/bugs.rs#L1145`](../src/views/bugs.rs#L1145)).
5. Uncheck `released` by default on the Add Version form ([`src/views/projects.rs#L535`](../src/views/projects.rs#L535)).
6. Convert `version`, `fixed_in_version`, and `target_version` on `/update_bug_page/:id` to `<select>` dropdowns (preserving any legacy non-matching string as an extra selected `<option>` for backward compatibility), and add `target_version` `<select>` on `/bug_report_page`.
7. Display `CustomFieldType::as_str()` on `/manage_custom_field_edit_page/:id`, exclude ancestor IDs from the subproject dropdown, and clarify `OS Build` vs `Product Build` labels.

### Track B — Structural UX Parity (Aligned with [roadmap-mantisbt-parity.md](./roadmap-mantisbt-parity.md))
- **Phase 2:** Collapsible filter & detail panels, `Bug Actiongroup` checkboxes on `View Issues`, `Clone`/`Move`/`Reminder` buttons & `Users monitoring` list on `view_bug_page`, and `MantisCoreFormatting` rich text rendering.
- **Phase 3:** `Print` / `Excel` / `RSS` export buttons, `My View` Activity Timeline, `Summary` extended tables + charts, and optionally separating `Released` versions out of Roadmap into Changelog.
- **Phase 4:** `Manage Overview` (`/manage_overview_page`), `Manage Configuration` tabs, `Recently Visited` breadcrumb strip, and lightweight inline SVG icons replacing mismatched Unicode glyphs.
