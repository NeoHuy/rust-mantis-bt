use crate::models::{
    ApiToken, Bug, BugFile, BugFilter, BugFull, BugHistory, BugRelationship, BugTag, BugUpdate,
    Bugnote, Category, CustomField, CustomFieldValue, EmailMessage, NewsItem, PluginItem, Project,
    ProjectVersion, SavedFilter, Tag, TimeTrackingEntry, User,
};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().expect("db lock poisoned")
    }

    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        let db = Self(Arc::new(Mutex::new(conn)));
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        let db = Self(Arc::new(Mutex::new(conn)));
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS mantis_config_table (
                config_id TEXT NOT NULL,
                project_id INTEGER NOT NULL DEFAULT 0,
                user_id INTEGER NOT NULL DEFAULT 0,
                access_reqd INTEGER NOT NULL DEFAULT 0,
                type INTEGER NOT NULL DEFAULT 90,
                value TEXT NOT NULL,
                PRIMARY KEY (config_id, project_id, user_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_user_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                realname TEXT NOT NULL DEFAULT '',
                email TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                protected INTEGER NOT NULL DEFAULT 0,
                access_level INTEGER NOT NULL DEFAULT 25,
                login_count INTEGER NOT NULL DEFAULT 0,
                lost_password_request_count INTEGER NOT NULL DEFAULT 0,
                failed_login_count INTEGER NOT NULL DEFAULT 0,
                cookie_string TEXT NOT NULL UNIQUE,
                last_visit INTEGER NOT NULL DEFAULT 0,
                date_created INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_project_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                status INTEGER NOT NULL DEFAULT 10,
                enabled INTEGER NOT NULL DEFAULT 1,
                view_state INTEGER NOT NULL DEFAULT 10,
                access_min INTEGER NOT NULL DEFAULT 10,
                description TEXT NOT NULL DEFAULT '',
                category_id INTEGER NOT NULL DEFAULT 1,
                inherit_global INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS mantis_project_hierarchy_table (
                child_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                parent_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                inherit_parent INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (child_id, parent_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_project_user_list_table (
                project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                access_level INTEGER NOT NULL DEFAULT 25,
                PRIMARY KEY (project_id, user_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_category_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL DEFAULT 0,
                user_id INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL,
                status INTEGER NOT NULL DEFAULT 0,
                UNIQUE (project_id, name)
            );

            CREATE TABLE IF NOT EXISTS mantis_project_version_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                version TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                released INTEGER NOT NULL DEFAULT 1,
                obsolete INTEGER NOT NULL DEFAULT 0,
                date_order INTEGER NOT NULL DEFAULT 0,
                UNIQUE (project_id, version)
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                reporter_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                handler_id INTEGER NOT NULL DEFAULT 0,
                duplicate_id INTEGER NOT NULL DEFAULT 0,
                priority INTEGER NOT NULL DEFAULT 30,
                severity INTEGER NOT NULL DEFAULT 50,
                reproducibility INTEGER NOT NULL DEFAULT 70,
                status INTEGER NOT NULL DEFAULT 10,
                resolution INTEGER NOT NULL DEFAULT 10,
                category_id INTEGER NOT NULL DEFAULT 0,
                date_submitted INTEGER NOT NULL DEFAULT 0,
                last_updated INTEGER NOT NULL DEFAULT 0,
                os TEXT NOT NULL DEFAULT '',
                os_build TEXT NOT NULL DEFAULT '',
                platform TEXT NOT NULL DEFAULT '',
                version TEXT NOT NULL DEFAULT '',
                fixed_in_version TEXT NOT NULL DEFAULT '',
                target_version TEXT NOT NULL DEFAULT '',
                build TEXT NOT NULL DEFAULT '',
                summary TEXT NOT NULL DEFAULT '',
                view_state INTEGER NOT NULL DEFAULT 10,
                sticky INTEGER NOT NULL DEFAULT 0,
                due_date INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_text_table (
                id INTEGER PRIMARY KEY REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                description TEXT NOT NULL DEFAULT '',
                steps_to_reproduce TEXT NOT NULL DEFAULT '',
                additional_information TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS mantis_bugnote_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                reporter_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                bugnote_text_id INTEGER NOT NULL,
                view_state INTEGER NOT NULL DEFAULT 10,
                date_submitted INTEGER NOT NULL DEFAULT 0,
                last_modified INTEGER NOT NULL DEFAULT 0,
                note_type INTEGER NOT NULL DEFAULT 0,
                note_attr TEXT NOT NULL DEFAULT '',
                time_tracking INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_bugnote_text_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_history_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                date_modified INTEGER NOT NULL DEFAULT 0,
                field_name TEXT NOT NULL DEFAULT '',
                old_value TEXT NOT NULL DEFAULT '',
                new_value TEXT NOT NULL DEFAULT '',
                type INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_relationship_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                destination_bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                relationship_type INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_file_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                title TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                diskfile TEXT NOT NULL DEFAULT '',
                filename TEXT NOT NULL DEFAULT '',
                folder TEXT NOT NULL DEFAULT '',
                filesize INTEGER NOT NULL DEFAULT 0,
                file_type TEXT NOT NULL DEFAULT '',
                content BLOB,
                date_added INTEGER NOT NULL DEFAULT 0,
                user_id INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_tag_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT '',
                date_created INTEGER NOT NULL DEFAULT 0,
                date_updated INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_tag_table (
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES mantis_tag_table(id) ON DELETE CASCADE,
                user_id INTEGER NOT NULL DEFAULT 0,
                date_attached INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (bug_id, tag_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_custom_field_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                type INTEGER NOT NULL DEFAULT 0,
                possible_values TEXT NOT NULL DEFAULT '',
                default_value TEXT NOT NULL DEFAULT '',
                valid_regexp TEXT NOT NULL DEFAULT '',
                access_level_r INTEGER NOT NULL DEFAULT 10,
                access_level_rw INTEGER NOT NULL DEFAULT 25,
                length_min INTEGER NOT NULL DEFAULT 0,
                length_max INTEGER NOT NULL DEFAULT 0,
                require_report INTEGER NOT NULL DEFAULT 0,
                require_update INTEGER NOT NULL DEFAULT 0,
                display_report INTEGER NOT NULL DEFAULT 1,
                display_update INTEGER NOT NULL DEFAULT 1,
                require_resolved INTEGER NOT NULL DEFAULT 0,
                display_resolved INTEGER NOT NULL DEFAULT 0,
                display_closed INTEGER NOT NULL DEFAULT 0,
                require_closed INTEGER NOT NULL DEFAULT 0,
                filter_by INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS mantis_custom_field_project_table (
                field_id INTEGER NOT NULL REFERENCES mantis_custom_field_table(id) ON DELETE CASCADE,
                project_id INTEGER NOT NULL REFERENCES mantis_project_table(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (field_id, project_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_custom_field_string_table (
                field_id INTEGER NOT NULL REFERENCES mantis_custom_field_table(id) ON DELETE CASCADE,
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                value TEXT NOT NULL DEFAULT '',
                text TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (field_id, bug_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_filters_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                project_id INTEGER NOT NULL DEFAULT 0,
                is_public INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL DEFAULT '',
                filter_string TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_monitor_table (
                user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                bug_id INTEGER NOT NULL REFERENCES mantis_bug_table(id) ON DELETE CASCADE,
                PRIMARY KEY (user_id, bug_id)
            );

            CREATE TABLE IF NOT EXISTS mantis_email_table (
                email_id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL DEFAULT '',
                subject TEXT NOT NULL DEFAULT '',
                metadata TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '',
                submitted INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_news_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL DEFAULT 0,
                poster_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                date_posted INTEGER NOT NULL DEFAULT 0,
                last_modified INTEGER NOT NULL DEFAULT 0,
                view_state INTEGER NOT NULL DEFAULT 10,
                announcement INTEGER NOT NULL DEFAULT 0,
                headline TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS mantis_api_token_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES mantis_user_table(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                hash TEXT NOT NULL UNIQUE,
                date_created INTEGER NOT NULL DEFAULT 0,
                date_used INTEGER NOT NULL DEFAULT 0,
                UNIQUE (user_id, name)
            );

            CREATE TABLE IF NOT EXISTS mantis_plugin_table (
                basename TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 1,
                protected INTEGER NOT NULL DEFAULT 0,
                priority INTEGER NOT NULL DEFAULT 3
            );",
        )?;

        // Default administrator seed if user table is empty
        let user_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_user_table", [], |row| {
                row.get(0)
            })?;
        if user_count == 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let cookie = crate::auth::generate_cookie_string();
            let hashed_pw = crate::auth::hash_password("root");
            conn.execute(
                "INSERT INTO mantis_user_table (
                    username, realname, email, password, enabled, protected,
                    access_level, login_count, lost_password_request_count, failed_login_count,
                    cookie_string, last_visit, date_created
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    "administrator",
                    "Administrator",
                    "root@localhost",
                    hashed_pw,
                    1,  // enabled
                    0,  // protected
                    90, // access_level (Administrator)
                    0,  // login_count
                    0,  // lost_password_request_count
                    0,  // failed_login_count
                    cookie,
                    0,   // last_visit
                    now, // date_created
                ],
            )?;
        }

        // Default global 'General' category seed
        let cat_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mantis_category_table WHERE project_id = 0 AND name = 'General'",
            [],
            |row| row.get(0),
        )?;
        if cat_count == 0 {
            conn.execute(
                "INSERT INTO mantis_category_table (project_id, user_id, name, status) VALUES (?1, ?2, ?3, ?4)",
                params![0, 0, "General", 0],
            )?;
        }

        // Default core plugins seed
        conn.execute(
            "INSERT OR IGNORE INTO mantis_plugin_table (basename, enabled, protected, priority)
             VALUES ('MantisCoreFormatting', 1, 1, 3)",
            [],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO mantis_plugin_table (basename, enabled, protected, priority)
             VALUES ('EventAuditLog', 1, 0, 3)",
            [],
        )?;

        Ok(())
    }

    pub fn get_user_by_username(&self, username: &str) -> rusqlite::Result<Option<User>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, username, realname, email, password, enabled, protected, access_level,
                    login_count, lost_password_request_count, failed_login_count, cookie_string,
                    last_visit, date_created
             FROM mantis_user_table
             WHERE username = ?1",
        )?;
        let mut rows = stmt.query(params![username])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_user_by_id(&self, id: i64) -> rusqlite::Result<Option<User>> {
        let conn = self.conn();
        Self::query_user_by_id(&conn, id)
    }

    fn query_user_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<User>> {
        let mut stmt = conn.prepare(
            "SELECT id, username, realname, email, password, enabled, protected, access_level,
                    login_count, lost_password_request_count, failed_login_count, cookie_string,
                    last_visit, date_created
             FROM mantis_user_table
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_user_by_cookie(&self, cookie: &str) -> rusqlite::Result<Option<User>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, username, realname, email, password, enabled, protected, access_level,
                    login_count, lost_password_request_count, failed_login_count, cookie_string,
                    last_visit, date_created
             FROM mantis_user_table
             WHERE cookie_string = ?1 AND enabled = 1",
        )?;
        let mut rows = stmt.query(params![cookie])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Result<Option<User>, String> {
        let user = match self.get_user_by_username(username).map_err(|e| e.to_string())? {
            Some(u) => u,
            None => return Ok(None),
        };

        if user.enabled == 0 {
            return Ok(None);
        }

        if !crate::auth::verify_password(password, &user.password) {
            let conn = self.conn();
            conn.execute(
                "UPDATE mantis_user_table SET failed_login_count = failed_login_count + 1 WHERE id = ?1",
                params![user.id],
            ).map_err(|e| e.to_string())?;
            return Ok(None);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        {
            let conn = self.conn();
            conn.execute(
                "UPDATE mantis_user_table
                 SET login_count = login_count + 1, failed_login_count = 0, last_visit = ?1
                 WHERE id = ?2",
                params![now, user.id],
            ).map_err(|e| e.to_string())?;
        }

        let mut updated = user;
        updated.login_count += 1;
        updated.failed_login_count = 0;
        updated.last_visit = now;
        Ok(Some(updated))
    }

    pub fn list_users(&self, search: Option<&str>, enabled_only: bool) -> rusqlite::Result<Vec<User>> {
        let conn = self.conn();
        let mut sql = "SELECT id, username, realname, email, password, enabled, protected, access_level,
                              login_count, lost_password_request_count, failed_login_count, cookie_string,
                              last_visit, date_created
                       FROM mantis_user_table".to_string();

        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if enabled_only {
            conditions.push("enabled = 1".to_string());
        }

        if let Some(s) = search {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                let pattern = format!("%{}%", trimmed);
                conditions.push("(username LIKE ? OR realname LIKE ? OR email LIKE ?)".to_string());
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern));
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY username ASC");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_user(
        &self,
        username: &str,
        realname: &str,
        email: &str,
        password: &str,
        access_level: i32,
        enabled: bool,
        protected: bool,
    ) -> Result<i64, String> {
        let trimmed_username = username.trim();
        if trimmed_username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }

        let cookie = crate::auth::generate_cookie_string();
        let hashed_pw = crate::auth::hash_password(password);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let enabled_val = if enabled { 1 } else { 0 };
        let protected_val = if protected { 1 } else { 0 };

        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_user_table (
                username, realname, email, password, enabled, protected,
                access_level, login_count, lost_password_request_count, failed_login_count,
                cookie_string, last_visit, date_created
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0, ?8, 0, ?9)",
            params![
                trimmed_username,
                realname,
                email,
                hashed_pw,
                enabled_val,
                protected_val,
                access_level,
                cookie,
                now,
            ],
        ).map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_user(
        &self,
        id: i64,
        realname: &str,
        email: &str,
        new_password: Option<&str>,
        access_level: i32,
        enabled: bool,
        protected: bool,
    ) -> Result<(), String> {
        let conn = self.conn();
        let existing = match Self::query_user_by_id(&conn, id).map_err(|e| e.to_string())? {
            Some(u) => u,
            None => return Err(format!("User {} not found", id)),
        };

        // Guard: Protected user cannot have password or access_level changed
        if existing.protected == 1 && (new_password.is_some() || access_level != existing.access_level) {
            return Err("Cannot modify password or access level of a protected user".to_string());
        }

        // Guard: Last enabled administrator cannot be demoted or disabled
        let is_currently_enabled_admin = existing.access_level >= 90 && existing.enabled == 1;
        let will_demote_or_disable = access_level < 90 || !enabled;
        if is_currently_enabled_admin && will_demote_or_disable {
            let other_admins: i64 = conn.query_row(
                "SELECT COUNT(*) FROM mantis_user_table WHERE access_level >= 90 AND enabled = 1 AND id != ?1",
                params![id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;

            if other_admins == 0 {
                return Err("Cannot demote or disable the last enabled administrator".to_string());
            }
        }

        let enabled_val = if enabled { 1 } else { 0 };
        let protected_val = if protected { 1 } else { 0 };

        if let Some(pw) = new_password {
            let hashed_pw = crate::auth::hash_password(pw);
            conn.execute(
                "UPDATE mantis_user_table
                 SET realname = ?1, email = ?2, password = ?3, access_level = ?4, enabled = ?5, protected = ?6
                 WHERE id = ?7",
                params![realname, email, hashed_pw, access_level, enabled_val, protected_val, id],
            ).map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE mantis_user_table
                 SET realname = ?1, email = ?2, access_level = ?3, enabled = ?4, protected = ?5
                 WHERE id = ?6",
                params![realname, email, access_level, enabled_val, protected_val, id],
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn delete_user(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let existing = match Self::query_user_by_id(&conn, id).map_err(|e| e.to_string())? {
            Some(u) => u,
            None => return Err(format!("User {} not found", id)),
        };

        // Guard: Cannot delete a protected user
        if existing.protected == 1 {
            return Err("Cannot delete a protected user".to_string());
        }

        // Guard: Last enabled administrator cannot be deleted
        if existing.access_level >= 90 && existing.enabled == 1 {
            let other_admins: i64 = conn.query_row(
                "SELECT COUNT(*) FROM mantis_user_table WHERE access_level >= 90 AND enabled = 1 AND id != ?1",
                params![id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;

            if other_admins == 0 {
                return Err("Cannot delete the last enabled administrator".to_string());
            }
        }

        conn.execute("DELETE FROM mantis_user_table WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn list_categories_for_project(&self, project_id: i64) -> rusqlite::Result<Vec<Category>> {
        self.list_effective_categories(project_id)
    }

    pub fn get_config(
        &self,
        config_id: &str,
        project_id: i64,
        user_id: i64,
    ) -> rusqlite::Result<Option<String>> {
        let conn = self.conn();
        let query = |p_id: i64, u_id: i64| -> rusqlite::Result<Option<String>> {
            match conn.query_row(
                "SELECT value FROM mantis_config_table WHERE config_id = ?1 AND project_id = ?2 AND user_id = ?3",
                params![config_id, p_id, u_id],
                |row| row.get(0),
            ) {
                Ok(val) => Ok(Some(val)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        };

        let candidates = [
            (project_id, user_id),
            (0, user_id),
            (project_id, 0),
            (0, 0),
        ];

        let mut seen = HashSet::new();
        for (p, u) in candidates {
            if seen.insert((p, u)) {
                if let Some(val) = query(p, u)? {
                    return Ok(Some(val));
                }
            }
        }

        Ok(None)
    }

    pub fn set_config(
        &self,
        config_id: &str,
        project_id: i64,
        user_id: i64,
        value: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_config_table (config_id, project_id, user_id, value)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(config_id, project_id, user_id) DO UPDATE SET value = excluded.value",
            params![config_id, project_id, user_id, value],
        )?;
        Ok(())
    }

    pub fn create_project(
        &self,
        name: &str,
        status: i32,
        view_state: i32,
        description: &str,
        inherit_global: bool,
    ) -> Result<i64, String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Project name cannot be empty".to_string());
        }

        let inherit_global_val = if inherit_global { 1 } else { 0 };
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_project_table (
                name, status, enabled, view_state, access_min, description, category_id, inherit_global
            ) VALUES (?1, ?2, 1, ?3, 10, ?4, 1, ?5)",
            params![
                trimmed_name,
                status,
                view_state,
                description,
                inherit_global_val,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_project_by_id(&self, id: i64) -> rusqlite::Result<Option<Project>> {
        let conn = self.conn();
        Self::query_project_by_id(&conn, id)
    }

    fn query_project_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Project>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, status, enabled, view_state, access_min, description, category_id, inherit_global
             FROM mantis_project_table
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                enabled: row.get(3)?,
                view_state: row.get(4)?,
                access_min: row.get(5)?,
                description: row.get(6)?,
                category_id: row.get(7)?,
                inherit_global: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_project(
        &self,
        id: i64,
        name: &str,
        status: i32,
        enabled: bool,
        view_state: i32,
        description: &str,
        inherit_global: bool,
    ) -> Result<(), String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Project name cannot be empty".to_string());
        }

        let conn = self.conn();
        let existing = match Self::query_project_by_id(&conn, id).map_err(|e| e.to_string())? {
            Some(p) => p,
            None => return Err(format!("Project {} not found", id)),
        };
        let _ = existing;

        let enabled_val = if enabled { 1 } else { 0 };
        let inherit_global_val = if inherit_global { 1 } else { 0 };

        conn.execute(
            "UPDATE mantis_project_table
             SET name = ?1, status = ?2, enabled = ?3, view_state = ?4, description = ?5, inherit_global = ?6
             WHERE id = ?7",
            params![
                trimmed_name,
                status,
                enabled_val,
                view_state,
                description,
                inherit_global_val,
                id,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_project(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let existing = match Self::query_project_by_id(&conn, id).map_err(|e| e.to_string())? {
            Some(p) => p,
            None => return Err(format!("Project {} not found", id)),
        };
        let _ = existing;

        conn.execute("DELETE FROM mantis_category_table WHERE project_id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        conn.execute("DELETE FROM mantis_project_table WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn add_subproject(&self, parent_id: i64, child_id: i64, inherit_parent: bool) -> Result<(), String> {
        let conn = self.conn();

        if parent_id == child_id {
            return Err("Cyclic project hierarchy detected".to_string());
        }

        if Self::query_project_by_id(&conn, parent_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("Project {} not found", parent_id));
        }
        if Self::query_project_by_id(&conn, child_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("Project {} not found", child_id));
        }

        // Walk ancestors of parent_id to ensure child_id is not already an ancestor of parent_id
        let mut queue = vec![parent_id];
        let mut visited = HashSet::new();
        visited.insert(parent_id);

        while let Some(curr) = queue.pop() {
            let mut stmt = conn
                .prepare("SELECT parent_id FROM mantis_project_hierarchy_table WHERE child_id = ?1")
                .map_err(|e| e.to_string())?;
            let parents = stmt
                .query_map(params![curr], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?;
            for p in parents {
                let ancestor = p.map_err(|e| e.to_string())?;
                if ancestor == child_id {
                    return Err("Cyclic project hierarchy detected".to_string());
                }
                if visited.insert(ancestor) {
                    queue.push(ancestor);
                }
            }
        }

        let inherit_parent_val = if inherit_parent { 1 } else { 0 };
        conn.execute(
            "INSERT INTO mantis_project_hierarchy_table (child_id, parent_id, inherit_parent)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(child_id, parent_id) DO UPDATE SET inherit_parent = excluded.inherit_parent",
            params![child_id, parent_id, inherit_parent_val],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn remove_subproject(&self, parent_id: i64, child_id: i64) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_project_hierarchy_table WHERE parent_id = ?1 AND child_id = ?2",
            params![parent_id, child_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_project_user(&self, project_id: i64, user_id: i64, access_level: i32) -> Result<(), String> {
        let conn = self.conn();
        if Self::query_project_by_id(&conn, project_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("Project {} not found", project_id));
        }
        if Self::query_user_by_id(&conn, user_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("User {} not found", user_id));
        }

        conn.execute(
            "INSERT INTO mantis_project_user_list_table (project_id, user_id, access_level)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id, user_id) DO UPDATE SET access_level = excluded.access_level",
            params![project_id, user_id, access_level],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn remove_project_user(&self, project_id: i64, user_id: i64) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_project_user_list_table WHERE project_id = ?1 AND user_id = ?2",
            params![project_id, user_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_subprojects(&self, parent_id: i64) -> rusqlite::Result<Vec<(Project, bool)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.status, p.enabled, p.view_state, p.access_min, p.description, p.category_id, p.inherit_global, h.inherit_parent
             FROM mantis_project_hierarchy_table h
             JOIN mantis_project_table p ON h.child_id = p.id
             WHERE h.parent_id = ?1
             ORDER BY p.name ASC",
        )?;
        let rows = stmt.query_map(params![parent_id], |row| {
            let proj = Project {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                enabled: row.get(3)?,
                view_state: row.get(4)?,
                access_min: row.get(5)?,
                description: row.get(6)?,
                category_id: row.get(7)?,
                inherit_global: row.get(8)?,
            };
            let inherit_parent: i32 = row.get(9)?;
            Ok((proj, inherit_parent == 1))
        })?;
        rows.collect()
    }

    pub fn list_project_users(&self, project_id: i64) -> rusqlite::Result<Vec<(User, i32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.username, u.realname, u.email, u.password, u.enabled, u.protected, u.access_level,
                    u.login_count, u.lost_password_request_count, u.failed_login_count, u.cookie_string, u.last_visit, u.date_created,
                    pu.access_level
             FROM mantis_project_user_list_table pu
             JOIN mantis_user_table u ON pu.user_id = u.id
             WHERE pu.project_id = ?1
             ORDER BY u.username ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let user = User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            };
            let access_level: i32 = row.get(14)?;
            Ok((user, access_level))
        })?;
        rows.collect()
    }

    pub fn list_all_projects(&self) -> rusqlite::Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, status, enabled, view_state, access_min, description, category_id, inherit_global
             FROM mantis_project_table
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                enabled: row.get(3)?,
                view_state: row.get(4)?,
                access_min: row.get(5)?,
                description: row.get(6)?,
                category_id: row.get(7)?,
                inherit_global: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn effective_access_level(&self, user: &User, project_id: i64) -> rusqlite::Result<i32> {
        // 1. All Projects context
        if project_id == 0 {
            return Ok(user.access_level);
        }

        // 2. Administrator
        if user.access_level >= 90 {
            return Ok(90);
        }

        let conn = self.conn();

        // 3. Project status & enabled check
        let mut stmt = conn.prepare(
            "SELECT enabled, view_state FROM mantis_project_table WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![project_id])?;
        let (enabled, view_state) = match rows.next()? {
            Some(row) => (row.get::<_, i32>(0)?, row.get::<_, i32>(1)?),
            None => return Ok(0),
        };

        if enabled == 0 {
            return Ok(0);
        }

        // 4. Project-specific membership override
        let mut stmt = conn.prepare(
            "SELECT access_level FROM mantis_project_user_list_table WHERE project_id = ?1 AND user_id = ?2",
        )?;
        let mut rows = stmt.query(params![project_id, user.id])?;
        if let Some(row) = rows.next()? {
            return row.get(0);
        }

        // 5. View state access level
        if view_state == 10 {
            // Public
            return Ok(user.access_level);
        } else if view_state == 50 {
            // Private
            if user.access_level >= 70 {
                return Ok(user.access_level);
            } else {
                return Ok(0);
            }
        }

        Ok(0)
    }

    pub fn list_visible_projects(&self, user: &User) -> rusqlite::Result<Vec<(Project, usize)>> {
        let all_projects = {
            let conn = self.conn();
            let mut stmt = conn.prepare(
                "SELECT id, name, status, enabled, view_state, access_min, description, category_id, inherit_global
                 FROM mantis_project_table
                 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    status: row.get(2)?,
                    enabled: row.get(3)?,
                    view_state: row.get(4)?,
                    access_min: row.get(5)?,
                    description: row.get(6)?,
                    category_id: row.get(7)?,
                    inherit_global: row.get(8)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        // Filter projects visible to user
        let mut visible_map = HashMap::new();
        for proj in all_projects {
            if self.effective_access_level(user, proj.id)? > 0 {
                visible_map.insert(proj.id, proj);
            }
        }

        let visible_ids: HashSet<i64> = visible_map.keys().copied().collect();

        // Query hierarchy links between visible projects
        let hierarchy_links: Vec<(i64, i64)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT child_id, parent_id FROM mantis_project_hierarchy_table")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            let mut links = Vec::new();
            for r in rows {
                let (child, parent) = r?;
                if visible_ids.contains(&child) && visible_ids.contains(&parent) {
                    links.push((child, parent));
                }
            }
            links
        };

        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut has_visible_parent = HashSet::new();
        for (child_id, parent_id) in hierarchy_links {
            children_map.entry(parent_id).or_default().push(child_id);
            has_visible_parent.insert(child_id);
        }

        // Sort children of each parent by name ASC
        for children in children_map.values_mut() {
            children.sort_by(|a, b| {
                let name_a = visible_map.get(a).map(|p| p.name.as_str()).unwrap_or("");
                let name_b = visible_map.get(b).map(|p| p.name.as_str()).unwrap_or("");
                name_a.cmp(name_b)
            });
        }

        // Roots are visible projects without visible parent
        let mut roots: Vec<i64> = visible_ids
            .iter()
            .copied()
            .filter(|id| !has_visible_parent.contains(id))
            .collect();
        roots.sort_by(|a, b| {
            let name_a = visible_map.get(a).map(|p| p.name.as_str()).unwrap_or("");
            let name_b = visible_map.get(b).map(|p| p.name.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        fn dfs(
            curr_id: i64,
            depth: usize,
            visible_map: &HashMap<i64, Project>,
            children_map: &HashMap<i64, Vec<i64>>,
            visited: &mut HashSet<i64>,
            result: &mut Vec<(Project, usize)>,
        ) {
            if !visited.insert(curr_id) {
                return;
            }
            if let Some(proj) = visible_map.get(&curr_id) {
                result.push((proj.clone(), depth));
            }
            if let Some(children) = children_map.get(&curr_id) {
                for &child_id in children {
                    dfs(child_id, depth + 1, visible_map, children_map, visited, result);
                }
            }
        }

        let mut visited = HashSet::new();
        let mut result = Vec::new();

        for root_id in roots {
            dfs(root_id, 0, &visible_map, &children_map, &mut visited, &mut result);
        }

        // Cycle fallback: any remaining unvisited visible projects
        let mut remaining: Vec<i64> = visible_ids
            .iter()
            .copied()
            .filter(|id| !visited.contains(id))
            .collect();
        remaining.sort_by(|a, b| {
            let name_a = visible_map.get(a).map(|p| p.name.as_str()).unwrap_or("");
            let name_b = visible_map.get(b).map(|p| p.name.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });
        for id in remaining {
            dfs(id, 0, &visible_map, &children_map, &mut visited, &mut result);
        }

        Ok(result)
    }

    pub fn add_category(&self, project_id: i64, name: &str, user_id: i64) -> Result<i64, String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        let conn = self.conn();
        if project_id != 0 && Self::query_project_by_id(&conn, project_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("Project {} not found", project_id));
        }

        conn.execute(
            "INSERT INTO mantis_category_table (project_id, user_id, name, status)
             VALUES (?1, ?2, ?3, 0)",
            params![project_id, user_id, trimmed_name],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    pub fn update_category(&self, id: i64, name: &str, user_id: i64) -> Result<(), String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        let conn = self.conn();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_category_table WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .map_err(|e| e.to_string())?;

        if !exists {
            return Err(format!("Category {} not found", id));
        }

        conn.execute(
            "UPDATE mantis_category_table SET name = ?1, user_id = ?2 WHERE id = ?3",
            params![trimmed_name, user_id, id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_category(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_category_table WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .map_err(|e| e.to_string())?;

        if !exists {
            return Err(format!("Category {} not found", id));
        }

        conn.execute("DELETE FROM mantis_category_table WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_category_by_id(&self, id: i64) -> rusqlite::Result<Option<Category>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, project_id, user_id, name, status FROM mantis_category_table WHERE id = ?1",
            params![id],
            |row| {
                Ok(Category {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    user_id: row.get(2)?,
                    name: row.get(3)?,
                    status: row.get(4)?,
                })
            },
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_direct_categories(&self, project_id: i64) -> rusqlite::Result<Vec<Category>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, user_id, name, status
             FROM mantis_category_table
             WHERE project_id = ?1
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(Category {
                id: row.get(0)?,
                project_id: row.get(1)?,
                user_id: row.get(2)?,
                name: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_effective_categories(&self, project_id: i64) -> rusqlite::Result<Vec<Category>> {
        let conn = self.conn();
        if project_id == 0 {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, user_id, name, status
                 FROM mantis_category_table
                 WHERE project_id = 0
                 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    user_id: row.get(2)?,
                    name: row.get(3)?,
                    status: row.get(4)?,
                })
            })?;
            return rows.collect();
        }

        // Ordered list of project IDs to inherit categories from (highest to lowest precedence)
        let mut project_ids = vec![project_id];
        let mut visited = HashSet::new();
        visited.insert(project_id);

        let mut include_global = {
            let inherit_global: Option<i32> = conn
                .query_row(
                    "SELECT inherit_global FROM mantis_project_table WHERE id = ?1",
                    params![project_id],
                    |row| row.get(0),
                )
                .ok();
            inherit_global.unwrap_or(1) == 1
        };

        // Traverse hierarchy for ancestors where inherit_parent = 1
        let mut current_ids = vec![project_id];
        while !current_ids.is_empty() {
            let mut next_ids = Vec::new();
            for pid in current_ids {
                let mut stmt = conn.prepare(
                    "SELECT parent_id FROM mantis_project_hierarchy_table WHERE child_id = ?1 AND inherit_parent = 1",
                )?;
                let parents = stmt.query_map(params![pid], |row| row.get::<_, i64>(0))?;
                for p in parents {
                    let parent_id = p?;
                    if visited.insert(parent_id) {
                        project_ids.push(parent_id);
                        next_ids.push(parent_id);

                        let inherit_g: Option<i32> = conn
                            .query_row(
                                "SELECT inherit_global FROM mantis_project_table WHERE id = ?1",
                                params![parent_id],
                                |row| row.get(0),
                            )
                            .ok();
                        if inherit_g.unwrap_or(1) == 1 {
                            include_global = true;
                        }
                    }
                }
            }
            current_ids = next_ids;
        }

        if include_global {
            project_ids.push(0);
        }

        let mut seen_names = HashSet::new();
        let mut result = Vec::new();

        for pid in project_ids {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, user_id, name, status
                 FROM mantis_category_table
                 WHERE project_id = ?1
                 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map(params![pid], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    user_id: row.get(2)?,
                    name: row.get(3)?,
                    status: row.get(4)?,
                })
            })?;
            for cat_res in rows {
                let cat = cat_res?;
                if seen_names.insert(cat.name.clone()) {
                    result.push(cat);
                }
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    pub fn add_version(
        &self,
        project_id: i64,
        version: &str,
        description: &str,
        released: bool,
        obsolete: bool,
    ) -> Result<i64, String> {
        let trimmed_version = version.trim();
        if trimmed_version.is_empty() {
            return Err("Version cannot be empty".to_string());
        }

        let conn = self.conn();
        if Self::query_project_by_id(&conn, project_id).map_err(|e| e.to_string())?.is_none() {
            return Err(format!("Project {} not found", project_id));
        }

        let released_val = if released { 1 } else { 0 };
        let obsolete_val = if obsolete { 1 } else { 0 };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO mantis_project_version_table (
                project_id, version, description, released, obsolete, date_order
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                project_id,
                trimmed_version,
                description,
                released_val,
                obsolete_val,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    pub fn update_version(
        &self,
        id: i64,
        version: &str,
        description: &str,
        released: bool,
        obsolete: bool,
    ) -> Result<(), String> {
        let trimmed_version = version.trim();
        if trimmed_version.is_empty() {
            return Err("Version cannot be empty".to_string());
        }

        let conn = self.conn();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_project_version_table WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .map_err(|e| e.to_string())?;

        if !exists {
            return Err(format!("Version {} not found", id));
        }

        let released_val = if released { 1 } else { 0 };
        let obsolete_val = if obsolete { 1 } else { 0 };

        conn.execute(
            "UPDATE mantis_project_version_table
             SET version = ?1, description = ?2, released = ?3, obsolete = ?4
             WHERE id = ?5",
            params![trimmed_version, description, released_val, obsolete_val, id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_version(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_project_version_table WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .map_err(|e| e.to_string())?;

        if !exists {
            return Err(format!("Version {} not found", id));
        }

        conn.execute("DELETE FROM mantis_project_version_table WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn list_versions(&self, project_id: i64) -> rusqlite::Result<Vec<ProjectVersion>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, version, description, released, obsolete, date_order
             FROM mantis_project_version_table
             WHERE project_id = ?1
             ORDER BY date_order DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(ProjectVersion {
                id: row.get(0)?,
                project_id: row.get(1)?,
                version: row.get(2)?,
                description: row.get(3)?,
                released: row.get(4)?,
                obsolete: row.get(5)?,
                date_order: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_version_by_id(&self, id: i64) -> rusqlite::Result<Option<ProjectVersion>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, project_id, version, description, released, obsolete, date_order FROM mantis_project_version_table WHERE id = ?1",
            params![id],
            |row| {
                Ok(ProjectVersion {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    version: row.get(2)?,
                    description: row.get(3)?,
                    released: row.get(4)?,
                    obsolete: row.get(5)?,
                    date_order: row.get(6)?,
                })
            },
        ) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ─── Sub-project 2: Issue Tracking & Workflow ────────────────────────────

    fn map_bug_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bug> {
        Ok(Bug {
            id: row.get(0)?,
            project_id: row.get(1)?,
            reporter_id: row.get(2)?,
            handler_id: row.get(3)?,
            duplicate_id: row.get(4)?,
            priority: row.get(5)?,
            severity: row.get(6)?,
            reproducibility: row.get(7)?,
            status: row.get(8)?,
            resolution: row.get(9)?,
            category_id: row.get(10)?,
            date_submitted: row.get(11)?,
            last_updated: row.get(12)?,
            os: row.get(13)?,
            os_build: row.get(14)?,
            platform: row.get(15)?,
            version: row.get(16)?,
            fixed_in_version: row.get(17)?,
            target_version: row.get(18)?,
            build: row.get(19)?,
            summary: row.get(20)?,
            view_state: row.get(21)?,
            sticky: row.get(22)?,
            due_date: row.get(23)?,
        })
    }

    fn query_bug_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Bug>> {
        match conn.query_row(
            "SELECT id, project_id, reporter_id, handler_id, duplicate_id,
                    priority, severity, reproducibility, status, resolution,
                    category_id, date_submitted, last_updated, os, os_build,
                    platform, version, fixed_in_version, target_version, build,
                    summary, view_state, sticky, due_date
             FROM mantis_bug_table
             WHERE id = ?1",
            params![id],
            Self::map_bug_row,
        ) {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_history(
        &self,
        conn: &Connection,
        bug_id: i64,
        user_id: i64,
        field_name: &str,
        old_value: &str,
        new_value: &str,
        type_: i32,
    ) -> rusqlite::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_bug_history_table (
                user_id, bug_id, date_modified, field_name, old_value, new_value, type
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![user_id, bug_id, now, field_name, old_value, new_value, type_],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_bug(
        &self,
        project_id: i64,
        reporter_id: i64,
        category_id: i64,
        summary: &str,
        description: &str,
        steps_to_reproduce: &str,
        additional_information: &str,
        priority: i32,
        severity: i32,
        reproducibility: i32,
        version: &str,
        os: &str,
        os_build: &str,
        platform: &str,
        view_state: i32,
    ) -> Result<i64, String> {
        let trimmed_summary = summary.trim();
        if trimmed_summary.is_empty() {
            return Err("Summary cannot be empty".to_string());
        }

        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO mantis_bug_table (
                project_id, reporter_id, handler_id, duplicate_id,
                priority, severity, reproducibility, status, resolution,
                category_id, date_submitted, last_updated, os, os_build,
                platform, version, fixed_in_version, target_version, build,
                summary, view_state, sticky, due_date
            ) VALUES (?1, ?2, 0, 0, ?3, ?4, ?5, 10, 10, ?6, ?7, ?7, ?8, ?9, ?10, ?11, '', '', '', ?12, ?13, 0, 0)",
            params![
                project_id,
                reporter_id,
                priority,
                severity,
                reproducibility,
                category_id,
                now,
                os,
                os_build,
                platform,
                version,
                trimmed_summary,
                view_state,
            ],
        )
        .map_err(|e| e.to_string())?;

        let bug_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO mantis_bug_text_table (
                id, description, steps_to_reproduce, additional_information
            ) VALUES (?1, ?2, ?3, ?4)",
            params![bug_id, description, steps_to_reproduce, additional_information],
        )
        .map_err(|e| e.to_string())?;

        // Record new bug creation in history (type = 1)
        let _ = self.record_history(&conn, bug_id, reporter_id, "", "", "", 1);
        self.notify_bug_event(
            &conn,
            bug_id,
            reporter_id,
            &format!("[Issue #{:07}] New: {}", bug_id, trimmed_summary),
            &format!("Issue #{:07} reported: {}\n\n{}", bug_id, trimmed_summary, description),
        );

        Ok(bug_id)
    }

    pub fn get_bug(&self, id: i64) -> rusqlite::Result<Option<Bug>> {
        let conn = self.conn();
        Self::query_bug_by_id(&conn, id)
    }

    pub fn get_bug_full(&self, id: i64) -> rusqlite::Result<Option<BugFull>> {
        let conn = self.conn();
        let bug = match Self::query_bug_by_id(&conn, id)? {
            Some(b) => b,
            None => return Ok(None),
        };

        let (description, steps_to_reproduce, additional_information) = match conn.query_row(
            "SELECT description, steps_to_reproduce, additional_information
             FROM mantis_bug_text_table
             WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ) {
            Ok(t) => t,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                (String::new(), String::new(), String::new())
            }
            Err(e) => return Err(e),
        };

        Ok(Some(BugFull {
            bug,
            description,
            steps_to_reproduce,
            additional_information,
        }))
    }

    pub fn list_bugs(
        &self,
        project_id: i64,
        user_access: i32,
        offset: i64,
        limit: i64,
    ) -> rusqlite::Result<Vec<Bug>> {
        let conn = self.conn();
        if project_id > 0 {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, reporter_id, handler_id, duplicate_id,
                        priority, severity, reproducibility, status, resolution,
                        category_id, date_submitted, last_updated, os, os_build,
                        platform, version, fixed_in_version, target_version, build,
                        summary, view_state, sticky, due_date
                 FROM mantis_bug_table
                 WHERE project_id = ?1 AND (view_state = 10 OR ?2 >= 55)
                 ORDER BY sticky DESC, last_updated DESC, id DESC
                 LIMIT ?3 OFFSET ?4",
            )?;
            let rows = stmt.query_map(
                params![project_id, user_access, limit, offset],
                Self::map_bug_row,
            )?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, reporter_id, handler_id, duplicate_id,
                        priority, severity, reproducibility, status, resolution,
                        category_id, date_submitted, last_updated, os, os_build,
                        platform, version, fixed_in_version, target_version, build,
                        summary, view_state, sticky, due_date
                 FROM mantis_bug_table
                 WHERE (view_state = 10 OR ?1 >= 55)
                 ORDER BY sticky DESC, last_updated DESC, id DESC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![user_access, limit, offset], Self::map_bug_row)?;
            rows.collect()
        }
    }

    pub fn update_bug_fields(
        &self,
        id: i64,
        updater_id: i64,
        fields: BugUpdate,
    ) -> Result<Vec<BugHistory>, String> {
        let conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", id))?;

        let (cur_desc, cur_steps, cur_add_info) = conn
            .query_row(
                "SELECT description, steps_to_reproduce, additional_information
                 FROM mantis_bug_text_table WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap_or_else(|_| (String::new(), String::new(), String::new()));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut changes: Vec<(&str, String, String)> = Vec::new();

        let new_summary = match fields.summary {
            Some(ref s) => {
                let t = s.trim().to_string();
                if t.is_empty() {
                    return Err("Summary cannot be empty".to_string());
                }
                if t != bug.summary {
                    changes.push(("summary", bug.summary.clone(), t.clone()));
                }
                t
            }
            None => bug.summary.clone(),
        };

        let new_category_id = match fields.category_id {
            Some(v) => {
                if v != bug.category_id {
                    changes.push(("category_id", bug.category_id.to_string(), v.to_string()));
                }
                v
            }
            None => bug.category_id,
        };

        let new_priority = match fields.priority {
            Some(v) => {
                if v != bug.priority {
                    changes.push(("priority", bug.priority.to_string(), v.to_string()));
                }
                v
            }
            None => bug.priority,
        };

        let new_severity = match fields.severity {
            Some(v) => {
                if v != bug.severity {
                    changes.push(("severity", bug.severity.to_string(), v.to_string()));
                }
                v
            }
            None => bug.severity,
        };

        let new_reproducibility = match fields.reproducibility {
            Some(v) => {
                if v != bug.reproducibility {
                    changes.push((
                        "reproducibility",
                        bug.reproducibility.to_string(),
                        v.to_string(),
                    ));
                }
                v
            }
            None => bug.reproducibility,
        };

        let new_version = match fields.version {
            Some(ref v) => {
                if *v != bug.version {
                    changes.push(("version", bug.version.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.version.clone(),
        };

        let new_target_version = match fields.target_version {
            Some(ref v) => {
                if *v != bug.target_version {
                    changes.push(("target_version", bug.target_version.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.target_version.clone(),
        };

        let new_fixed_in_version = match fields.fixed_in_version {
            Some(ref v) => {
                if *v != bug.fixed_in_version {
                    changes.push(("fixed_in_version", bug.fixed_in_version.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.fixed_in_version.clone(),
        };

        let new_view_state = match fields.view_state {
            Some(v) => {
                if v != bug.view_state {
                    changes.push(("view_state", bug.view_state.to_string(), v.to_string()));
                }
                v
            }
            None => bug.view_state,
        };

        let new_handler_id = match fields.handler_id {
            Some(v) => {
                if v != bug.handler_id {
                    changes.push(("handler_id", bug.handler_id.to_string(), v.to_string()));
                }
                v
            }
            None => bug.handler_id,
        };

        let new_desc = match fields.description {
            Some(ref v) => {
                if *v != cur_desc {
                    changes.push(("description", cur_desc.clone(), v.clone()));
                }
                v.clone()
            }
            None => cur_desc.clone(),
        };

        let new_steps = match fields.steps_to_reproduce {
            Some(ref v) => {
                if *v != cur_steps {
                    changes.push(("steps_to_reproduce", cur_steps.clone(), v.clone()));
                }
                v.clone()
            }
            None => cur_steps.clone(),
        };

        let new_add_info = match fields.additional_information {
            Some(ref v) => {
                if *v != cur_add_info {
                    changes.push(("additional_information", cur_add_info.clone(), v.clone()));
                }
                v.clone()
            }
            None => cur_add_info.clone(),
        };

        if !changes.is_empty() {
            conn.execute(
                "UPDATE mantis_bug_table
                 SET summary = ?1, category_id = ?2, priority = ?3, severity = ?4,
                     reproducibility = ?5, version = ?6, target_version = ?7,
                     fixed_in_version = ?8, view_state = ?9, handler_id = ?10,
                     last_updated = ?11
                 WHERE id = ?12",
                params![
                    new_summary,
                    new_category_id,
                    new_priority,
                    new_severity,
                    new_reproducibility,
                    new_version,
                    new_target_version,
                    new_fixed_in_version,
                    new_view_state,
                    new_handler_id,
                    now,
                    id,
                ],
            )
            .map_err(|e| e.to_string())?;

            conn.execute(
                "UPDATE mantis_bug_text_table
                 SET description = ?1, steps_to_reproduce = ?2, additional_information = ?3
                 WHERE id = ?4",
                params![new_desc, new_steps, new_add_info, id],
            )
            .map_err(|e| e.to_string())?;
        }

        let mut recorded = Vec::new();
        for (field_name, old_val, new_val) in changes {
            self.record_history(&conn, id, updater_id, field_name, &old_val, &new_val, 0)
                .map_err(|e| e.to_string())?;
            let hid = conn.last_insert_rowid();
            recorded.push(BugHistory {
                id: hid,
                user_id: updater_id,
                bug_id: id,
                date_modified: now,
                field_name: field_name.to_string(),
                old_value: old_val,
                new_value: new_val,
                type_: 0,
            });
        }

        Ok(recorded)
    }

    pub fn delete_bug(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM mantis_bug_table WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn change_bug_status(
        &self,
        bug_id: i64,
        user_id: i64,
        new_status: i32,
        resolution: i32,
        note: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", bug_id))?;

        // Closed bug (status == 90) can only transition to feedback (20) to reopen
        if bug.status == 90 && new_status != 20 && new_status != 90 {
            return Err(
                "Closed bug can only be reopened to feedback (status 20)".to_string(),
            );
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "UPDATE mantis_bug_table
             SET status = ?1, resolution = ?2, last_updated = ?3
             WHERE id = ?4",
            params![new_status, resolution, now, bug_id],
        )
        .map_err(|e| e.to_string())?;

        if bug.status != new_status {
            self.record_history(
                &conn,
                bug_id,
                user_id,
                "status",
                &bug.status.to_string(),
                &new_status.to_string(),
                0,
            )
            .map_err(|e| e.to_string())?;
        }

        if bug.resolution != resolution {
            self.record_history(
                &conn,
                bug_id,
                user_id,
                "resolution",
                &bug.resolution.to_string(),
                &resolution.to_string(),
                0,
            )
            .map_err(|e| e.to_string())?;
        }

        if let Some(note_text) = note {
            let trimmed = note_text.trim();
            if !trimmed.is_empty() {
                Self::insert_bugnote_inner(&conn, self, bug_id, user_id, trimmed, 10, 0)
                    .map_err(|e| e.to_string())?;
            }
        }

        self.notify_bug_event(
            &conn,
            bug_id,
            user_id,
            &format!("[Issue #{:07}] Status changed to {}", bug_id, new_status),
            &format!(
                "Issue #{:07} ({}) status changed from {} to {} (resolution {}).",
                bug_id, bug.summary, bug.status, new_status, resolution
            ),
        );

        Ok(())
    }

    pub fn assign_bug(&self, bug_id: i64, user_id: i64, handler_id: i64) -> Result<(), String> {
        let conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", bug_id))?;

        if handler_id > 0
            && Self::query_user_by_id(&conn, handler_id)
                .map_err(|e| e.to_string())?
                .is_none()
        {
            return Err(format!("User {} not found", handler_id));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let new_status = if handler_id > 0 && bug.status < 50 {
            50 // Assigned
        } else {
            bug.status
        };

        conn.execute(
            "UPDATE mantis_bug_table
             SET handler_id = ?1, status = ?2, last_updated = ?3
             WHERE id = ?4",
            params![handler_id, new_status, now, bug_id],
        )
        .map_err(|e| e.to_string())?;

        if bug.handler_id != handler_id {
            self.record_history(
                &conn,
                bug_id,
                user_id,
                "handler_id",
                &bug.handler_id.to_string(),
                &handler_id.to_string(),
                0,
            )
            .map_err(|e| e.to_string())?;
        }

        if bug.status != new_status {
            self.record_history(
                &conn,
                bug_id,
                user_id,
                "status",
                &bug.status.to_string(),
                &new_status.to_string(),
                0,
            )
            .map_err(|e| e.to_string())?;
        }

        self.notify_bug_event(
            &conn,
            bug_id,
            user_id,
            &format!("[Issue #{:07}] Assigned: {}", bug_id, bug.summary),
            &format!(
                "Issue #{:07} ({}) assigned to user #{}.",
                bug_id, bug.summary, handler_id
            ),
        );

        Ok(())
    }

    fn insert_bugnote_inner(
        conn: &Connection,
        db: &Db,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
    ) -> rusqlite::Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO mantis_bugnote_text_table (note) VALUES (?1)",
            params![note],
        )?;
        let text_id = conn.last_insert_rowid();

        let safe_time = time_tracking.max(0);
        conn.execute(
            "INSERT INTO mantis_bugnote_table (
                bug_id, reporter_id, bugnote_text_id, view_state,
                date_submitted, last_modified, note_type, note_attr, time_tracking
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0, '', ?6)",
            params![bug_id, reporter_id, text_id, view_state, now, safe_time],
        )?;
        let note_id = conn.last_insert_rowid();

        conn.execute(
            "UPDATE mantis_bug_table SET last_updated = ?1 WHERE id = ?2",
            params![now, bug_id],
        )?;

        db.record_history(
            conn,
            bug_id,
            reporter_id,
            "bugnote_added",
            "",
            &note_id.to_string(),
            1,
        )?;

        if safe_time > 0 {
            db.record_history(
                conn,
                bug_id,
                reporter_id,
                "time_tracking",
                "0",
                &safe_time.to_string(),
                0,
            )?;
        }

        db.notify_bug_event(
            conn,
            bug_id,
            reporter_id,
            &format!("[Issue #{:07}] Note added", bug_id),
            &format!("Note added to Issue #{:07}:\n\n{}", bug_id, note),
        );

        Ok(note_id)
    }

    pub fn add_bugnote(
        &self,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
    ) -> Result<i64, String> {
        self.add_bugnote_with_time(bug_id, reporter_id, note, view_state, 0)
    }

    pub fn add_bugnote_with_time(
        &self,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
    ) -> Result<i64, String> {
        let trimmed = note.trim();
        if trimmed.is_empty() {
            return Err("Note cannot be empty".to_string());
        }

        let conn = self.conn();
        if Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Bug {} not found", bug_id));
        }

        Self::insert_bugnote_inner(
            &conn,
            self,
            bug_id,
            reporter_id,
            trimmed,
            view_state,
            time_tracking,
        )
        .map_err(|e| e.to_string())
    }

    pub fn get_bugnote_by_id(&self, note_id: i64) -> rusqlite::Result<Option<Bugnote>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT n.id, n.bug_id, n.reporter_id, n.bugnote_text_id, n.view_state,
                    n.date_submitted, n.last_modified, n.note_type, n.note_attr,
                    n.time_tracking, COALESCE(t.note, '')
             FROM mantis_bugnote_table n
             LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
             WHERE n.id = ?1",
            params![note_id],
            |row| {
                Ok(Bugnote {
                    id: row.get(0)?,
                    bug_id: row.get(1)?,
                    reporter_id: row.get(2)?,
                    bugnote_text_id: row.get(3)?,
                    view_state: row.get(4)?,
                    date_submitted: row.get(5)?,
                    last_modified: row.get(6)?,
                    note_type: row.get(7)?,
                    note_attr: row.get(8)?,
                    time_tracking: row.get(9)?,
                    note: row.get(10)?,
                })
            },
        ) {
            Ok(n) => Ok(Some(n)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_bugnote(&self, note_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        let text_id: Option<i64> = conn
            .query_row(
                "SELECT bugnote_text_id FROM mantis_bugnote_table WHERE id = ?1",
                params![note_id],
                |row| row.get(0),
            )
            .ok();

        conn.execute(
            "DELETE FROM mantis_bugnote_table WHERE id = ?1",
            params![note_id],
        )?;

        if let Some(tid) = text_id {
            let _ = conn.execute(
                "DELETE FROM mantis_bugnote_text_table WHERE id = ?1",
                params![tid],
            );
        }
        Ok(())
    }

    pub fn list_bugnotes(&self, bug_id: i64, user_access: i32) -> rusqlite::Result<Vec<Bugnote>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT n.id, n.bug_id, n.reporter_id, n.bugnote_text_id, n.view_state,
                    n.date_submitted, n.last_modified, n.note_type, n.note_attr,
                    n.time_tracking, COALESCE(t.note, '')
             FROM mantis_bugnote_table n
             LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
             WHERE n.bug_id = ?1 AND (n.view_state = 10 OR ?2 >= 55)
             ORDER BY n.date_submitted ASC, n.id ASC",
        )?;
        let rows = stmt.query_map(params![bug_id, user_access], |row| {
            Ok(Bugnote {
                id: row.get(0)?,
                bug_id: row.get(1)?,
                reporter_id: row.get(2)?,
                bugnote_text_id: row.get(3)?,
                view_state: row.get(4)?,
                date_submitted: row.get(5)?,
                last_modified: row.get(6)?,
                note_type: row.get(7)?,
                note_attr: row.get(8)?,
                time_tracking: row.get(9)?,
                note: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_bug_history(&self, bug_id: i64) -> rusqlite::Result<Vec<BugHistory>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, bug_id, date_modified, field_name, old_value, new_value, type
             FROM mantis_bug_history_table
             WHERE bug_id = ?1
             ORDER BY date_modified ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![bug_id], |row| {
            Ok(BugHistory {
                id: row.get(0)?,
                user_id: row.get(1)?,
                bug_id: row.get(2)?,
                date_modified: row.get(3)?,
                field_name: row.get(4)?,
                old_value: row.get(5)?,
                new_value: row.get(6)?,
                type_: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_bug_relationship(
        &self,
        source_id: i64,
        dest_id: i64,
        rel_type: i32,
    ) -> Result<(), String> {
        if source_id == dest_id {
            return Err("A bug cannot have a relationship with itself".to_string());
        }

        let conn = self.conn();
        if Self::query_bug_by_id(&conn, source_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Source bug {} not found", source_id));
        }
        if Self::query_bug_by_id(&conn, dest_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Destination bug {} not found", dest_id));
        }

        conn.execute(
            "INSERT INTO mantis_bug_relationship_table (
                source_bug_id, destination_bug_id, relationship_type
            ) VALUES (?1, ?2, ?3)",
            params![source_id, dest_id, rel_type],
        )
        .map_err(|e| e.to_string())?;

        // Auto-create reverse relationship for duplicate_of (1) <-> has_duplicate (2)
        if rel_type == 1 {
            conn.execute(
                "UPDATE mantis_bug_table SET duplicate_id = ?1 WHERE id = ?2",
                params![dest_id, source_id],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO mantis_bug_relationship_table (
                    source_bug_id, destination_bug_id, relationship_type
                ) VALUES (?1, ?2, 2)",
                params![dest_id, source_id],
            )
            .map_err(|e| e.to_string())?;
        } else if rel_type == 2 {
            conn.execute(
                "UPDATE mantis_bug_table SET duplicate_id = ?1 WHERE id = ?2",
                params![source_id, dest_id],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO mantis_bug_relationship_table (
                    source_bug_id, destination_bug_id, relationship_type
                ) VALUES (?1, ?2, 1)",
                params![dest_id, source_id],
            )
            .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn get_bug_relationship_by_id(
        &self,
        rel_id: i64,
    ) -> rusqlite::Result<Option<BugRelationship>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, source_bug_id, destination_bug_id, relationship_type
             FROM mantis_bug_relationship_table
             WHERE id = ?1",
            params![rel_id],
            |row| {
                Ok(BugRelationship {
                    id: row.get(0)?,
                    source_bug_id: row.get(1)?,
                    destination_bug_id: row.get(2)?,
                    relationship_type: row.get(3)?,
                })
            },
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_bug_relationship(&self, rel_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        let rel: Option<(i64, i64, i32)> = conn
            .query_row(
                "SELECT source_bug_id, destination_bug_id, relationship_type
                 FROM mantis_bug_relationship_table
                 WHERE id = ?1",
                params![rel_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        conn.execute(
            "DELETE FROM mantis_bug_relationship_table WHERE id = ?1",
            params![rel_id],
        )?;

        if let Some((src, dst, rtype)) = rel {
            if rtype == 1 {
                conn.execute(
                    "DELETE FROM mantis_bug_relationship_table
                     WHERE source_bug_id = ?1 AND destination_bug_id = ?2 AND relationship_type = 2",
                    params![dst, src],
                )?;
                conn.execute(
                    "UPDATE mantis_bug_table SET duplicate_id = 0 WHERE id = ?1 AND duplicate_id = ?2",
                    params![src, dst],
                )?;
            } else if rtype == 2 {
                conn.execute(
                    "DELETE FROM mantis_bug_relationship_table
                     WHERE source_bug_id = ?1 AND destination_bug_id = ?2 AND relationship_type = 1",
                    params![dst, src],
                )?;
                conn.execute(
                    "UPDATE mantis_bug_table SET duplicate_id = 0 WHERE id = ?1 AND duplicate_id = ?2",
                    params![dst, src],
                )?;
            }
        }

        Ok(())
    }

    pub fn list_bug_relationships(&self, bug_id: i64) -> rusqlite::Result<Vec<BugRelationship>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, source_bug_id, destination_bug_id, relationship_type
             FROM mantis_bug_relationship_table
             WHERE source_bug_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![bug_id], |row| {
            Ok(BugRelationship {
                id: row.get(0)?,
                source_bug_id: row.get(1)?,
                destination_bug_id: row.get(2)?,
                relationship_type: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_bug_file(
        &self,
        bug_id: i64,
        user_id: i64,
        filename: &str,
        file_type: &str,
        content: Vec<u8>,
        upload_dir: &str,
    ) -> Result<i64, String> {
        let trimmed_filename = filename.trim();
        if trimmed_filename.is_empty() {
            return Err("Filename cannot be empty".to_string());
        }

        let conn = self.conn();
        if Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Bug {} not found", bug_id));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let filesize = content.len() as i64;

        let (diskfile, folder, blob_opt): (String, String, Option<Vec<u8>>) =
            if upload_dir.trim().is_empty() {
                (String::new(), String::new(), Some(content))
            } else {
                std::fs::create_dir_all(upload_dir).map_err(|e| e.to_string())?;
                let uuid_name = format!(
                    "{:016x}{:016x}",
                    rand::random::<u64>(),
                    rand::random::<u64>()
                );
                let full_path = std::path::Path::new(upload_dir).join(&uuid_name);
                std::fs::write(full_path, &content).map_err(|e| e.to_string())?;
                (uuid_name, upload_dir.to_string(), None)
            };

        conn.execute(
            "INSERT INTO mantis_bug_file_table (
                bug_id, title, description, diskfile, filename, folder,
                filesize, file_type, content, date_added, user_id
            ) VALUES (?1, '', '', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                bug_id,
                diskfile,
                trimmed_filename,
                folder,
                filesize,
                file_type,
                blob_opt,
                now,
                user_id,
            ],
        )
        .map_err(|e| e.to_string())?;

        let file_id = conn.last_insert_rowid();

        self.record_history(&conn, bug_id, user_id, "file_added", "", trimmed_filename, 2)
            .map_err(|e| e.to_string())?;

        Ok(file_id)
    }

    pub fn get_bug_file(
        &self,
        file_id: i64,
        upload_dir: &str,
    ) -> rusqlite::Result<Option<(BugFile, Vec<u8>)>> {
        let conn = self.conn();
        let query_res = conn.query_row(
            "SELECT id, bug_id, title, description, diskfile, filename, folder,
                    filesize, file_type, content, date_added, user_id
             FROM mantis_bug_file_table
             WHERE id = ?1",
            params![file_id],
            |row| {
                let meta = BugFile {
                    id: row.get(0)?,
                    bug_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    diskfile: row.get(4)?,
                    filename: row.get(5)?,
                    folder: row.get(6)?,
                    filesize: row.get(7)?,
                    file_type: row.get(8)?,
                    date_added: row.get(10)?,
                    user_id: row.get(11)?,
                };
                let blob: Option<Vec<u8>> = row.get(9)?;
                Ok((meta, blob))
            },
        );
        let row_opt = match query_res {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        let (meta, blob_opt) = match row_opt {
            Some(v) => v,
            None => return Ok(None),
        };

        if let Some(bytes) = blob_opt {
            return Ok(Some((meta, bytes)));
        }

        let dir = if !upload_dir.trim().is_empty() {
            upload_dir
        } else {
            &meta.folder
        };
        let path = std::path::Path::new(dir).join(&meta.diskfile);
        let bytes = std::fs::read(path).unwrap_or_default();
        Ok(Some((meta, bytes)))
    }

    pub fn get_bug_file_meta(&self, file_id: i64) -> rusqlite::Result<Option<BugFile>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, bug_id, title, description, diskfile, filename, folder,
                    filesize, file_type, date_added, user_id
             FROM mantis_bug_file_table
             WHERE id = ?1",
            params![file_id],
            |row| {
                Ok(BugFile {
                    id: row.get(0)?,
                    bug_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    diskfile: row.get(4)?,
                    filename: row.get(5)?,
                    folder: row.get(6)?,
                    filesize: row.get(7)?,
                    file_type: row.get(8)?,
                    date_added: row.get(9)?,
                    user_id: row.get(10)?,
                })
            },
        ) {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_bug_file(&self, file_id: i64, upload_dir: &str) -> rusqlite::Result<()> {
        let conn = self.conn();
        let meta: Option<(i64, i64, String, String, String)> = conn
            .query_row(
                "SELECT bug_id, user_id, filename, diskfile, folder
                 FROM mantis_bug_file_table
                 WHERE id = ?1",
                params![file_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .ok();

        conn.execute(
            "DELETE FROM mantis_bug_file_table WHERE id = ?1",
            params![file_id],
        )?;

        if let Some((bug_id, user_id, filename, diskfile, folder)) = meta {
            if !diskfile.is_empty() {
                let dir = if !upload_dir.trim().is_empty() {
                    upload_dir
                } else {
                    &folder
                };
                let path = std::path::Path::new(dir).join(&diskfile);
                let _ = std::fs::remove_file(path);
            }
            let _ = self.record_history(&conn, bug_id, user_id, "file_deleted", &filename, "", 2);
        }

        Ok(())
    }

    pub fn list_bug_files(&self, bug_id: i64) -> rusqlite::Result<Vec<BugFile>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, bug_id, title, description, diskfile, filename, folder,
                    filesize, file_type, date_added, user_id
             FROM mantis_bug_file_table
             WHERE bug_id = ?1
             ORDER BY date_added ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![bug_id], |row| {
            Ok(BugFile {
                id: row.get(0)?,
                bug_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                diskfile: row.get(4)?,
                filename: row.get(5)?,
                folder: row.get(6)?,
                filesize: row.get(7)?,
                file_type: row.get(8)?,
                date_added: row.get(9)?,
                user_id: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    // ─── Sub-project 3: Tags, Custom Fields, Filters, Search & Monitors ──────

    pub fn create_tag(&self, user_id: i64, name: &str, description: &str) -> Result<i64, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Tag name cannot be empty".to_string());
        }
        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_tag_table (user_id, name, description, date_created, date_updated)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![user_id, trimmed, description, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_tag_by_id(&self, id: i64) -> rusqlite::Result<Option<Tag>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, user_id, name, description, date_created, date_updated
             FROM mantis_tag_table WHERE id = ?1",
            params![id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    date_created: row.get(4)?,
                    date_updated: row.get(5)?,
                })
            },
        ) {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_tag_by_name(&self, name: &str) -> rusqlite::Result<Option<Tag>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, user_id, name, description, date_created, date_updated
             FROM mantis_tag_table WHERE name = ?1",
            params![name.trim()],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    date_created: row.get(4)?,
                    date_updated: row.get(5)?,
                })
            },
        ) {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_tag(&self, id: i64, name: &str, description: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Tag name cannot be empty".to_string());
        }
        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let updated = conn
            .execute(
                "UPDATE mantis_tag_table SET name = ?1, description = ?2, date_updated = ?3 WHERE id = ?4",
                params![trimmed, description, now, id],
            )
            .map_err(|e| e.to_string())?;
        if updated == 0 {
            return Err(format!("Tag {} not found", id));
        }
        Ok(())
    }

    pub fn delete_tag(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM mantis_tag_table WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_tags(&self) -> rusqlite::Result<Vec<Tag>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, description, date_created, date_updated
             FROM mantis_tag_table
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                date_created: row.get(4)?,
                date_updated: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn attach_tag_to_bug(
        &self,
        bug_id: i64,
        tag_id: i64,
        user_id: i64,
    ) -> Result<(), String> {
        let conn = self.conn();
        if Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Bug {} not found", bug_id));
        }
        let tag_name: String = conn
            .query_row(
                "SELECT name FROM mantis_tag_table WHERE id = ?1",
                params![tag_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("Tag {} not found", tag_id))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO mantis_bug_tag_table (bug_id, tag_id, user_id, date_attached)
                 VALUES (?1, ?2, ?3, ?4)",
                params![bug_id, tag_id, user_id, now],
            )
            .map_err(|e| e.to_string())?;

        if inserted > 0 {
            let _ = self.record_history(&conn, bug_id, user_id, "tag_attached", "", &tag_name, 0);
        }
        Ok(())
    }

    pub fn detach_tag_from_bug(
        &self,
        bug_id: i64,
        tag_id: i64,
        user_id: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn();
        let tag_name: Option<String> = conn
            .query_row(
                "SELECT name FROM mantis_tag_table WHERE id = ?1",
                params![tag_id],
                |row| row.get(0),
            )
            .ok();

        let deleted = conn.execute(
            "DELETE FROM mantis_bug_tag_table WHERE bug_id = ?1 AND tag_id = ?2",
            params![bug_id, tag_id],
        )?;

        if deleted > 0 {
            if let Some(tname) = tag_name {
                let _ = self.record_history(&conn, bug_id, user_id, "tag_detached", &tname, "", 0);
            }
        }
        Ok(())
    }

    pub fn list_bug_tags(&self, bug_id: i64) -> rusqlite::Result<Vec<BugTag>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT bt.bug_id, bt.tag_id, bt.user_id, bt.date_attached, t.name
             FROM mantis_bug_tag_table bt
             JOIN mantis_tag_table t ON bt.tag_id = t.id
             WHERE bt.bug_id = ?1
             ORDER BY t.name ASC",
        )?;
        let rows = stmt.query_map(params![bug_id], |row| {
            Ok(BugTag {
                bug_id: row.get(0)?,
                tag_id: row.get(1)?,
                user_id: row.get(2)?,
                date_attached: row.get(3)?,
                tag_name: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    fn map_custom_field_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomField> {
        Ok(CustomField {
            id: row.get(0)?,
            name: row.get(1)?,
            field_type: row.get(2)?,
            possible_values: row.get(3)?,
            default_value: row.get(4)?,
            valid_regexp: row.get(5)?,
            access_level_r: row.get(6)?,
            access_level_rw: row.get(7)?,
            length_min: row.get(8)?,
            length_max: row.get(9)?,
            require_report: row.get(10)?,
            require_update: row.get(11)?,
            display_report: row.get(12)?,
            display_update: row.get(13)?,
            require_resolved: row.get(14)?,
            display_resolved: row.get(15)?,
            display_closed: row.get(16)?,
            require_closed: row.get(17)?,
            filter_by: row.get(18)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_field(
        &self,
        name: &str,
        field_type: i32,
        possible_values: &str,
        default_value: &str,
        valid_regexp: &str,
        length_min: i32,
        length_max: i32,
        require_report: bool,
        require_update: bool,
        display_report: bool,
        display_update: bool,
    ) -> Result<i64, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Custom field name cannot be empty".to_string());
        }
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_custom_field_table (
                name, type, possible_values, default_value, valid_regexp,
                length_min, length_max, require_report, require_update,
                display_report, display_update
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                trimmed,
                field_type,
                possible_values,
                default_value,
                valid_regexp,
                length_min,
                length_max,
                if require_report { 1 } else { 0 },
                if require_update { 1 } else { 0 },
                if display_report { 1 } else { 0 },
                if display_update { 1 } else { 0 },
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_custom_field(
        &self,
        id: i64,
        name: &str,
        field_type: i32,
        possible_values: &str,
        default_value: &str,
        valid_regexp: &str,
        length_min: i32,
        length_max: i32,
        require_report: bool,
        require_update: bool,
        display_report: bool,
        display_update: bool,
    ) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Custom field name cannot be empty".to_string());
        }
        let conn = self.conn();
        let rows = conn
            .execute(
                "UPDATE mantis_custom_field_table
                 SET name = ?1, type = ?2, possible_values = ?3, default_value = ?4,
                     valid_regexp = ?5, length_min = ?6, length_max = ?7,
                     require_report = ?8, require_update = ?9,
                     display_report = ?10, display_update = ?11
                 WHERE id = ?12",
                params![
                    trimmed,
                    field_type,
                    possible_values,
                    default_value,
                    valid_regexp,
                    length_min,
                    length_max,
                    if require_report { 1 } else { 0 },
                    if require_update { 1 } else { 0 },
                    if display_report { 1 } else { 0 },
                    if display_update { 1 } else { 0 },
                    id,
                ],
            )
            .map_err(|e| e.to_string())?;
        if rows == 0 {
            return Err(format!("Custom field {} not found", id));
        }
        Ok(())
    }

    pub fn delete_custom_field(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_custom_field_table WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn get_custom_field_by_id(&self, id: i64) -> rusqlite::Result<Option<CustomField>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, name, type, possible_values, default_value, valid_regexp,
                    access_level_r, access_level_rw, length_min, length_max,
                    require_report, require_update, display_report, display_update,
                    require_resolved, display_resolved, display_closed, require_closed, filter_by
             FROM mantis_custom_field_table WHERE id = ?1",
            params![id],
            Self::map_custom_field_row,
        ) {
            Ok(cf) => Ok(Some(cf)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_custom_fields(&self) -> rusqlite::Result<Vec<CustomField>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, type, possible_values, default_value, valid_regexp,
                    access_level_r, access_level_rw, length_min, length_max,
                    require_report, require_update, display_report, display_update,
                    require_resolved, display_resolved, display_closed, require_closed, filter_by
             FROM mantis_custom_field_table
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], Self::map_custom_field_row)?;
        rows.collect()
    }

    pub fn link_custom_field_to_project(
        &self,
        field_id: i64,
        project_id: i64,
        sequence: i32,
    ) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_custom_field_project_table (field_id, project_id, sequence)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (field_id, project_id) DO UPDATE SET sequence = excluded.sequence",
            params![field_id, project_id, sequence],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn unlink_custom_field_from_project(
        &self,
        field_id: i64,
        project_id: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_custom_field_project_table WHERE field_id = ?1 AND project_id = ?2",
            params![field_id, project_id],
        )?;
        Ok(())
    }

    pub fn list_projects_for_custom_field(
        &self,
        field_id: i64,
    ) -> rusqlite::Result<Vec<(Project, i32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.status, p.enabled, p.view_state, p.access_min,
                    p.description, p.category_id, p.inherit_global, cfp.sequence
             FROM mantis_custom_field_project_table cfp
             JOIN mantis_project_table p ON cfp.project_id = p.id
             WHERE cfp.field_id = ?1
             ORDER BY cfp.sequence ASC, p.name ASC",
        )?;
        let rows = stmt.query_map(params![field_id], |row| {
            Ok((
                Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    status: row.get(2)?,
                    enabled: row.get(3)?,
                    view_state: row.get(4)?,
                    access_min: row.get(5)?,
                    description: row.get(6)?,
                    category_id: row.get(7)?,
                    inherit_global: row.get(8)?,
                },
                row.get(9)?,
            ))
        })?;
        rows.collect()
    }

    pub fn list_custom_fields_for_project(
        &self,
        project_id: i64,
    ) -> rusqlite::Result<Vec<CustomField>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT cf.id, cf.name, cf.type, cf.possible_values, cf.default_value, cf.valid_regexp,
                    cf.access_level_r, cf.access_level_rw, cf.length_min, cf.length_max,
                    cf.require_report, cf.require_update, cf.display_report, cf.display_update,
                    cf.require_resolved, cf.display_resolved, cf.display_closed, cf.require_closed, cf.filter_by
             FROM mantis_custom_field_table cf
             JOIN mantis_custom_field_project_table cfp ON cf.id = cfp.field_id
             WHERE cfp.project_id = ?1
             ORDER BY cfp.sequence ASC, cf.name ASC",
        )?;
        let rows = stmt.query_map(params![project_id], Self::map_custom_field_row)?;
        rows.collect()
    }

    pub fn set_custom_field_value(
        &self,
        field_id: i64,
        bug_id: i64,
        user_id: i64,
        value: &str,
    ) -> Result<(), String> {
        let conn = self.conn();
        let cf: CustomField = conn
            .query_row(
                "SELECT id, name, type, possible_values, default_value, valid_regexp,
                        access_level_r, access_level_rw, length_min, length_max,
                        require_report, require_update, display_report, display_update,
                        require_resolved, display_resolved, display_closed, require_closed, filter_by
                 FROM mantis_custom_field_table WHERE id = ?1",
                params![field_id],
                Self::map_custom_field_row,
            )
            .map_err(|_| format!("Custom field {} not found", field_id))?;

        let len = value.chars().count() as i32;
        if cf.length_min > 0 && len < cf.length_min {
            return Err(format!(
                "Custom field '{}' requires at least {} characters",
                cf.name, cf.length_min
            ));
        }
        if cf.length_max > 0 && len > cf.length_max {
            return Err(format!(
                "Custom field '{}' allows at most {} characters",
                cf.name, cf.length_max
            ));
        }

        let old_val: String = conn
            .query_row(
                "SELECT value FROM mantis_custom_field_string_table WHERE field_id = ?1 AND bug_id = ?2",
                params![field_id, bug_id],
                |row| row.get(0),
            )
            .unwrap_or_default();

        conn.execute(
            "INSERT INTO mantis_custom_field_string_table (field_id, bug_id, value, text)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT (field_id, bug_id) DO UPDATE SET value = excluded.value, text = excluded.text",
            params![field_id, bug_id, value],
        )
        .map_err(|e| e.to_string())?;

        if old_val != value {
            let _ = self.record_history(&conn, bug_id, user_id, &cf.name, &old_val, value, 0);
        }

        Ok(())
    }

    pub fn get_custom_field_values_for_bug(
        &self,
        bug_id: i64,
    ) -> rusqlite::Result<Vec<CustomFieldValue>> {
        let conn = self.conn();
        let bug = match Self::query_bug_by_id(&conn, bug_id)? {
            Some(b) => b,
            None => return Ok(Vec::new()),
        };

        let mut stmt = conn.prepare(
            "SELECT cf.id, cf.name, cf.type, COALESCE(cfs.value, cf.default_value)
             FROM mantis_custom_field_table cf
             JOIN mantis_custom_field_project_table cfp ON cf.id = cfp.field_id
             LEFT JOIN mantis_custom_field_string_table cfs ON cf.id = cfs.field_id AND cfs.bug_id = ?1
             WHERE cfp.project_id = ?2
             ORDER BY cfp.sequence ASC, cf.name ASC",
        )?;
        let rows = stmt.query_map(params![bug_id, bug.project_id], |row| {
            Ok(CustomFieldValue {
                field_id: row.get(0)?,
                field_name: row.get(1)?,
                field_type: row.get(2)?,
                bug_id,
                value: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn save_filter(
        &self,
        user_id: i64,
        project_id: i64,
        is_public: bool,
        name: &str,
        filter: &BugFilter,
    ) -> Result<i64, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Filter name cannot be empty".to_string());
        }
        let json = serde_json::to_string(filter).map_err(|e| e.to_string())?;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_filters_table (user_id, project_id, is_public, name, filter_string)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                user_id,
                project_id,
                if is_public { 1 } else { 0 },
                trimmed,
                json
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_filter(&self, id: i64) -> rusqlite::Result<Option<(SavedFilter, BugFilter)>> {
        let conn = self.conn();
        let sf_opt = match conn.query_row(
            "SELECT id, user_id, project_id, is_public, name, filter_string
             FROM mantis_filters_table WHERE id = ?1",
            params![id],
            |row| {
                Ok(SavedFilter {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    project_id: row.get(2)?,
                    is_public: row.get(3)?,
                    name: row.get(4)?,
                    filter_string: row.get(5)?,
                })
            },
        ) {
            Ok(sf) => Some(sf),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        let sf = match sf_opt {
            Some(s) => s,
            None => return Ok(None),
        };
        let bf: BugFilter = serde_json::from_str(&sf.filter_string).unwrap_or_default();
        Ok(Some((sf, bf)))
    }

    pub fn list_visible_filters(
        &self,
        user_id: i64,
        project_id: i64,
    ) -> rusqlite::Result<Vec<SavedFilter>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, project_id, is_public, name, filter_string
             FROM mantis_filters_table
             WHERE (user_id = ?1 OR is_public = 1)
               AND (project_id = 0 OR project_id = ?2 OR ?2 = 0)
             ORDER BY name ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![user_id, project_id], |row| {
            Ok(SavedFilter {
                id: row.get(0)?,
                user_id: row.get(1)?,
                project_id: row.get(2)?,
                is_public: row.get(3)?,
                name: row.get(4)?,
                filter_string: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_filter(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM mantis_filters_table WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_bug_monitor(&self, bug_id: i64, user_id: i64) -> Result<(), String> {
        let conn = self.conn();
        if Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Bug {} not found", bug_id));
        }
        conn.execute(
            "INSERT OR IGNORE INTO mantis_bug_monitor_table (user_id, bug_id) VALUES (?1, ?2)",
            params![user_id, bug_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_bug_monitor(&self, bug_id: i64, user_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_bug_monitor_table WHERE user_id = ?1 AND bug_id = ?2",
            params![user_id, bug_id],
        )?;
        Ok(())
    }

    pub fn list_bug_monitors(&self, bug_id: i64) -> rusqlite::Result<Vec<User>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.username, u.realname, u.email, u.password, u.enabled, u.protected,
                    u.access_level, u.login_count, u.lost_password_request_count,
                    u.failed_login_count, u.cookie_string, u.last_visit, u.date_created
             FROM mantis_bug_monitor_table bm
             JOIN mantis_user_table u ON bm.user_id = u.id
             WHERE bm.bug_id = ?1
             ORDER BY u.username ASC",
        )?;
        let rows = stmt.query_map(params![bug_id], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                realname: row.get(2)?,
                email: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                protected: row.get(6)?,
                access_level: row.get(7)?,
                login_count: row.get(8)?,
                lost_password_request_count: row.get(9)?,
                failed_login_count: row.get(10)?,
                cookie_string: row.get(11)?,
                last_visit: row.get(12)?,
                date_created: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn filter_bugs(&self, filter: &BugFilter, user: &User) -> rusqlite::Result<Vec<Bug>> {
        let all_candidates = if let Some(pid) = filter.project_id {
            if pid > 0 {
                let eff = self.effective_access_level(user, pid)?;
                if eff < 10 {
                    return Ok(Vec::new());
                }
                self.list_bugs(pid, eff, 0, 50_000)?
            } else {
                self.list_bugs(0, 90, 0, 50_000)?
            }
        } else {
            self.list_bugs(0, 90, 0, 50_000)?
        };

        let search_kw = filter
            .search
            .as_ref()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());

        let mut results = Vec::new();
        for b in all_candidates {
            let eff = self.effective_access_level(user, b.project_id)?;
            if eff < 10 || (b.view_state == 50 && eff < 55) {
                continue;
            }
            if let Some(st) = filter.status {
                if st > 0 && b.status != st {
                    continue;
                }
            }
            if let Some(hide_st) = filter.hide_status {
                if hide_st > 0 && b.status >= hide_st {
                    continue;
                }
            }
            if let Some(pr) = filter.priority {
                if pr > 0 && b.priority != pr {
                    continue;
                }
            }
            if let Some(sev) = filter.severity {
                if sev > 0 && b.severity != sev {
                    continue;
                }
            }
            if let Some(cid) = filter.category_id {
                if cid > 0 && b.category_id != cid {
                    continue;
                }
            }
            if let Some(rid) = filter.reporter_id {
                if rid > 0 && b.reporter_id != rid {
                    continue;
                }
            }
            if let Some(hid) = filter.handler_id {
                if hid > 0 && b.handler_id != hid {
                    continue;
                } else if hid == -1 && b.handler_id != 0 {
                    // -1 = Unassigned
                    continue;
                }
            }
            if let Some(tid) = filter.tag_id {
                if tid > 0 {
                    let tags = self.list_bug_tags(b.id)?;
                    if !tags.iter().any(|t| t.tag_id == tid) {
                        continue;
                    }
                }
            }
            if let Some(mid) = filter.monitored_by {
                if mid > 0 {
                    let monitors = self.list_bug_monitors(b.id)?;
                    if !monitors.iter().any(|u| u.id == mid) {
                        continue;
                    }
                }
            }
            if let Some(ref kw) = search_kw {
                let mut matched = b.summary.to_ascii_lowercase().contains(kw)
                    || format!("{:07}", b.id).contains(kw);
                if !matched {
                    if let Some(full) = self.get_bug_full(b.id)? {
                        if full.description.to_ascii_lowercase().contains(kw)
                            || full.steps_to_reproduce.to_ascii_lowercase().contains(kw)
                            || full
                                .additional_information
                                .to_ascii_lowercase()
                                .contains(kw)
                        {
                            matched = true;
                        }
                    }
                }
                if !matched {
                    let notes = self.list_bugnotes(b.id, eff)?;
                    if notes
                        .iter()
                        .any(|n| n.note.to_ascii_lowercase().contains(kw))
                    {
                        matched = true;
                    }
                }
                if !matched {
                    continue;
                }
            }

            results.push(b);
        }

        Ok(results)
    }

    // ─── Sub-project 4: Email Queue, Time Tracking & Project News ────────────

    pub fn enqueue_email(
        &self,
        conn: &Connection,
        email: &str,
        subject: &str,
        body: &str,
    ) -> rusqlite::Result<i64> {
        let trimmed = email.trim();
        if trimmed.is_empty() {
            return Ok(0);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_email_table (email, subject, metadata, body, submitted)
             VALUES (?1, ?2, '', ?3, ?4)",
            params![trimmed, subject, body, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn notify_bug_event(
        &self,
        conn: &Connection,
        bug_id: i64,
        _actor_id: i64,
        event_subject: &str,
        event_body: &str,
    ) {
        let bug = match Self::query_bug_by_id(conn, bug_id) {
            Ok(Some(b)) => b,
            _ => return,
        };

        let mut recipients = HashSet::new();
        if let Ok(Some(reporter)) = Self::query_user_by_id(conn, bug.reporter_id) {
            if reporter.enabled != 0 && !reporter.email.trim().is_empty() {
                recipients.insert(reporter.email.trim().to_string());
            }
        }
        if bug.handler_id > 0 {
            if let Ok(Some(handler)) = Self::query_user_by_id(conn, bug.handler_id) {
                if handler.enabled != 0 && !handler.email.trim().is_empty() {
                    recipients.insert(handler.email.trim().to_string());
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare(
            "SELECT u.email
             FROM mantis_bug_monitor_table bm
             JOIN mantis_user_table u ON bm.user_id = u.id
             WHERE bm.bug_id = ?1 AND u.enabled != 0",
        ) {
            if let Ok(rows) = stmt.query_map(params![bug_id], |row| row.get::<_, String>(0)) {
                for email in rows.flatten() {
                    let trimmed = email.trim();
                    if !trimmed.is_empty() {
                        recipients.insert(trimmed.to_string());
                    }
                }
            }
        }

        let mut sorted: Vec<String> = recipients.into_iter().collect();
        sorted.sort();
        for addr in sorted {
            let _ = self.enqueue_email(conn, &addr, event_subject, event_body);
        }

        // Plugin hook: if EventAuditLog plugin is enabled, increment plugin_event_audit_count
        let audit_enabled: i32 = conn
            .query_row(
                "SELECT enabled FROM mantis_plugin_table WHERE basename = 'EventAuditLog'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if audit_enabled != 0 {
            let prev_count: i64 = conn
                .query_row(
                    "SELECT value FROM mantis_config_table WHERE config_id = 'plugin_event_audit_count' AND project_id = 0 AND user_id = 0",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let _ = conn.execute(
                "INSERT INTO mantis_config_table (config_id, project_id, user_id, access_reqd, type, value)
                 VALUES ('plugin_event_audit_count', 0, 0, 0, 90, ?1)
                 ON CONFLICT (config_id, project_id, user_id) DO UPDATE SET value = excluded.value",
                params![(prev_count + 1).to_string()],
            );
        }
    }

    pub fn list_queued_emails(&self) -> rusqlite::Result<Vec<EmailMessage>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT email_id, email, subject, metadata, body, submitted
             FROM mantis_email_table
             ORDER BY submitted DESC, email_id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(EmailMessage {
                email_id: row.get(0)?,
                email: row.get(1)?,
                subject: row.get(2)?,
                metadata: row.get(3)?,
                body: row.get(4)?,
                submitted: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_queued_email(&self, email_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_email_table WHERE email_id = ?1",
            params![email_id],
        )?;
        Ok(())
    }

    pub fn flush_email_queue(&self) -> rusqlite::Result<usize> {
        let conn = self.conn();
        let count = conn.execute("DELETE FROM mantis_email_table", [])?;
        Ok(count)
    }

    pub fn list_time_tracking(&self, project_id: i64) -> rusqlite::Result<Vec<TimeTrackingEntry>> {
        let conn = self.conn();
        if project_id > 0 {
            let mut stmt = conn.prepare(
                "SELECT n.bug_id, b.summary, n.reporter_id, COALESCE(u.username, 'unknown'),
                        n.time_tracking, n.date_submitted, COALESCE(t.note, '')
                 FROM mantis_bugnote_table n
                 JOIN mantis_bug_table b ON n.bug_id = b.id
                 LEFT JOIN mantis_user_table u ON n.reporter_id = u.id
                 LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
                 WHERE n.time_tracking > 0 AND b.project_id = ?1
                 ORDER BY n.date_submitted DESC, n.id DESC",
            )?;
            let rows = stmt.query_map(params![project_id], |row| {
                Ok(TimeTrackingEntry {
                    bug_id: row.get(0)?,
                    bug_summary: row.get(1)?,
                    user_id: row.get(2)?,
                    username: row.get(3)?,
                    minutes: row.get(4)?,
                    date_submitted: row.get(5)?,
                    note: row.get(6)?,
                })
            })?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT n.bug_id, b.summary, n.reporter_id, COALESCE(u.username, 'unknown'),
                        n.time_tracking, n.date_submitted, COALESCE(t.note, '')
                 FROM mantis_bugnote_table n
                 JOIN mantis_bug_table b ON n.bug_id = b.id
                 LEFT JOIN mantis_user_table u ON n.reporter_id = u.id
                 LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
                 WHERE n.time_tracking > 0
                 ORDER BY n.date_submitted DESC, n.id DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(TimeTrackingEntry {
                    bug_id: row.get(0)?,
                    bug_summary: row.get(1)?,
                    user_id: row.get(2)?,
                    username: row.get(3)?,
                    minutes: row.get(4)?,
                    date_submitted: row.get(5)?,
                    note: row.get(6)?,
                })
            })?;
            rows.collect()
        }
    }

    pub fn create_news(
        &self,
        project_id: i64,
        poster_id: i64,
        headline: &str,
        body: &str,
        announcement: bool,
        view_state: i32,
    ) -> Result<i64, String> {
        let trimmed_headline = headline.trim();
        if trimmed_headline.is_empty() {
            return Err("News headline cannot be empty".to_string());
        }
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            return Err("News body cannot be empty".to_string());
        }
        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_news_table (
                project_id, poster_id, date_posted, last_modified,
                view_state, announcement, headline, body
             ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7)",
            params![
                project_id,
                poster_id,
                now,
                view_state,
                if announcement { 1 } else { 0 },
                trimmed_headline,
                trimmed_body,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_news(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM mantis_news_table WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_news(&self, project_id: i64, user_access: i32) -> rusqlite::Result<Vec<NewsItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, poster_id, date_posted, last_modified,
                    view_state, announcement, headline, body
             FROM mantis_news_table
             WHERE (project_id = 0 OR project_id = ?1 OR ?1 = 0)
               AND (view_state = 10 OR ?2 >= 55)
             ORDER BY announcement DESC, date_posted DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![project_id, user_access], |row| {
            Ok(NewsItem {
                id: row.get(0)?,
                project_id: row.get(1)?,
                poster_id: row.get(2)?,
                date_posted: row.get(3)?,
                last_modified: row.get(4)?,
                view_state: row.get(5)?,
                announcement: row.get(6)?,
                headline: row.get(7)?,
                body: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    // ─── Sub-project 5: API Tokens (`mantis_api_token_table`) & Plugins ──────

    pub fn create_api_token(&self, user_id: i64, name: &str) -> Result<(i64, String), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Token name cannot be empty".to_string());
        }
        let raw_token = crate::auth::generate_api_token();
        let token_hash = crate::auth::hash_api_token(&raw_token);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_api_token_table (user_id, name, hash, date_created, date_used)
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![user_id, trimmed, token_hash, now],
        )
        .map_err(|e| e.to_string())?;
        Ok((conn.last_insert_rowid(), raw_token))
    }

    pub fn list_api_tokens(&self, user_id: i64) -> rusqlite::Result<Vec<ApiToken>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, hash, date_created, date_used
             FROM mantis_api_token_table
             WHERE user_id = ?1
             ORDER BY date_created DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(ApiToken {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                hash: row.get(3)?,
                date_created: row.get(4)?,
                date_used: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn revoke_api_token(&self, id: i64, user_id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "DELETE FROM mantis_api_token_table WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )?;
        Ok(rows > 0)
    }

    pub fn authenticate_api_token(&self, raw_token: &str) -> rusqlite::Result<Option<User>> {
        let trimmed = raw_token.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let token_hash = crate::auth::hash_api_token(trimmed);
        let conn = self.conn();
        let row_opt: Option<(i64, i64)> = match conn.query_row(
            "SELECT id, user_id FROM mantis_api_token_table WHERE hash = ?1",
            params![token_hash],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

        let (token_id, user_id) = match row_opt {
            Some(v) => v,
            None => return Ok(None),
        };

        let user = match Self::query_user_by_id(&conn, user_id)? {
            Some(u) if u.enabled != 0 => u,
            _ => return Ok(None),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let _ = conn.execute(
            "UPDATE mantis_api_token_table SET date_used = ?1 WHERE id = ?2",
            params![now, token_id],
        );

        Ok(Some(user))
    }

    pub fn list_plugins(&self) -> rusqlite::Result<Vec<PluginItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT basename, enabled, protected, priority
             FROM mantis_plugin_table
             ORDER BY protected DESC, priority ASC, basename ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PluginItem {
                basename: row.get(0)?,
                enabled: row.get(1)?,
                protected: row.get(2)?,
                priority: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn set_plugin_enabled(&self, basename: &str, enabled: bool) -> Result<(), String> {
        let conn = self.conn();
        let protected: i32 = conn
            .query_row(
                "SELECT protected FROM mantis_plugin_table WHERE basename = ?1",
                params![basename],
                |row| row.get(0),
            )
            .map_err(|_| format!("Plugin '{}' not found", basename))?;
        if protected != 0 && !enabled {
            return Err(format!("Protected plugin '{}' cannot be disabled", basename));
        }
        conn.execute(
            "UPDATE mantis_plugin_table SET enabled = ?1 WHERE basename = ?2",
            params![if enabled { 1 } else { 0 }, basename],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn is_plugin_enabled(&self, basename: &str) -> bool {
        let conn = self.conn();
        conn.query_row(
            "SELECT enabled FROM mantis_plugin_table WHERE basename = ?1",
            params![basename],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(0)
            != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_creates_tables_and_default_seed() {
        let db = Db::open_in_memory().unwrap();
        let admin = db
            .get_user_by_username("administrator")
            .unwrap()
            .expect("default admin");
        assert_eq!(admin.access_level, 90);
        assert_eq!(admin.enabled, 1);

        let cats = db.list_categories_for_project(0).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].name, "General");
        assert_eq!(cats[0].project_id, 0);

        db.set_config("window_title", 0, 0, "Rust MantisBT")
            .unwrap();
        assert_eq!(
            db.get_config("window_title", 0, 0).unwrap().as_deref(),
            Some("Rust MantisBT")
        );
    }

    #[test]
    fn test_user_lookup_helpers() {
        let db = Db::open_in_memory().unwrap();
        let admin_by_name = db.get_user_by_username("administrator").unwrap().unwrap();
        let admin_by_id = db.get_user_by_id(admin_by_name.id).unwrap().unwrap();
        assert_eq!(admin_by_name.id, admin_by_id.id);

        let admin_by_cookie = db
            .get_user_by_cookie(&admin_by_name.cookie_string)
            .unwrap()
            .unwrap();
        assert_eq!(admin_by_name.id, admin_by_cookie.id);

        assert!(db.get_user_by_username("nonexistent").unwrap().is_none());
        assert!(db.get_user_by_id(9999).unwrap().is_none());
        assert!(db.get_user_by_cookie("invalid_cookie").unwrap().is_none());
    }

    #[test]
    fn test_config_hierarchical_fallback() {
        let db = Db::open_in_memory().unwrap();

        // 1. Global config
        db.set_config("theme", 0, 0, "default").unwrap();
        assert_eq!(
            db.get_config("theme", 0, 0).unwrap().as_deref(),
            Some("default")
        );
        assert_eq!(
            db.get_config("theme", 10, 20).unwrap().as_deref(),
            Some("default")
        );

        // 2. Project config override
        db.set_config("theme", 10, 0, "project_blue").unwrap();
        assert_eq!(
            db.get_config("theme", 10, 20).unwrap().as_deref(),
            Some("project_blue")
        );
        assert_eq!(
            db.get_config("theme", 0, 0).unwrap().as_deref(),
            Some("default")
        );

        // 3. User config override
        db.set_config("theme", 10, 20, "user_dark").unwrap();
        assert_eq!(
            db.get_config("theme", 10, 20).unwrap().as_deref(),
            Some("user_dark")
        );

        // 4. Global user config override (0, user_id)
        db.set_config("language", 0, 20, "french").unwrap();
        assert_eq!(
            db.get_config("language", 10, 20).unwrap().as_deref(),
            Some("french")
        );
        assert_eq!(
            db.get_config("language", 10, 30).unwrap().as_deref(),
            None
        );
    }

    #[test]
    fn test_category_inheritance() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();

        // Create Project 1 (inherit_global = 1)
        conn.execute(
            "INSERT INTO mantis_project_table (id, name, inherit_global) VALUES (1, 'Project Alpha', 1)",
            [],
        ).unwrap();
        // Add project-specific category for Project 1
        conn.execute(
            "INSERT INTO mantis_category_table (project_id, user_id, name) VALUES (1, 0, 'Alpha Features')",
            [],
        ).unwrap();

        // Project 1 categories should include 'General' (global) and 'Alpha Features'
        drop(conn);
        let cats1 = db.list_categories_for_project(1).unwrap();
        let cat_names1: Vec<&str> = cats1.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(cat_names1, vec!["Alpha Features", "General"]);

        // Create Project 2 (child of Project 1, inherit_parent = 1)
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_project_table (id, name, inherit_global) VALUES (2, 'Project Alpha Sub', 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mantis_project_hierarchy_table (child_id, parent_id, inherit_parent) VALUES (2, 1, 1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mantis_category_table (project_id, user_id, name) VALUES (2, 0, 'Sub Category')",
            [],
        ).unwrap();

        drop(conn);
        let cats2 = db.list_categories_for_project(2).unwrap();
        let cat_names2: Vec<&str> = cats2.iter().map(|c| c.name.as_str()).collect();
        // Since project 2 has inherit_parent = 1, it should inherit Project 1's categories
        assert!(cat_names2.contains(&"Sub Category"));
        assert!(cat_names2.contains(&"Alpha Features"));
    }

    #[test]
    fn test_db_open_file_based() {
        let tmp_dir = std::env::temp_dir();
        let db_path = tmp_dir.join(format!("mantis_test_{}.db", rand::random::<u64>()));
        let db_str = db_path.to_str().unwrap();

        {
            let db = Db::open(db_str).unwrap();
            let admin = db
                .get_user_by_username("administrator")
                .unwrap()
                .expect("admin exists");
            assert_eq!(admin.access_level, 90);
        }

        // Reopen existing db
        {
            let db = Db::open(db_str).unwrap();
            let admin = db
                .get_user_by_username("administrator")
                .unwrap()
                .expect("admin exists");
            assert_eq!(admin.access_level, 90);
        }

        let _ = std::fs::remove_file(db_path);
    }

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

        // Disabled project returns 0 even with explicit override
        db.update_project(priv_id, "PrivateProj", 10, false, 50, "desc", true).unwrap();
        assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 0);
        assert_eq!(db.effective_access_level(&bob, priv_id).unwrap(), 0);
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

    #[test]
    fn test_project_crud_and_validation() {
        let db = Db::open_in_memory().unwrap();

        // Empty project name fails
        assert!(db.create_project("  ", 10, 10, "desc", true).is_err());

        // Create project successfully
        let pid = db.create_project("Project Alpha", 10, 10, "First project", true).unwrap();
        assert!(pid > 0);

        // Duplicate name fails
        assert!(db.create_project("Project Alpha", 10, 10, "Duplicate", true).is_err());

        // Get by ID
        let proj = db.get_project_by_id(pid).unwrap().unwrap();
        assert_eq!(proj.id, pid);
        assert_eq!(proj.name, "Project Alpha");
        assert_eq!(proj.status, 10);
        assert_eq!(proj.enabled, 1);
        assert_eq!(proj.view_state, 10);
        assert_eq!(proj.description, "First project");
        assert_eq!(proj.inherit_global, 1);

        // Update project
        assert!(db.update_project(pid, "Project Alpha Updated", 30, false, 50, "Updated desc", false).is_ok());
        let updated = db.get_project_by_id(pid).unwrap().unwrap();
        assert_eq!(updated.name, "Project Alpha Updated");
        assert_eq!(updated.status, 30);
        assert_eq!(updated.enabled, 0);
        assert_eq!(updated.view_state, 50);
        assert_eq!(updated.description, "Updated desc");
        assert_eq!(updated.inherit_global, 0);

        // Update nonexistent project fails
        assert!(db.update_project(9999, "Nonexistent", 10, true, 10, "", true).is_err());
        // Update with empty name fails
        assert!(db.update_project(pid, "", 10, true, 10, "", true).is_err());

        // Add a project-specific category and verify delete_project cleans it up
        let p_cat = db.add_category(pid, "Project-Specific Cat", 0).unwrap();
        assert!(db.get_category_by_id(p_cat).unwrap().is_some());

        // Delete project
        assert!(db.delete_project(pid).is_ok());
        assert!(db.get_project_by_id(pid).unwrap().is_none());
        assert!(db.delete_project(pid).is_err());

        // Verify project-specific category was deleted
        assert!(db.get_category_by_id(p_cat).unwrap().is_none());
        // Verify global category General (project_id = 0) is still intact
        let global_cats = db.list_effective_categories(0).unwrap();
        assert!(global_cats.iter().any(|c| c.name == "General"));
    }

    #[test]
    fn test_subproject_hierarchy_multi_level_cycle_prevention() {
        let db = Db::open_in_memory().unwrap();
        let p1 = db.create_project("A", 10, 10, "", true).unwrap();
        let p2 = db.create_project("B", 10, 10, "", true).unwrap();
        let p3 = db.create_project("C", 10, 10, "", true).unwrap();

        // A -> B -> C
        db.add_subproject(p1, p2, true).unwrap();
        db.add_subproject(p2, p3, true).unwrap();

        // Adding C -> A would form A -> B -> C -> A cycle, must fail
        assert!(db.add_subproject(p3, p1, true).is_err());

        // Adding nonexistent project fails
        assert!(db.add_subproject(p1, 9999, true).is_err());
        assert!(db.add_subproject(9999, p1, true).is_err());

        // Remove subproject
        assert!(db.remove_subproject(p2, p3).is_ok());
        // After removing B -> C, C -> A is now allowed!
        assert!(db.add_subproject(p3, p1, true).is_ok());
    }

    #[test]
    fn test_project_user_management_and_overrides() {
        let db = Db::open_in_memory().unwrap();
        let pid = db.create_project("SecuredProj", 10, 50, "", true).unwrap();
        let uid = db.create_user("charlie", "Charlie", "c@x.com", "pw", 10, true, false).unwrap();
        let user = db.get_user_by_id(uid).unwrap().unwrap();

        // Initially Viewer (10) has no access to private project
        assert_eq!(db.effective_access_level(&user, pid).unwrap(), 0);

        // Assign Developer (55)
        assert!(db.set_project_user(pid, uid, 55).is_ok());
        assert_eq!(db.effective_access_level(&user, pid).unwrap(), 55);

        // Update to Manager (70)
        assert!(db.set_project_user(pid, uid, 70).is_ok());
        assert_eq!(db.effective_access_level(&user, pid).unwrap(), 70);

        // Remove user from project
        assert!(db.remove_project_user(pid, uid).is_ok());
        assert_eq!(db.effective_access_level(&user, pid).unwrap(), 0);

        // Nonexistent project or user fails
        assert!(db.set_project_user(9999, uid, 55).is_err());
        assert!(db.set_project_user(pid, 9999, 55).is_err());
    }

    #[test]
    fn test_effective_access_level_rules() {
        let db = Db::open_in_memory().unwrap();
        let pub_id = db.create_project("Pub", 10, 10, "", true).unwrap();
        let priv_id = db.create_project("Priv", 10, 50, "", true).unwrap();
        let disabled_id = db.create_project("Disabled", 10, 10, "", true).unwrap();
        db.update_project(disabled_id, "Disabled", 10, false, 10, "", true).unwrap();

        let admin = db.get_user_by_username("administrator").unwrap().unwrap();
        let dev_id = db.create_user("dev", "Dev", "d@x.com", "pw", 55, true, false).unwrap();
        let dev = db.get_user_by_id(dev_id).unwrap().unwrap();
        let mgr_id = db.create_user("mgr", "Mgr", "m@x.com", "pw", 70, true, false).unwrap();
        let mgr = db.get_user_by_id(mgr_id).unwrap().unwrap();

        // Rule 1: project_id == 0 returns user.access_level
        assert_eq!(db.effective_access_level(&admin, 0).unwrap(), 90);
        assert_eq!(db.effective_access_level(&dev, 0).unwrap(), 55);
        assert_eq!(db.effective_access_level(&mgr, 0).unwrap(), 70);

        // Rule 2: Administrator always gets 90
        assert_eq!(db.effective_access_level(&admin, pub_id).unwrap(), 90);
        assert_eq!(db.effective_access_level(&admin, priv_id).unwrap(), 90);
        assert_eq!(db.effective_access_level(&admin, disabled_id).unwrap(), 90);

        // Disabled project returns 0 for non-administrator
        assert_eq!(db.effective_access_level(&dev, disabled_id).unwrap(), 0);
        assert_eq!(db.effective_access_level(&mgr, disabled_id).unwrap(), 0);

        // Nonexistent project returns 0 for non-administrator
        assert_eq!(db.effective_access_level(&dev, 99999).unwrap(), 0);

        // auth module wrapper
        assert_eq!(crate::auth::effective_access_level(&db, &dev, pub_id).unwrap(), 55);
    }

    #[test]
    fn test_list_visible_projects_and_depths() {
        let db = Db::open_in_memory().unwrap();
        // Create tree:
        // P_Parent (Public)
        //   -> P_Child (Public)
        //        -> P_Grandchild (Public)
        // P_Private (Private)
        //   -> P_PrivateSub (Public)
        let p_parent = db.create_project("Proj Parent", 10, 10, "", true).unwrap();
        let p_child = db.create_project("Proj Child", 10, 10, "", true).unwrap();
        let p_grandchild = db.create_project("Proj Grandchild", 10, 10, "", true).unwrap();
        db.add_subproject(p_parent, p_child, true).unwrap();
        db.add_subproject(p_child, p_grandchild, true).unwrap();

        let p_priv = db.create_project("Proj Priv", 10, 50, "", true).unwrap();
        let p_priv_sub = db.create_project("Proj Priv Sub", 10, 10, "", true).unwrap();
        db.add_subproject(p_priv, p_priv_sub, true).unwrap();

        let rep_id = db.create_user("rep", "Rep", "r@x.com", "pw", 25, true, false).unwrap();
        let rep = db.get_user_by_id(rep_id).unwrap().unwrap();

        let visible = db.list_visible_projects(&rep).unwrap();
        let vis_tuples: Vec<(&str, usize)> = visible.iter().map(|(p, d)| (p.name.as_str(), *d)).collect();

        // P_Parent (0) -> P_Child (1) -> P_Grandchild (2)
        // P_Priv is hidden from rep, so P_Priv_Sub becomes root (depth 0)
        assert!(vis_tuples.contains(&("Proj Parent", 0)));
        assert!(vis_tuples.contains(&("Proj Child", 1)));
        assert!(vis_tuples.contains(&("Proj Grandchild", 2)));
        assert!(vis_tuples.contains(&("Proj Priv Sub", 0)));
        assert!(!vis_tuples.iter().any(|(n, _)| *n == "Proj Priv"));

        // Admin sees all, including P_Priv at depth 0 and P_Priv_Sub at depth 1
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();
        let admin_visible = db.list_visible_projects(&admin).unwrap();
        let admin_tuples: Vec<(&str, usize)> = admin_visible.iter().map(|(p, d)| (p.name.as_str(), *d)).collect();
        assert!(admin_tuples.contains(&("Proj Priv", 0)));
        assert!(admin_tuples.contains(&("Proj Priv Sub", 1)));
    }

    #[test]
    fn test_category_crud_operations() {
        let db = Db::open_in_memory().unwrap();
        let pid = db.create_project("CatProj", 10, 10, "", true).unwrap();

        // Empty category name fails
        assert!(db.add_category(pid, "  ", 0).is_err());

        // Add category
        let cid = db.add_category(pid, "Database", 0).unwrap();
        assert!(cid > 0);

        // Duplicate category in same project fails
        assert!(db.add_category(pid, "Database", 0).is_err());

        // Update category
        assert!(db.update_category(cid, "Database Engine", 0).is_ok());
        // Update nonexistent category fails
        assert!(db.update_category(9999, "None", 0).is_err());

        // Delete category
        assert!(db.delete_category(cid).is_ok());
        // Delete nonexistent category fails
        assert!(db.delete_category(cid).is_err());
    }

    #[test]
    fn test_version_crud_and_ordering() {
        let db = Db::open_in_memory().unwrap();
        let pid = db.create_project("VerProj", 10, 10, "", true).unwrap();

        // Empty version fails
        assert!(db.add_version(pid, "   ", "desc", true, false).is_err());

        // Add versions
        let v1 = db.add_version(pid, "1.0.0", "First release", true, false).unwrap();
        let v2 = db.add_version(pid, "2.0.0", "Major update", false, false).unwrap();
        assert!(v1 > 0);
        assert!(v2 > 0);

        // Duplicate version in same project fails
        assert!(db.add_version(pid, "1.0.0", "Dup", true, false).is_err());

        // List versions
        let versions = db.list_versions(pid).unwrap();
        assert_eq!(versions.len(), 2);
        let ver_names: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert!(ver_names.contains(&"1.0.0"));
        assert!(ver_names.contains(&"2.0.0"));

        // Update version
        assert!(db.update_version(v2, "2.0.0-rc1", "Release candidate", true, false).is_ok());
        let updated_versions = db.list_versions(pid).unwrap();
        assert!(updated_versions.iter().any(|v| v.version == "2.0.0-rc1"));

        // Update nonexistent fails
        assert!(db.update_version(9999, "none", "", true, false).is_err());

        // Delete version
        assert!(db.delete_version(v1).is_ok());
        assert_eq!(db.list_versions(pid).unwrap().len(), 1);
        // Delete nonexistent fails
        assert!(db.delete_version(v1).is_err());
    }

    #[test]
    fn test_bug_schema_bootstrap_and_crud() {
        let db = Db::open_in_memory().unwrap();
        let proj_id = db.create_project("TestProj", 10, 10, "", true).unwrap();
        let uid = db
            .create_user("alice", "Alice", "a@x.com", "pw", 25, true, false)
            .unwrap();

        let bug_id = db
            .create_bug(
                proj_id,
                uid,
                1,
                "Test summary",
                "Description here",
                "Steps",
                "Additional",
                30,
                50,
                70,
                "1.0",
                "",
                "",
                "",
                10,
            )
            .unwrap();
        assert!(bug_id > 0);

        let bug = db.get_bug(bug_id).unwrap().expect("bug exists");
        assert_eq!(bug.summary, "Test summary");
        assert_eq!(bug.status, 10); // new
        assert_eq!(bug.priority, 30); // normal
        assert_eq!(bug.severity, 50); // minor

        let full = db.get_bug_full(bug_id).unwrap().expect("full bug");
        assert_eq!(full.description, "Description here");
        assert_eq!(full.steps_to_reproduce, "Steps");

        let bugs = db.list_bugs(proj_id, 10, 0, 25).unwrap();
        assert_eq!(bugs.len(), 1);

        // Private bug filtering at SQL level
        let priv_bug_id = db
            .create_bug(
                proj_id,
                uid,
                1,
                "Secret bug",
                "Private",
                "",
                "",
                30,
                50,
                70,
                "",
                "",
                "",
                "",
                50,
            )
            .unwrap();
        assert!(priv_bug_id > 0);
        // Viewer (10) only sees public bug
        assert_eq!(db.list_bugs(proj_id, 10, 0, 25).unwrap().len(), 1);
        // Developer (55) sees both
        assert_eq!(db.list_bugs(proj_id, 55, 0, 25).unwrap().len(), 2);

        // Update bug fields & verify history only records changed fields
        let changed = db
            .update_bug_fields(
                bug_id,
                uid,
                BugUpdate {
                    summary: Some("Updated summary".to_string()),
                    priority: Some(30), // unchanged!
                    severity: Some(60), // changed 50 -> 60
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(changed.len(), 2);
        assert!(changed.iter().any(|h| h.field_name == "summary"
            && h.old_value == "Test summary"
            && h.new_value == "Updated summary"));
        assert!(changed
            .iter()
            .any(|h| h.field_name == "severity" && h.old_value == "50" && h.new_value == "60"));

        db.delete_bug(bug_id).unwrap();
        assert!(db.get_bug(bug_id).unwrap().is_none());
    }

    #[test]
    fn test_bug_status_workflow_and_history() {
        let db = Db::open_in_memory().unwrap();
        let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
        let uid = db
            .create_user("dev", "Dev", "d@x.com", "pw", 55, true, false)
            .unwrap();
        let bug_id = db
            .create_bug(
                proj_id, uid, 1, "Summary", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
            )
            .unwrap();

        // new -> resolved (with note)
        db.change_bug_status(bug_id, uid, 80, 20, Some("Fixed it"))
            .unwrap();
        let bug = db.get_bug(bug_id).unwrap().unwrap();
        assert_eq!(bug.status, 80);
        assert_eq!(bug.resolution, 20);

        let history = db.list_bug_history(bug_id).unwrap();
        let status_change = history.iter().find(|h| h.field_name == "status").unwrap();
        assert_eq!(status_change.old_value, "10");
        assert_eq!(status_change.new_value, "80");

        let notes = db.list_bugnotes(bug_id, 55).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].note, "Fixed it");

        // Private note filtering
        db.add_bugnote(bug_id, uid, "Private internal note", 50)
            .unwrap();
        assert_eq!(db.list_bugnotes(bug_id, 25).unwrap().len(), 1);
        assert_eq!(db.list_bugnotes(bug_id, 55).unwrap().len(), 2);

        // closed -> cannot go to resolved directly
        db.change_bug_status(bug_id, uid, 90, 20, None).unwrap();
        assert!(db.change_bug_status(bug_id, uid, 80, 20, None).is_err());
        // closed -> feedback (20) is allowed
        assert!(db.change_bug_status(bug_id, uid, 20, 30, None).is_ok());
    }

    #[test]
    fn test_bug_relationships_and_duplicate_reverse() {
        let db = Db::open_in_memory().unwrap();
        let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
        let uid = db
            .create_user("dev", "Dev", "d@x.com", "pw", 55, true, false)
            .unwrap();
        let b1 = db
            .create_bug(
                proj_id, uid, 1, "Bug 1", "", "", "", 30, 50, 70, "", "", "", "", 10,
            )
            .unwrap();
        let b2 = db
            .create_bug(
                proj_id, uid, 1, "Bug 2", "", "", "", 30, 50, 70, "", "", "", "", 10,
            )
            .unwrap();

        // add duplicate_of relationship -> reverse must be auto-created
        db.add_bug_relationship(b2, b1, 1).unwrap(); // b2 is duplicate_of b1
        let rels = db.list_bug_relationships(b1).unwrap();
        assert!(rels
            .iter()
            .any(|r| r.relationship_type == 2 && r.destination_bug_id == b2)); // b1 has_duplicate b2
        assert_eq!(db.get_bug(b2).unwrap().unwrap().duplicate_id, b1);

        // Deleting one side deletes both sides
        let rel_on_b1 = rels[0].id;
        db.delete_bug_relationship(rel_on_b1).unwrap();
        assert!(db.list_bug_relationships(b1).unwrap().is_empty());
        assert!(db.list_bug_relationships(b2).unwrap().is_empty());
        assert_eq!(db.get_bug(b2).unwrap().unwrap().duplicate_id, 0);

        // add related_to -> no reverse auto-created
        db.add_bug_relationship(b1, b2, 0).unwrap();
        let rels2 = db.list_bug_relationships(b2).unwrap();
        assert!(!rels2
            .iter()
            .any(|r| r.source_bug_id == b2 && r.relationship_type == 0));
    }

    #[test]
    fn test_bug_file_attachment_inline_and_cascade() {
        let db = Db::open_in_memory().unwrap();
        let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
        let uid = db
            .create_user("rep", "Rep", "r@x.com", "pw", 25, true, false)
            .unwrap();
        let bug_id = db
            .create_bug(
                proj_id, uid, 1, "Bug", "", "", "", 30, 50, 70, "", "", "", "", 10,
            )
            .unwrap();

        let content = b"hello file".to_vec();
        let fid = db
            .add_bug_file(bug_id, uid, "test.txt", "text/plain", content.clone(), "")
            .unwrap();
        assert!(fid > 0);

        let files = db.list_bug_files(bug_id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "test.txt");
        assert_eq!(files[0].filesize, 10);

        let (_, fetched) = db.get_bug_file(fid, "").unwrap().unwrap();
        assert_eq!(fetched, content);

        db.delete_bug_file(fid, "").unwrap();
        assert!(db.list_bug_files(bug_id).unwrap().is_empty());

        // Cascade: deleting the bug removes all attachments
        let fid2 = db
            .add_bug_file(bug_id, uid, "test2.txt", "text/plain", b"data".to_vec(), "")
            .unwrap();
        assert!(fid2 > 0);
        db.delete_bug(bug_id).unwrap();
        assert!(db.get_bug_file(fid2, "").unwrap().is_none());
    }

    #[test]
    fn test_tags_custom_fields_filters_search_and_monitors() {
        let db = Db::open_in_memory().unwrap();
        let proj_id = db.create_project("Proj3", 10, 10, "", true).unwrap();
        let uid = db
            .create_user("dev3", "Dev 3", "d3@x.com", "pw", 55, true, false)
            .unwrap();
        let user = db.get_user_by_id(uid).unwrap().unwrap();

        let b1 = db
            .create_bug(
                proj_id,
                uid,
                1,
                "Memory leak in parser",
                "Heap grows unbounded",
                "",
                "",
                40,
                60,
                10,
                "1.0",
                "",
                "",
                "",
                10,
            )
            .unwrap();
        let b2 = db
            .create_bug(
                proj_id,
                uid,
                1,
                "UI button alignment",
                "CSS flex issue",
                "",
                "",
                20,
                20,
                10,
                "1.0",
                "",
                "",
                "",
                10,
            )
            .unwrap();

        // 1. Tags
        let tag_id = db.create_tag(uid, "memory", "Memory related issues").unwrap();
        db.attach_tag_to_bug(b1, tag_id, uid).unwrap();
        let b1_tags = db.list_bug_tags(b1).unwrap();
        assert_eq!(b1_tags.len(), 1);
        assert_eq!(b1_tags[0].tag_name, "memory");

        // 2. Custom Fields
        let cf_id = db
            .create_custom_field(
                "Environment URL",
                0,
                "",
                "https://staging.local",
                "",
                3,
                100,
                false,
                false,
                true,
                true,
            )
            .unwrap();
        db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

        // Default value returned before explicit set
        let cf_vals = db.get_custom_field_values_for_bug(b1).unwrap();
        assert_eq!(cf_vals.len(), 1);
        assert_eq!(cf_vals[0].value, "https://staging.local");

        // Min length validation
        assert!(db.set_custom_field_value(cf_id, b1, uid, "ab").is_err());
        db.set_custom_field_value(cf_id, b1, uid, "https://prod.example.com")
            .unwrap();
        let cf_vals2 = db.get_custom_field_values_for_bug(b1).unwrap();
        assert_eq!(cf_vals2[0].value, "https://prod.example.com");

        // History recorded for custom field change
        let hist = db.list_bug_history(b1).unwrap();
        assert!(hist
            .iter()
            .any(|h| h.field_name == "Environment URL" && h.new_value == "https://prod.example.com"));

        // 3. Monitors
        db.add_bug_monitor(b2, uid).unwrap();
        assert_eq!(db.list_bug_monitors(b2).unwrap().len(), 1);

        // 4. Full-text Search & Filtering
        let by_kw = db
            .filter_bugs(
                &BugFilter {
                    project_id: Some(proj_id),
                    search: Some("unbounded".to_string()),
                    ..Default::default()
                },
                &user,
            )
            .unwrap();
        assert_eq!(by_kw.len(), 1);
        assert_eq!(by_kw[0].id, b1);

        let by_tag = db
            .filter_bugs(
                &BugFilter {
                    project_id: Some(proj_id),
                    tag_id: Some(tag_id),
                    ..Default::default()
                },
                &user,
            )
            .unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, b1);

        let by_monitor = db
            .filter_bugs(
                &BugFilter {
                    project_id: Some(proj_id),
                    monitored_by: Some(uid),
                    ..Default::default()
                },
                &user,
            )
            .unwrap();
        assert_eq!(by_monitor.len(), 1);
        assert_eq!(by_monitor[0].id, b2);

        // 5. Saved Filters
        let filter_id = db
            .save_filter(
                uid,
                proj_id,
                true,
                "High Priority Memory",
                &BugFilter {
                    project_id: Some(proj_id),
                    tag_id: Some(tag_id),
                    ..Default::default()
                },
            )
            .unwrap();
        let visible = db.list_visible_filters(uid, proj_id).unwrap();
        assert_eq!(visible.len(), 1);
        let (sf, bf) = db.get_filter(filter_id).unwrap().unwrap();
        assert_eq!(sf.name, "High Priority Memory");
        assert_eq!(bf.tag_id, Some(tag_id));
    }

    #[test]
    fn test_email_queue_time_tracking_and_news() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();
        let dev_id = db
            .create_user("dev1", "Dev One", "dev1@example.com", "pw", 55, true, false)
            .unwrap();
        let watcher_id = db
            .create_user(
                "watcher",
                "Watcher",
                "watcher@example.com",
                "pw",
                25,
                true,
                false,
            )
            .unwrap();
        let proj_id = db
            .create_project("Core", 10, 10, "desc", true)
            .unwrap();

        // 1. Creating bug queues email to reporter (admin: root@localhost)
        let bug_id = db
            .create_bug(
                proj_id,
                admin.id,
                1,
                "Null pointer in parser",
                "Crash on empty token",
                "",
                "",
                30,
                50,
                10,
                "1.0",
                "",
                "",
                "",
                10,
            )
            .unwrap();
        let emails = db.list_queued_emails().unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].email, "root@localhost");
        assert!(emails[0].subject.contains("Null pointer in parser"));

        // Flush queue
        assert_eq!(db.flush_email_queue().unwrap(), 1);
        assert!(db.list_queued_emails().unwrap().is_empty());

        // 2. Add monitor + assign to dev1 -> queues emails to reporter, handler, and monitor
        db.add_bug_monitor(bug_id, watcher_id).unwrap();
        db.assign_bug(bug_id, admin.id, dev_id).unwrap();
        let emails2 = db.list_queued_emails().unwrap();
        let addrs: Vec<&str> = emails2.iter().map(|e| e.email.as_str()).collect();
        assert!(addrs.contains(&"root@localhost"));
        assert!(addrs.contains(&"dev1@example.com"));
        assert!(addrs.contains(&"watcher@example.com"));

        // 3. Time tracking via bugnote
        db.add_bugnote_with_time(bug_id, dev_id, "Investigated and fixed", 10, 95)
            .unwrap();
        let entries = db.list_time_tracking(proj_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].minutes, 95);
        assert_eq!(entries[0].username, "dev1");

        // History recorded for time_tracking
        let hist = db.list_bug_history(bug_id).unwrap();
        assert!(hist
            .iter()
            .any(|h| h.field_name == "time_tracking" && h.new_value == "95"));

        // 4. Project News & visibility
        let n_pub = db
            .create_news(
                proj_id,
                admin.id,
                "v1.0 Released",
                "Release notes here",
                true,
                10,
            )
            .unwrap();
        let _n_priv = db
            .create_news(
                proj_id,
                admin.id,
                "Internal Security Audit",
                "Secret body",
                false,
                50,
            )
            .unwrap();

        // Viewer (10) only sees public news
        let viewer_news = db.list_news(proj_id, 10).unwrap();
        assert_eq!(viewer_news.len(), 1);
        assert_eq!(viewer_news[0].id, n_pub);

        // Developer (55) sees both public and private news
        let dev_news = db.list_news(proj_id, 55).unwrap();
        assert_eq!(dev_news.len(), 2);
    }

    #[test]
    fn test_api_tokens_and_plugin_system() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();

        // 1. Create and authenticate API token
        let (tok_id, raw_tok) = db.create_api_token(admin.id, "CI Pipeline").unwrap();
        assert_eq!(raw_tok.len(), 64);

        let tokens = db.list_api_tokens(admin.id).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "CI Pipeline");
        assert_eq!(tokens[0].date_used, 0);
        // Raw token is never stored in DB
        assert_ne!(tokens[0].hash, raw_tok);

        let authed = db.authenticate_api_token(&raw_tok).unwrap().unwrap();
        assert_eq!(authed.id, admin.id);
        assert!(db.list_api_tokens(admin.id).unwrap()[0].date_used > 0);

        // Revoke token -> subsequent auth fails
        assert!(db.revoke_api_token(tok_id, admin.id).unwrap());
        assert!(db.authenticate_api_token(&raw_tok).unwrap().is_none());

        // 2. Plugin system & protected plugin guard
        let plugins = db.list_plugins().unwrap();
        assert_eq!(plugins.len(), 2);
        assert!(db.is_plugin_enabled("MantisCoreFormatting"));
        assert!(db.is_plugin_enabled("EventAuditLog"));

        // Protected plugin cannot be disabled
        assert!(db.set_plugin_enabled("MantisCoreFormatting", false).is_err());

        // EventAuditLog increments plugin_event_audit_count when enabled
        let proj_id = db.create_project("PluginProj", 10, 10, "", true).unwrap();
        let bug_id = db
            .create_bug(
                proj_id,
                admin.id,
                1,
                "Test plugin event",
                "desc",
                "",
                "",
                30,
                50,
                10,
                "",
                "",
                "",
                "",
                10,
            )
            .unwrap();
        assert_eq!(
            db.get_config("plugin_event_audit_count", 0, 0)
                .unwrap()
                .as_deref(),
            Some("1")
        );

        // Disable EventAuditLog -> subsequent events do not increment counter
        db.set_plugin_enabled("EventAuditLog", false).unwrap();
        assert!(!db.is_plugin_enabled("EventAuditLog"));
        db.add_bugnote(bug_id, admin.id, "Note while plugin disabled", 10)
            .unwrap();
        assert_eq!(
            db.get_config("plugin_event_audit_count", 0, 0)
                .unwrap()
                .as_deref(),
            Some("1")
        );
    }
}


