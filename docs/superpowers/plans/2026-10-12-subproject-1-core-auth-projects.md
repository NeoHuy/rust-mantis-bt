# Sub-project 1: Core Foundation, Auth & Projects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the single-binary Rust + SQLite foundation of MantisBT covering schema bootstrap, configuration store, user management, session authentication, effective project-level authorization, project hierarchy, categories, and versions.

**Architecture:** An `axum` HTTP server backed by an embedded `rusqlite` (`bundled`) database handle (`Arc<Mutex<Connection>>`) with server-rendered HTML views (`src/ui.rs`). Domain models, enum constants, and SQLite tables strictly mirror MantisBT (`mantis_*_table` and integer codes `10, 25, 40, 55, 70, 90`).

**Tech Stack:** Rust (2021 edition), `axum = "0.7"`, `tokio = "1"`, `rusqlite = { version = "0.32", features = ["bundled"] }`, `sha2 = "0.10"`, `rand = "0.8"`, `serde = { version = "1", features = ["derive"] }`, `tower = { version = "0.5", features = ["util"] }`.

**Spec:** `docs/superpowers/specs/2026-10-12-subproject-1-core-auth-projects-design.md`

## Global Constraints

- Database table names and columns must match `mantis_config_table`, `mantis_user_table`, `mantis_project_table`, `mantis_project_hierarchy_table`, `mantis_project_user_list_table`, `mantis_category_table`, and `mantis_project_version_table`.
- SQLite connection must enable `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;`.
- Enum values must match MantisBT integers:
  - `AccessLevel`: `Viewer = 10`, `Reporter = 25`, `Updater = 40`, `Developer = 55`, `Manager = 70`, `Administrator = 90`, `Nobody = 100`.
  - `ProjectStatus`: `Development = 10`, `Release = 30`, `Stable = 50`, `Obsolete = 70`.
  - `ViewState`: `Public = 10`, `Private = 50`.
- Default bootstrap on empty DB: `administrator` user (`password = "root"`, `access_level = 90`, `enabled = 1`) and global category `General` (`project_id = 0`, `name = "General"`).
- All HTML output must escape user-controlled strings (`html_escape`).

## Review Focus

- Cyclic sub-project hierarchy (`A -> B -> A` or self-parent `A -> A`) when linking sub-projects or resolving inherited categories must be rejected with an error instead of infinite recursion.
- Demoting, disabling, or deleting the last enabled `Administrator` (`access_level = 90`) must be rejected so the instance cannot lock itself out.
- Updating password or access level of a `protected = 1` user must be rejected.
- Disabled users (`enabled = 0`) must be denied login and existing cookie sessions (`MANTIS_STRING_COOKIE`) for disabled users must be rejected.
- Switching active project (`MANTIS_PROJECT_COOKIE`) to a private project (`view_state = 50`) where the user has `effective_access_level == 0` must fall back to `0` (`All Projects`) or reject access.

---

### Task 1: Crate Setup, Domain Models & SQLite Schema Bootstrap

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/models.rs`
- Create: `src/db.rs`
- Create: `src/main.rs`

**Interfaces:**
- Produces:
  - `models::AccessLevel` (`Viewer=10`, `Reporter=25`, `Updater=40`, `Developer=55`, `Manager=70`, `Administrator=90`, `Nobody=100`, `from_i32(i32) -> Option<Self>`, `as_str(&self) -> &'static str`)
  - `models::ProjectStatus` (`Development=10`, `Release=30`, `Stable=50`, `Obsolete=70`)
  - `models::ViewState` (`Public=10`, `Private=50`)
  - `models::{User, Project, Category, ProjectVersion, ProjectUser}`
  - `db::Db` (`#[derive(Clone)] pub struct Db(Arc<Mutex<Connection>>);`)
  - `db::Db::open(path: &str) -> rusqlite::Result<Self>`
  - `db::Db::open_in_memory() -> rusqlite::Result<Self>`
  - `db::Db::get_config(&self, config_id: &str, project_id: i64, user_id: i64) -> rusqlite::Result<Option<String>>`
  - `db::Db::set_config(&self, config_id: &str, project_id: i64, user_id: i64, value: &str) -> rusqlite::Result<()>`

- [ ] **Step 1: Create `Cargo.toml` and minimal module stubs with failing schema & seed tests in `src/db.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_creates_tables_and_default_seed() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().expect("default admin");
        assert_eq!(admin.access_level, 90);
        assert_eq!(admin.enabled, 1);

        let cats = db.list_categories_for_project(0).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].name, "General");
        assert_eq!(cats[0].project_id, 0);

        db.set_config("window_title", 0, 0, "Rust MantisBT").unwrap();
        assert_eq!(
            db.get_config("window_title", 0, 0).unwrap().as_deref(),
            Some("Rust MantisBT")
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_bootstrap_creates_tables_and_default_seed`
Expected: FAIL (missing implementation of `Db::open_in_memory`, schema migration, and seed).

- [ ] **Step 3: Implement `src/models.rs` and `src/db.rs` bootstrap schema + config store**

Create all 7 Sub-project 1 tables in `Db::migrate(&self)` with `PRAGMA foreign_keys = ON;`, seed `administrator` (`password = "root"`, hashed via `auth::hash_password`) and global `General` category if empty, and implement `get_config` / `set_config`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_bootstrap_creates_tables_and_default_seed`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat(db): initialize crate, MantisBT models, and SQLite schema bootstrap"
```

---

### Task 2: Authentication, Password Verification & User Management (`mantis_user_table`)

**Files:**
- Create: `src/auth.rs`
- Modify: `src/db.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::{User, AccessLevel}`
- Produces:
  - `auth::hash_password(plain: &str) -> String` (format `sha256$<salt>$<hex>`)
  - `auth::verify_password(plain: &str, stored: &str) -> bool` (supports `sha256$<salt>$<hex>` and legacy plain/MD5 fallback for migrated DBs)
  - `auth::generate_cookie_string() -> String` (64-char hex random token)
  - `db::Db::authenticate(&self, username: &str, password: &str) -> Result<Option<User>, String>`
  - `db::Db::get_user_by_cookie(&self, cookie: &str) -> rusqlite::Result<Option<User>>`
  - `db::Db::get_user_by_id(&self, id: i64) -> rusqlite::Result<Option<User>>`
  - `db::Db::list_users(&self, search: Option<&str>, enabled_only: bool) -> rusqlite::Result<Vec<User>>`
  - `db::Db::create_user(&self, username: &str, realname: &str, email: &str, password: &str, access_level: i32, enabled: bool, protected: bool) -> Result<i64, String>`
  - `db::Db::update_user(&self, id: i64, realname: &str, email: &str, new_password: Option<&str>, access_level: i32, enabled: bool, protected: bool) -> Result<(), String>`
  - `db::Db::delete_user(&self, id: i64) -> Result<(), String>`

- [ ] **Step 1: Write failing unit tests in `src/auth.rs` for login counters, disabled/protected accounts, and last-admin protection**

```rust
#[cfg(test)]
mod tests {
    use crate::db::Db;

    #[test]
    fn test_auth_login_counters_and_disabled_user() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.authenticate("administrator", "wrong").unwrap().is_none());
        let u = db.get_user_by_username("administrator").unwrap().unwrap();
        assert_eq!(u.failed_login_count, 1);

        let logged_in = db.authenticate("administrator", "root").unwrap().unwrap();
        assert_eq!(logged_in.failed_login_count, 0);
        assert_eq!(logged_in.login_count, 1);
        assert!(db.get_user_by_cookie(&logged_in.cookie_string).unwrap().is_some());
    }

    #[test]
    fn test_protected_and_last_admin_guards() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();
        // Cannot delete or demote the only administrator
        assert!(db.delete_user(admin.id).is_err());
        assert!(db.update_user(admin.id, "Admin", "a@b.c", None, 25, true, false).is_err());

        // Create a protected reporter
        let uid = db.create_user("prot_user", "Prot", "p@b.c", "secret", 25, true, true).unwrap();
        // Protected user cannot have password or access_level changed while protected=true
        assert!(db.update_user(uid, "Prot", "p@b.c", Some("newpass"), 55, true, true).is_err());
        assert!(db.delete_user(uid).is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_auth_`
Expected: FAIL

- [ ] **Step 3: Implement password hashing, authentication counters, and guarded user CRUD in `src/auth.rs` and `src/db.rs`**

Ensure `get_user_by_cookie` returns `None` when `enabled == 0`, `update_user` / `delete_user` check active administrator count (`SELECT COUNT(*) FROM mantis_user_table WHERE access_level >= 90 AND enabled = 1 AND id != ?1`), and protected accounts reject password/role mutation or deletion.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(auth): implement user CRUD, password hashing, and admin/protected guards"
```

---

### Task 3: Projects, Hierarchy, Categories, Versions & Effective Authorization

**Files:**
- Modify: `src/db.rs`
- Modify: `src/auth.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::{User, Project, Category, ProjectVersion, ProjectUser}`
- Produces:
  - `db::Db::create_project(&self, name: &str, status: i32, view_state: i32, description: &str, inherit_global: bool) -> Result<i64, String>`
  - `db::Db::update_project(&self, id: i64, name: &str, status: i32, enabled: bool, view_state: i32, description: &str, inherit_global: bool) -> Result<(), String>`
  - `db::Db::delete_project(&self, id: i64) -> Result<(), String>`
  - `db::Db::add_subproject(&self, parent_id: i64, child_id: i64, inherit_parent: bool) -> Result<(), String>`
  - `db::Db::remove_subproject(&self, parent_id: i64, child_id: i64) -> Result<(), String>`
  - `db::Db::set_project_user(&self, project_id: i64, user_id: i64, access_level: i32) -> Result<(), String>`
  - `db::Db::remove_project_user(&self, project_id: i64, user_id: i64) -> Result<(), String>`
  - `db::Db::effective_access_level(&self, user: &User, project_id: i64) -> rusqlite::Result<i32>`
  - `db::Db::list_visible_projects(&self, user: &User) -> rusqlite::Result<Vec<(Project, usize)>>` (returns project + hierarchy depth)
  - `db::Db::add_category(&self, project_id: i64, name: &str, user_id: i64) -> Result<i64, String>`
  - `db::Db::update_category(&self, id: i64, name: &str, user_id: i64) -> Result<(), String>`
  - `db::Db::delete_category(&self, id: i64) -> Result<(), String>`
  - `db::Db::list_effective_categories(&self, project_id: i64) -> rusqlite::Result<Vec<Category>>`
  - `db::Db::add_version(&self, project_id: i64, version: &str, description: &str, released: bool, obsolete: bool) -> Result<i64, String>`
  - `db::Db::update_version(&self, id: i64, version: &str, description: &str, released: bool, obsolete: bool) -> Result<(), String>`
  - `db::Db::delete_version(&self, id: i64) -> Result<(), String>`
  - `db::Db::list_versions(&self, project_id: i64) -> rusqlite::Result<Vec<ProjectVersion>>`

- [ ] **Step 1: Write failing tests in `src/db.rs` covering private project access, cycle rejection, and category inheritance**

```rust
#[test]
fn test_effective_access_and_private_projects() {
    let db = Db::open_in_memory().unwrap();
    let pub_id = db.create_project("PublicProj", 10, 10, "desc", true).unwrap();
    let priv_id = db.create_project("PrivateProj", 10, 50, "desc", true).unwrap();

    let rep_id = db.create_user("alice", "Alice", "a@x.com", "pw", 25, true, false).unwrap();
    let mgr_id = db.create_user("bob", "Bob", "b@x.com", "pw", 70, true, false).unwrap();
    let alice = db.get_user_by_id(rep_id).unwrap().unwrap();
    let bob = db.get_user_by_id(mgr_id).unwrap().unwrap();

    assert_eq!(db.effective_access_level(&alice, pub_id).unwrap(), 25);
    assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 0);
    assert_eq!(db.effective_access_level(&bob, priv_id).unwrap(), 70);

    // Grant Alice Developer (55) on PrivateProj
    db.set_project_user(priv_id, alice.id, 55).unwrap();
    assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 55);
}

#[test]
fn test_hierarchy_cycle_prevention_and_category_inheritance() {
    let db = Db::open_in_memory().unwrap();
    let p1 = db.create_project("Parent", 10, 10, "", true).unwrap();
    let p2 = db.create_project("Child", 10, 10, "", false).unwrap();

    db.add_subproject(p1, p2, true).unwrap();
    // Self-parent and reverse cycle must fail
    assert!(db.add_subproject(p1, p1, true).is_err());
    assert!(db.add_subproject(p2, p1, true).is_err());

    db.add_category(p1, "Backend", 0).unwrap();
    db.add_category(p2, "UI", 0).unwrap();

    // Child inherits from Parent (which inherits Global 'General')
    let child_cats = db.list_effective_categories(p2).unwrap();
    let names: Vec<_> = child_cats.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"General"));
    assert!(names.contains(&"Backend"));
    assert!(names.contains(&"UI"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_effective_access_and_private_projects test_hierarchy_cycle_prevention_and_category_inheritance`
Expected: FAIL

- [ ] **Step 3: Implement project CRUD, cycle-checked hierarchy, effective access level, category inheritance, and versions in `src/db.rs`**

Before inserting `(child_id, parent_id)` into `mantis_project_hierarchy_table`, walk ancestors of `parent_id` with a `HashSet<i64>` visited guard; if `child_id == parent_id` or `child_id` is an ancestor of `parent_id`, return `Err("Cyclic project hierarchy detected".into())`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(projects): add project hierarchy, category inheritance, versions, and access resolution"
```

---

### Task 4: Server-Side UI Layout & HTTP Handlers (Auth, Users, Projects)

**Files:**
- Create: `src/ui.rs`
- Create: `src/handlers/mod.rs`
- Create: `src/handlers/auth.rs`
- Create: `src/handlers/users.rs`
- Create: `src/handlers/projects.rs`
- Modify: `src/auth.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `db::Db`, `models::*`, `auth::*`
- Produces:
  - `ui::html_escape(s: &str) -> String`
  - `ui::render_layout(title: &str, ctx: &AuthContext, flash: Option<&str>, content: &str) -> axum::response::Html<String>`
  - `auth::AuthContext` (`pub user: User, pub active_project_id: i64, pub visible_projects: Vec<(Project, usize)>`)
  - `build_router(db: Db) -> axum::Router`

- [ ] **Step 1: Write failing HTTP integration test in `tests/subproject1_http_test.rs`**

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

#[tokio::test]
async fn test_login_project_creation_and_private_project_cookie_guard() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Unauthenticated GET / redirects to /login
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 2. POST /login with administrator/root sets MANTIS_STRING_COOKIE
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=root"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res.headers().get("set-cookie").unwrap().to_str().unwrap().to_string();
    assert!(cookie_hdr.contains("MANTIS_STRING_COOKIE="));

    // 3. Create a private project via POST /manage_proj_create
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_proj_create")
                .header("cookie", &cookie_hdr)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=SecretCore&status=10&view_state=50&description=Top+secret&inherit_global=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 4. Create a Reporter user and verify they cannot switch active project to SecretCore (id=1)
    let uid = db.create_user("rep1", "Reporter One", "r@x.com", "pass123", 25, true, false).unwrap();
    let rep = db.get_user_by_id(uid).unwrap().unwrap();
    let rep_cookie = format!("MANTIS_STRING_COOKIE={}; MANTIS_PROJECT_COOKIE=1", rep.cookie_string);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // SecretCore must not appear in Reporter's HTML response
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(!html.contains("SecretCore"));
}
```

- [ ] **Step 2: Run HTTP integration test to verify it fails**

Run: `cargo test --test subproject1_http_test`
Expected: FAIL

- [ ] **Step 3: Implement `src/ui.rs`, `src/handlers/{auth,users,projects}.rs`, and `build_router` in `src/lib.rs`**

Implement all routes specified in Section 5.1 of the Spec:
- `GET /` (redirects to `/manage_proj_page` or `/account_page` in Sub-project 1 until `/my_view_page` arrives in Sub-project 3)
- Auth routes (`/login`, `/logout`, `/set_project`, `/account_page`, `/account_update`)
- User management routes (`/manage_user_page`, `/manage_user_create_page`, `/manage_user_create`, `/manage_user_edit_page/:id`, `/manage_user_update/:id`, `/manage_user_delete/:id`) requiring `Administrator (90)`
- Project management routes (`/manage_proj_page`, `/manage_proj_create_page`, `/manage_proj_create`, `/manage_proj_edit_page/:id`, `/manage_proj_update/:id`, `/manage_proj_delete/:id`, subprojects, categories, versions, project user list) requiring `Manager (70)` or `Administrator (90)`.
- Ensure `AuthContext` validates `MANTIS_PROJECT_COOKIE` using `effective_access_level(&user, project_id) > 0`, falling back to `0` (`All Projects`) if unauthorized.

- [ ] **Step 4: Run all unit and integration tests to verify they pass**

Run: `cargo test`
Expected: PASS (all unit and integration tests green, zero warnings)

- [ ] **Step 5: Commit**

```bash
git add src/ tests/
git commit -m "feat(web): implement MantisBT UI shell, auth sessions, user & project management handlers"
```
