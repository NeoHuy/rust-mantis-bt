# User Registration Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide a public self-service user registration page (`/signup_page` and `/signup`) for MantisBT allowing new users to create accounts with default Reporter privileges, complete with validation, duplicate detection, and login page integration.

**Architecture:** Extend Axum auth handlers with `signup_page` and `signup_submit`. Add DB methods to check config `allow_signup`, check email/username uniqueness, and register users. Render using the standard MantisBT unauthenticated layout with input error flash messages.

**Tech Stack:** Rust, Axum, Rusqlite, HTML/CSS server-rendered templates, Tower.

**Spec:** `docs/superpowers/specs/2026-11-13-user-registration-page-design.md`

## Global Constraints
- Passwords must be hashed using `crate::auth::hash_password`.
- Passwords must have a minimum length of 6 characters.
- Default access level for new self-registered users must be `AccessLevel::Reporter` (25).
- Newly registered accounts must be `enabled = 1` and `protected = 0`.
- All user input must be sanitized/escaped with `crate::ui::html_escape` before HTML rendering.
- Check case-insensitive uniqueness for usernames.
- Verify email format and check for duplicate email address.
- If `allow_signup` is set to `0` / `false` in `mantis_config_table`, signup page must reject registration.

## Review Focus
- User submits mismatched `password` and `password_confirm`: must return 400 with a clear error message and keep other form inputs.
- Username or email already exists: must reject with descriptive message without crashing.
- Whitespace in username: must trim and reject if empty or invalid.
- Signup disabled: accessing `/signup_page` or posting `/signup` when `allow_signup=0` returns 403 Forbidden with notification.
- Successful signup: user is registered, cookie header `MANTIS_STRING_COOKIE` is set, and user is redirected to `/my_view_page` (or `/login?msg=Registered`).

---

### Task 1: DB Layer - Config & User Registration Helpers

**Files:**
- Modify: `src/db.rs`
- Test: `src/db.rs` (unit tests)

**Interfaces:**
- `Db::is_signup_allowed(&self) -> bool`
- `Db::get_user_by_email(&self, email: &str) -> rusqlite::Result<Option<User>>`
- `Db::signup_user(&self, username: &str, realname: &str, email: &str, password: &str) -> Result<User, String>`

- [x] **Step 1: Write unit tests in `src/db.rs` for signup helper functions**
  - Add `test_signup_user_success_and_validations`:
    - Test successful user signup returns `User` with `access_level == 25`, `enabled == 1`, `protected == 0`.
    - Test duplicate username fails with error.
    - Test duplicate email fails with error.
    - Test empty username or password < 6 characters fails with error.
  - Add `test_is_signup_allowed_default_and_override`:
    - Tests default returns `true`.
    - Tests setting `allow_signup = "0"` via `set_config` returns `false`.

- [x] **Step 2: Run `cargo test db::tests::test_signup` to verify failures**
  - Ensure compiler fails or tests fail because functions are not yet implemented.

- [x] **Step 3: Implement `is_signup_allowed`, `get_user_by_email`, and `signup_user` in `src/db.rs`**
  - Read `allow_signup` from `mantis_config_table` (`0` / `"off"` / `"false"` returns false, else true).
  - Implement case-insensitive username lookup and email uniqueness check.
  - Insert new user record and return `User`.

- [x] **Step 4: Re-run `cargo test db::tests::test_signup` and verify passes**

---

### Task 2: UI & Handlers - Signup View, Submission, and Login Integration

**Files:**
- Modify: `src/ui.rs`
- Modify: `src/handlers/auth.rs`
- Modify: `src/handlers/mod.rs`
- Test: `tests/subproject1_http_test.rs`

**Interfaces:**
- `GET /signup_page` -> `handlers::auth::signup_page`
- `POST /signup` -> `handlers::auth::signup_submit`
- Update `handlers::auth::login_page` to render signup link and optional message.

- [x] **Step 1: Write HTTP integration tests in `tests/subproject1_http_test.rs`**
  - `test_user_signup_flow`:
    - `GET /signup_page` returns 200 OK with username, realname, email, password, password_confirm fields.
    - `POST /signup` with invalid confirmation returns 400 Bad Request with error message.
    - `POST /signup` with valid data creates user, returns redirect to `/my_view_page` with `Set-Cookie: MANTIS_STRING_COOKIE=...`.
    - Verify new user can authenticate and access `/account_page`.
    - `GET /login` displays link to `/signup_page`.
    - Setting config `allow_signup = 0` causes `GET /signup_page` and `POST /signup` to be forbidden / disabled.

- [x] **Step 2: Run `cargo test --test subproject1_http_test` to verify failures**

- [x] **Step 3: Implement `signup_page` and `signup_submit` in `src/handlers/auth.rs`**
  - Check `db.is_signup_allowed()`.
  - Validate password confirmation and fields.
  - Call `db.signup_user(...)`.
  - Issue session cookie `MANTIS_STRING_COOKIE` and redirect to `/my_view_page`.
  - Add link `<a href="/signup_page">Signup for a new account</a>` in `login_page`.

- [x] **Step 4: Register routes in `src/handlers/mod.rs`**
  - Add `.route("/signup_page", get(auth::signup_page))`
  - Add `.route("/signup", post(auth::signup_submit))`

- [x] **Step 5: Run tests and ensure all tests pass**
  - Run `cargo test` to verify full regression test suite passes.
