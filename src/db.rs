use crate::models::{Category, Project, ProjectVersion, User};
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

        if let Some(val) = query(project_id, user_id)? {
            return Ok(Some(val));
        }

        if user_id != 0 {
            if let Some(val) = query(project_id, 0)? {
                return Ok(Some(val));
            }
        }

        if project_id != 0 {
            if let Some(val) = query(0, 0)? {
                return Ok(Some(val));
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

        // 3. Project-specific membership override
        let mut stmt = conn.prepare(
            "SELECT access_level FROM mantis_project_user_list_table WHERE project_id = ?1 AND user_id = ?2",
        )?;
        let mut rows = stmt.query(params![project_id, user.id])?;
        if let Some(row) = rows.next()? {
            return row.get(0);
        }

        // 4. View state & project status
        let mut stmt = conn.prepare(
            "SELECT enabled, view_state FROM mantis_project_table WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![project_id])?;
        if let Some(row) = rows.next()? {
            let enabled: i32 = row.get(0)?;
            let view_state: i32 = row.get(1)?;
            if enabled == 0 {
                return Ok(0);
            }
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

        // Delete project
        assert!(db.delete_project(pid).is_ok());
        assert!(db.get_project_by_id(pid).unwrap().is_none());
        assert!(db.delete_project(pid).is_err());
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
}
