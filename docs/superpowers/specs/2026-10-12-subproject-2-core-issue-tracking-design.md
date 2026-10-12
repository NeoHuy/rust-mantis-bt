# Rust MantisBT — Sub-project 2: Core Issue Tracking & Workflow Design Spec

**Date:** 2026-10-12  
**Status:** Draft for Review  
**Scope:** Sub-project 2 of 5 (Full 1:1 MantisBT Port to Rust + SQLite)

---

## 1. Foundation from Sub-project 1

Sub-project 1 delivered:
- `src/db.rs` — `Db` wrapper with `Arc<Mutex<Connection>>`, schema migration runner, all 7 foundation tables.
- `src/models.rs` — `AccessLevel`, `ProjectStatus`, `ViewState` enums + domain structs.
- `src/auth.rs` — cookie session auth, `AuthContext` extractor, constant-time password verification.
- `src/ui.rs` — `render_layout`, `html_escape`, MantisBT nav/sidebar shell.
- `src/handlers/{auth,users,projects}.rs` — full user/project/category/version management HTTP endpoints.

Sub-project 2 adds the issue lifecycle system on top of this foundation, adding 6 new tables and the corresponding handlers and DB methods. It does NOT touch Sub-project 1 tables beyond reading from them (foreign keys).

---

## 2. New Database Tables (Sub-project 2)

All tables use SQLite `STRICT` (where applicable), `PRAGMA foreign_keys = ON;`, and retain exact MantisBT column names.

### 2.1 `mantis_bug_table`
Core bug/issue record:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE`
- `reporter_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `handler_id INTEGER NOT NULL DEFAULT 0` (assigned developer; 0 = unassigned)
- `duplicate_id INTEGER NOT NULL DEFAULT 0` (ID of the bug this is a duplicate of; 0 = none)
- `priority INTEGER NOT NULL DEFAULT 30` (`10=none`, `20=low`, `30=normal`, `40=high`, `50=urgent`, `60=immediate`)
- `severity INTEGER NOT NULL DEFAULT 50` (`10=feature`, `20=trivial`, `30=text`, `40=tweak`, `50=minor`, `60=major`, `70=crash`, `80=block`)
- `reproducibility INTEGER NOT NULL DEFAULT 70` (`10=always`, `30=sometimes`, `50=random`, `70=have not tried`, `90=unable to reproduce`, `100=N/A`)
- `status INTEGER NOT NULL DEFAULT 10` (`10=new`, `20=feedback`, `30=acknowledged`, `40=confirmed`, `50=assigned`, `80=resolved`, `90=closed`)
- `resolution INTEGER NOT NULL DEFAULT 10` (`10=open`, `20=fixed`, `30=reopened`, `40=unable to reproduce`, `50=not fixable`, `60=duplicate`, `70=not a bug`, `80=suspended`, `90=wont fix`)
- `category_id INTEGER NOT NULL DEFAULT 0`
- `date_submitted INTEGER NOT NULL DEFAULT 0` (Unix timestamp)
- `last_updated INTEGER NOT NULL DEFAULT 0` (Unix timestamp)
- `os TEXT NOT NULL DEFAULT ''`
- `os_build TEXT NOT NULL DEFAULT ''`
- `platform TEXT NOT NULL DEFAULT ''`
- `version TEXT NOT NULL DEFAULT ''` (Affected Version)
- `fixed_in_version TEXT NOT NULL DEFAULT ''`
- `target_version TEXT NOT NULL DEFAULT ''`
- `build TEXT NOT NULL DEFAULT ''`
- `summary TEXT NOT NULL DEFAULT ''`
- `view_state INTEGER NOT NULL DEFAULT 10` (`10=public`, `50=private`)
- `sticky INTEGER NOT NULL DEFAULT 0`
- `due_date INTEGER NOT NULL DEFAULT 0`

### 2.2 `mantis_bug_text_table`
Stores the long description and steps-to-reproduce (split table design from MantisBT PHP to allow efficient listing without loading full text):
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `description TEXT NOT NULL DEFAULT ''`
- `steps_to_reproduce TEXT NOT NULL DEFAULT ''`
- `additional_information TEXT NOT NULL DEFAULT ''`

> Note: `mantis_bug_table.id == mantis_bug_text_table.id` (same primary key, 1:1 relationship — inserted together).

### 2.3 `mantis_bugnote_table`
Bug comments/notes:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `reporter_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `bugnote_text_id INTEGER NOT NULL` (references `mantis_bugnote_text_table.id`)
- `view_state INTEGER NOT NULL DEFAULT 10` (`10=public`, `50=private`)
- `date_submitted INTEGER NOT NULL DEFAULT 0`
- `last_modified INTEGER NOT NULL DEFAULT 0`
- `note_type INTEGER NOT NULL DEFAULT 0`
- `note_attr TEXT NOT NULL DEFAULT ''`
- `time_tracking INTEGER NOT NULL DEFAULT 0` (minutes; reserved for Sub-project 4)

### 2.4 `mantis_bugnote_text_table`
Note text (separate to match MantisBT schema):
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `note TEXT NOT NULL DEFAULT ''`

### 2.5 `mantis_bug_history_table`
Audit log of all field changes on a bug:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `date_modified INTEGER NOT NULL DEFAULT 0`
- `field_name TEXT NOT NULL DEFAULT ''`
- `old_value TEXT NOT NULL DEFAULT ''`
- `new_value TEXT NOT NULL DEFAULT ''`
- `type INTEGER NOT NULL DEFAULT 0`

### 2.6 `mantis_bug_relationship_table`
Inter-bug relationships:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `source_bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `destination_bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `relationship_type INTEGER NOT NULL DEFAULT 0` (`0=related to`, `1=duplicate of`, `2=has duplicate`, `3=blocks`, `4=blocked by`, `5=parent of`, `6=child of`)

### 2.7 `mantis_bug_file_table`
File attachments:
- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE`
- `title TEXT NOT NULL DEFAULT ''`
- `description TEXT NOT NULL DEFAULT ''`
- `diskfile TEXT NOT NULL DEFAULT ''` (UUID-based filename on disk under `MANTIS_UPLOAD_DIR`)
- `filename TEXT NOT NULL DEFAULT ''` (original filename)
- `folder TEXT NOT NULL DEFAULT ''`
- `filesize INTEGER NOT NULL DEFAULT 0`
- `file_type TEXT NOT NULL DEFAULT ''` (MIME type)
- `content BLOB` (NULL when stored on disk; NOT NULL when stored inline in SQLite)
- `date_added INTEGER NOT NULL DEFAULT 0`
- `user_id INTEGER NOT NULL DEFAULT 0`

> `MANTIS_UPLOAD_DIR` env var (default `"uploads/"`) controls whether files are stored on disk or inline in SQLite BLOB. Inline mode is used when `MANTIS_UPLOAD_DIR` is empty.

---

## 3. MantisBT Integer Enum Constants (Sub-project 2)

### 3.1 Bug Status
`10=new`, `20=feedback`, `30=acknowledged`, `40=confirmed`, `50=assigned`, `80=resolved`, `90=closed`

### 3.2 Bug Priority
`10=none`, `20=low`, `30=normal`, `40=high`, `50=urgent`, `60=immediate`

### 3.3 Bug Severity
`10=feature`, `20=trivial`, `30=text`, `40=tweak`, `50=minor`, `60=major`, `70=crash`, `80=block`

### 3.4 Bug Reproducibility
`10=always`, `30=sometimes`, `50=random`, `70=have not tried`, `90=unable to reproduce`, `100=N/A`

### 3.5 Bug Resolution
`10=open`, `20=fixed`, `30=reopened`, `40=unable to reproduce`, `50=not fixable`, `60=duplicate`, `70=not a bug`, `80=suspended`, `90=wont fix`

### 3.6 Relationship Type
`0=related to`, `1=duplicate of`, `2=has duplicate`, `3=blocks`, `4=blocked by`, `5=parent of`, `6=child of`

---

## 4. Domain Logic & Access Control

### 4.1 Who Can Do What (Effective Access Level Checks per Action)
Based on MantisBT's default configuration (`$g_report_bug_threshold`, `$g_update_bug_threshold`, etc.):

| Action | Minimum Access Level |
|--------|---------------------|
| View public bugs in accessible project | `Viewer (10)` |
| View private bugs | `Developer (55)` |
| Report/create a new bug | `Reporter (25)` |
| Add a public bugnote | `Reporter (25)` |
| Add a private bugnote | `Developer (55)` |
| Update own bug (summary, description) | `Reporter (25)` if own bug |
| Update any bug fields | `Updater (40)` |
| Assign bug (`handler_id`) | `Developer (55)` |
| Change status to `acknowledged/confirmed` | `Updater (40)` |
| Change status to `resolved` | `Developer (55)` |
| Change status to `closed` | `Manager (70)` |
| Reopen a closed/resolved bug | `Developer (55)` |
| Delete a bug | `Manager (70)` |
| Delete a bugnote | `Manager (70)` |
| View private bugnotes | `Developer (55)` |
| Manage relationships | `Developer (55)` |
| Upload attachment | `Reporter (25)` |
| Delete attachment | `Manager (70)` |

### 4.2 Bug History Recording
Every field change on `mantis_bug_table` (status, priority, severity, handler_id, category_id, summary, version, etc.) is appended to `mantis_bug_history_table` with `old_value`, `new_value`, and `field_name`. Note additions are also recorded as history entries (`type=1`). File attachment additions/deletions are recorded as history entries (`type=2`).

### 4.3 Duplicate Relationships
When setting a bug as duplicate of another:
1. Set `duplicate_id` on `mantis_bug_table`.
2. Insert `relationship_type=1` (duplicate of) in `mantis_bug_relationship_table` from source → target.
3. Insert `relationship_type=2` (has duplicate) in reverse direction automatically.

### 4.4 Bug Visibility
A bug is visible to a user if:
1. User has `effective_access_level >= Viewer (10)` on the bug's project.
2. If `bug.view_state == 50` (private), user must have `effective_access_level >= Developer (55)`.
3. Private bugnotes are only visible to users with `effective_access_level >= Developer (55)`.

---

## 5. HTTP Routes & Module Structure

### 5.1 New/Modified Files
- `src/models.rs`: Add `BugStatus`, `BugPriority`, `BugSeverity`, `BugReproducibility`, `BugResolution`, `RelationshipType` enums + `Bug`, `BugFull` (with text fields), `Bugnote`, `BugHistory`, `BugRelationship`, `BugFile` structs.
- `src/db.rs`: Add all Sub-project 2 table migrations and data access methods.
- `src/handlers/bugs.rs` (new): All issue-tracking endpoints.
- `src/lib.rs`: Wire `bugs.rs` routes into `build_router`.

### 5.2 HTTP Routes (`src/handlers/bugs.rs`)

#### Viewing Issues
- `GET /view_all_bug_page` — Issue list for active project (paginated, with basic filter by status/priority/severity/assignee)
- `GET /view_bug_page/:id` — Full bug detail view (description, notes, history, relationships, attachments)

#### Creating & Editing
- `GET /bug_report_page` — Bug report form
- `POST /bug_report` — Submit new bug (creates `mantis_bug_table` + `mantis_bug_text_table` rows, records history)
- `GET /update_bug_page/:id` — Edit bug form
- `POST /update_bug/:id` — Update bug fields (records field changes in `mantis_bug_history_table`)
- `POST /bug_delete/:id` — Delete bug (requires `Manager (70)`)

#### Status Workflow Actions (POST-only quick-action forms on view_bug_page)
- `POST /bug_change_status/:id` — Change bug status (with optional resolution and note)
- `POST /bug_assign/:id` — Assign bug to a user

#### Bug Notes
- `POST /bugnote_add/:bug_id` — Add a bugnote
- `POST /bugnote_delete/:id` — Delete a bugnote (requires `Manager (70)` or own note for Reporter)

#### Relationships
- `POST /bug_relationship_add/:bug_id` — Add a relationship between bugs
- `POST /bug_relationship_delete/:id` — Delete a relationship

#### Attachments
- `POST /bug_file_add/:bug_id` — Upload a file attachment (multipart form)
- `GET /file/bug/:id/:filename` — Download a file attachment
- `POST /bug_file_delete/:id` — Delete a file attachment (requires `Manager (70)`)

---

## 6. Error Handling & Security

- All user-supplied HTML content (summary, description, notes) is HTML-escaped in templates.
- Attachment upload is limited to `MANTIS_MAX_FILE_SIZE` env var (default `5MB`). Rejected files return HTTP 413.
- Bug and bugnote `view_state == private` enforced at query level (not just template level).
- Access level checks happen in handler before any DB mutation. Insufficient access returns HTTP 403 page.
- Deleting a bug cascades via SQLite `ON DELETE CASCADE` to all related `mantis_bugnote_table`, `mantis_bug_history_table`, `mantis_bug_relationship_table`, and `mantis_bug_file_table` rows.

---

## 7. Verification & Testing Strategy

Integration tests (`tests/subproject2_http_test.rs`) and unit tests in `src/db.rs`:

1. **Schema & Seed:** Verify all 6 new tables are created; verify bug can be created and fetched back with all text fields.
2. **Bug CRUD & Access Control:** Reporter creates bug; Updater edits; Developer resolves; Manager closes; Reporter blocked from closing/deleting; anonymous redirected to login.
3. **Bug Status Lifecycle:** `new → feedback → acknowledged → confirmed → assigned → resolved → closed → reopened` flow with history recorded at each step.
4. **Bug History:** Verify every field change in `update_bug` produces correct entries in `mantis_bug_history_table`.
5. **Bugnotes:** Add public/private note; verify private note hidden from Viewer; delete note as Manager.
6. **Relationships & Duplicates:** Add related-to and duplicate-of relationships; verify reverse relationship auto-created for duplicate; verify relationship deletion.
7. **File Attachments:** Upload a text file inline (no MANTIS_UPLOAD_DIR); download it; delete it; verify deletion from history.
8. **Bug Visibility (Private Bugs):** Create private bug (`view_state=50`); verify Viewer sees nothing; Developer sees it.
