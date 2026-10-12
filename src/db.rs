use crate::models::{Category, User};
use rusqlite::{params, Connection};
use std::collections::HashSet;
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

        // Collect project IDs including project_id, global if inherited, and parent project ancestors
        let mut project_ids = vec![project_id];
        let mut visited = HashSet::new();
        visited.insert(project_id);

        let inherit_global: Option<i32> = conn
            .query_row(
                "SELECT inherit_global FROM mantis_project_table WHERE id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .ok();

        if inherit_global.unwrap_or(1) == 1 {
            project_ids.push(0);
        }

        // Traverse hierarchy for parents if inherit_parent = 1
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
                    }
                }
            }
            current_ids = next_ids;
        }

        // Query categories for collected project IDs
        let placeholders = project_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, project_id, user_id, name, status
             FROM mantis_category_table
             WHERE project_id IN ({})
             ORDER BY name ASC",
            placeholders
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_vec: Vec<&dyn rusqlite::ToSql> = project_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(params_vec.as_slice(), |row| {
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
}
