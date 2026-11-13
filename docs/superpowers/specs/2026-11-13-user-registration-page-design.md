# User Registration Page Design Spec

## Goal
Add a self-service user registration page (`/signup_page` and `/signup`) to MantisBT so new users can create an account without needing an administrator to pre-create it, adhering to MantisBT conventions and styling.

## Context & Motivation
Currently, MantisBT only permits administrators (`access_level >= 90`) to create user accounts via `/manage_user_create_page`. In standard MantisBT (PHP), a public signup page exists (`signup_page.php`) allowing visitors to register their own account with a default access level (`REPORTER = 25`), controlled by an `allow_signup` configuration.

## Requirements

### 1. Functional Requirements
1. **Public Registration Form (`GET /signup_page`)**:
   - Accessible without authentication.
   - Form fields:
     - `username` (Text, required, trimmed, alphanumeric with allowed separators `_`, `.`, `-`).
     - `realname` (Text, optional).
     - `email` (Email, required, valid email format).
     - `password` (Password, required, minimum 6 characters).
     - `password_confirm` (Password, required, must match `password`).
   - Links:
     - Link back to "Sign In" (`/login`).
2. **Registration Form Processing (`POST /signup`)**:
   - Validation checks:
     - Configuration check: `allow_signup` is enabled (default `true` / `1`). If disabled, return HTTP 403 Forbidden with error message "Registration is currently disabled."
     - Required fields present and non-empty (`username`, `email`, `password`, `password_confirm`).
     - Valid username: Length between 1 and 32 characters, no spaces, valid format.
     - Uniqueness: Case-insensitive check that username does not already exist.
     - Valid email: Must contain `@` and domain dot, length > 3.
     - Uniqueness: Email does not already exist in database (prevent multi-account conflicts).
     - Password policy: Minimum 6 characters.
     - Password match: `password == password_confirm`.
   - On success:
     - Insert into `mantis_user_table` with `access_level = 25` (Reporter), `enabled = 1`, `protected = 0`.
     - Log in the user immediately by issuing `MANTIS_STRING_COOKIE` and redirecting to `/my_view_page`, OR redirect to `/login` with success banner. (Auto-login provides seamless UX).
   - On failure:
     - Re-render the signup form with an error banner (`flash_error`), preserving non-sensitive fields (`username`, `realname`, `email`).
3. **Login Page Integration (`GET /login`)**:
   - Add a "Signup for a new account" button/link under the login form when `allow_signup` is enabled.
   - Support displaying success flash message if redirected from registration (e.g. `?msg=...`).
4. **Configuration**:
   - `allow_signup`: integer/boolean config stored in `mantis_config_table` (`config_id = 'allow_signup'`). Default to `1` (enabled) if unset.
   - `default_new_account_access_level`: integer config in `mantis_config_table`, default to `25` (`Reporter`).

### 2. Architecture & File Structure
- `src/db.rs`:
  - `Db::is_signup_allowed(&self) -> bool`: Reads config `'allow_signup'`, defaulting to `true`.
  - `Db::get_user_by_email(&self, email: &str) -> rusqlite::Result<Option<User>>`: Helper for email lookup.
  - `Db::signup_user(&self, username: &str, realname: &str, email: &str, password: &str) -> Result<User, String>`: Validates input, uniqueness, hashes password, inserts user with Reporter access level, and returns the new user.
- `src/handlers/auth.rs`:
  - `pub struct SignupForm`: Serde model for signup request.
  - `pub async fn signup_page(headers: HeaderMap, State(db): State<Db>) -> Response`: Renders signup view.
  - `pub async fn signup_submit(State(db): State<Db>, Form(form): Form<SignupForm>) -> Response`: Handles submission.
  - Update `login_page` to display "Signup for a new account" link and query messages.
- `src/handlers/mod.rs`:
  - Register `/signup_page` and `/signup`.
- `src/ui.rs`:
  - Add optional flash success styling or ensure flash messages render cleanly in unauthenticated layout.
- `tests/subproject1_http_test.rs`:
  - Integration tests for registration flow, validations, duplicate checks, and disable flag.
