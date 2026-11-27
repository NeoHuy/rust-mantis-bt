use crate::models::{
    ApiToken, Bug, BugFile, BugFilter, BugFull, BugHistory, BugPriority, BugRelationship,
    BugReproducibility, BugResolution, BugRevision, BugSeverity, BugStatus, BugTag, BugUpdate,
    Bugnote, Category, CustomField, CustomFieldValue, EmailMessage, JiraLink, JiraResolvedConfig,
    MyViewConfig, NewsItem, PluginItem, Project, ProjectVersion, RevisionType, SavedFilter, Tag,
    TimeTrackingEntry, TimelineEvent, User, MY_VIEW_CONFIG_ID,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    allow_local_jira: Arc<std::sync::atomic::AtomicBool>,
    base_url: Arc<String>,
}

struct DiskFileCleanupGuard(Vec<std::path::PathBuf>);

impl DiskFileCleanupGuard {
    fn dismiss(mut self) {
        self.0.clear();
    }
}

impl Drop for DiskFileCleanupGuard {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BugCreateExtras<'a> {
    pub build: &'a str,
    pub target_version: &'a str,
    pub due_date: i64,
    pub handler_id: Option<i64>,
}

impl Db {
    pub(crate) fn from_connection(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            allow_local_jira: Arc::new(std::sync::atomic::AtomicBool::new(
                crate::jira::env_allow_local_jira(),
            )),
            base_url: Arc::new(Self::resolve_base_url()),
        }
    }

    pub fn allow_local_jira(&self) -> bool {
        self.allow_local_jira
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_allow_local_jira(&self, allow: bool) {
        self.allow_local_jira
            .store(allow, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db lock poisoned")
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn resolve_base_url() -> String {
        let addr_str =
            std::env::var("MANTIS_ADDR").unwrap_or_else(|_| crate::DEFAULT_MANTIS_ADDR.to_string());
        std::env::var("MANTIS_BASE_URL").unwrap_or_else(|_| format!("http://{}", addr_str))
    }

    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        let db = Self::from_connection(conn);
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        let db = Self::from_connection(conn);
        db.migrate()?;
        Ok(db)
    }

    pub fn from_conn(conn: Connection) -> Self {
        Self::from_connection(conn)
    }

    pub fn can_view_bug(user_id: i64, user_access: i32, bug: &Bug) -> bool {
        if user_access < 10 {
            return false;
        }
        if bug.view_state == 50 {
            user_access >= 55
                || user_id == bug.reporter_id
                || (bug.handler_id > 0 && user_id == bug.handler_id)
        } else {
            true
        }
    }

    pub fn migrate(&self) -> rusqlite::Result<()> {
        let mut conn = self.conn();
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
            );

            CREATE TABLE IF NOT EXISTS mantis_jira_links_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bug_id INTEGER NOT NULL,
                jira_key TEXT NOT NULL,
                jira_url TEXT NOT NULL,
                issue_type TEXT NOT NULL DEFAULT 'Bug',
                jira_status TEXT NOT NULL DEFAULT '',
                created_by INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(bug_id) REFERENCES mantis_bug_table(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_jira_links_key ON mantis_jira_links_table(jira_key);

            CREATE TABLE IF NOT EXISTS mantis_jira_epics_cache_table (
                project_key TEXT PRIMARY KEY,
                epics_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS mantis_jira_status_mapping_table (
                jira_status TEXT PRIMARY KEY COLLATE NOCASE,
                mantis_status INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS mantis_bug_revision_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bug_id INTEGER NOT NULL,
                bugnote_id INTEGER NOT NULL DEFAULT 0,
                user_id INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                type INTEGER NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_bug_revision_bug_id ON mantis_bug_revision_table (bug_id);
            CREATE INDEX IF NOT EXISTS idx_bug_revision_bugnote_id ON mantis_bug_revision_table (bugnote_id);
            CREATE INDEX IF NOT EXISTS idx_bug_revision_bug_note ON mantis_bug_revision_table (bug_id, bugnote_id);",
        )?;

        // Idempotent migration for mantis_jira_epics_cache_table: ensure expires_at column exists
        let has_expires_at = {
            let mut stmt = conn.prepare("PRAGMA table_info(mantis_jira_epics_cache_table)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let mut found = false;
            for name in rows.flatten() {
                if name == "expires_at" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_expires_at {
            conn.execute(
                "ALTER TABLE mantis_jira_epics_cache_table ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        // Idempotent migration for mantis_jira_links_table: ensure unique index on bug_id exists
        let has_unique_bug_id_idx: bool = {
            let mut stmt = conn.prepare("PRAGMA index_list(mantis_jira_links_table)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                let unique: i64 = row.get(2)?;
                Ok((name, unique))
            })?;
            let mut found = false;
            for (name, unique) in rows.flatten() {
                if (name == "idx_jira_links_bug_id_unique"
                    || name.starts_with("sqlite_autoindex_mantis_jira_links_table"))
                    && unique == 1
                {
                    found = true;
                    break;
                }
            }
            found
        };

        if !has_unique_bug_id_idx {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM mantis_jira_links_table WHERE id NOT IN (SELECT MAX(id) FROM mantis_jira_links_table GROUP BY bug_id)",
                [],
            )?;
            tx.execute("DROP INDEX IF EXISTS idx_jira_links_bug_id", [])?;
            tx.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_jira_links_bug_id_unique ON mantis_jira_links_table(bug_id)",
                [],
            )?;
            tx.commit()?;
        }

        // Idempotent migration for mantis_jira_status_mapping_table: ensure COLLATE NOCASE
        let needs_nocase_migration = {
            let table_sql: Option<String> = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='mantis_jira_status_mapping_table'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            match table_sql {
                Some(sql) => !sql.to_uppercase().contains("NOCASE"),
                None => false,
            }
        };

        if needs_nocase_migration {
            let tx = conn.transaction()?;
            tx.execute(
                "CREATE TABLE IF NOT EXISTS mantis_jira_status_mapping_table_mig (
                    jira_status TEXT PRIMARY KEY COLLATE NOCASE,
                    mantis_status INTEGER NOT NULL
                )",
                [],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO mantis_jira_status_mapping_table_mig (jira_status, mantis_status)
                 SELECT TRIM(jira_status), mantis_status FROM mantis_jira_status_mapping_table",
                [],
            )?;
            tx.execute("DROP TABLE mantis_jira_status_mapping_table", [])?;
            tx.execute(
                "ALTER TABLE mantis_jira_status_mapping_table_mig RENAME TO mantis_jira_status_mapping_table",
                [],
            )?;
            tx.commit()?;
        }

        // Seed default Jira status mappings if table is empty
        let status_map_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mantis_jira_status_mapping_table",
            [],
            |row| row.get(0),
        )?;
        if status_map_count == 0 {
            let defaults = [
                ("To Do", 30),
                ("Open", 30),
                ("In Progress", 50),
                ("In Review", 20),
                ("Done", 80),
                ("Closed", 90),
            ];
            for (j_status, m_status) in defaults {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO mantis_jira_status_mapping_table (jira_status, mantis_status) VALUES (?1, ?2)",
                    params![j_status, m_status],
                );
            }
        }

        // Ensure jira_status column exists in mantis_jira_links_table for existing databases
        let has_jira_status = conn
            .prepare("SELECT jira_status FROM mantis_jira_links_table LIMIT 1")
            .is_ok();
        if !has_jira_status {
            let _ = conn.execute(
                "ALTER TABLE mantis_jira_links_table ADD COLUMN jira_status TEXT NOT NULL DEFAULT ''",
                [],
            );
        }

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
                    now, // last_visit
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
            Ok(Some(Self::map_user_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_user_by_id(&self, id: i64) -> rusqlite::Result<Option<User>> {
        let conn = self.conn();
        Self::query_user_by_id(&conn, id)
    }

    pub fn get_user_by_email(&self, email: &str) -> rusqlite::Result<Option<User>> {
        let trimmed = email.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, username, realname, email, password, enabled, protected, access_level,
                    login_count, lost_password_request_count, failed_login_count, cookie_string,
                    last_visit, date_created
             FROM mantis_user_table
             WHERE LOWER(email) = LOWER(?1)",
        )?;
        let mut rows = stmt.query(params![trimmed])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::map_user_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn is_signup_allowed(&self) -> bool {
        match self.get_config("allow_signup", 0, 0) {
            Ok(Some(val)) => {
                let v = val.trim().to_ascii_lowercase();
                !(v == "0" || v == "false" || v == "off" || v == "no")
            }
            _ => true,
        }
    }

    pub fn signup_user(
        &self,
        username: &str,
        realname: &str,
        email: &str,
        password: &str,
    ) -> Result<User, String> {
        if !self.is_signup_allowed() {
            return Err("User registration is currently disabled.".to_string());
        }

        let trimmed_username = username.trim();
        if trimmed_username.is_empty() {
            return Err("Username cannot be empty.".to_string());
        }
        if trimmed_username.len() > 32 {
            return Err("Username cannot exceed 32 characters.".to_string());
        }
        if !trimmed_username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-')
        {
            return Err("Username contains invalid characters. Only letters, numbers, '.', '_' and '-' are allowed.".to_string());
        }

        // Check if username already exists (case-insensitive)
        let conn = self.conn();
        let username_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM mantis_user_table WHERE LOWER(username) = LOWER(?1)",
                params![trimmed_username],
                |row| row.get::<_, i64>(0),
            )
            .map(|cnt| cnt > 0)
            .unwrap_or(false);
        drop(conn);

        if username_exists {
            return Err("Username is already taken.".to_string());
        }

        let trimmed_email = email.trim();
        if trimmed_email.is_empty() {
            return Err("Email cannot be empty.".to_string());
        }
        if !trimmed_email.contains('@') || !trimmed_email.contains('.') {
            return Err("Invalid email address format.".to_string());
        }

        // Check if email already exists
        if let Ok(Some(_)) = self.get_user_by_email(trimmed_email) {
            return Err("Email address is already in use.".to_string());
        }

        if password.len() < 6 {
            return Err("Password must be at least 6 characters long.".to_string());
        }

        let default_access_level = self
            .get_config("default_new_account_access_level", 0, 0)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(25); // Reporter = 25

        let user_id = self.create_user(
            trimmed_username,
            realname.trim(),
            trimmed_email,
            password,
            default_access_level,
            true,
            false,
        )?;

        self.get_user_by_id(user_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Failed to retrieve newly created user.".to_string())
    }

    fn map_user_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
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
            Ok(Some(Self::map_user_row(row)?))
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
            Ok(Some(Self::map_user_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn rotate_user_cookie(&self, user_id: i64) -> Result<String, String> {
        let new_cookie = crate::auth::generate_cookie_string();
        let conn = self.conn();
        conn.execute(
            "UPDATE mantis_user_table SET cookie_string = ?1 WHERE id = ?2",
            params![new_cookie, user_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(new_cookie)
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Result<Option<User>, String> {
        let user = match self
            .get_user_by_username(username)
            .map_err(|e| e.to_string())?
        {
            Some(u) => u,
            None => {
                crate::auth::verify_password(password, &crate::auth::dummy_password_hash());
                return Ok(None);
            }
        };

        if user.enabled == 0 {
            crate::auth::verify_password(password, &crate::auth::dummy_password_hash());
            return Ok(None);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let max_failed_logins = std::env::var("MANTIS_MAX_FAILED_LOGINS")
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(|v| v.max(1))
            .unwrap_or(5);
        let lockout_secs = std::env::var("MANTIS_LOCKOUT_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .map(|v| v.max(1))
            .unwrap_or(900);

        let locked_until = self
            .get_config("login_locked_until", 0, user.id)
            .ok()
            .flatten()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0);
        if locked_until > now {
            crate::auth::verify_password(password, &crate::auth::dummy_password_hash());
            return Err(
                "Account temporarily locked due to too many failed login attempts. Please try again later."
                    .to_string(),
            );
        } else if locked_until > 0 {
            // Lockout period has elapsed: clear lock config and reset failure counter
            let _ = self.set_config("login_locked_until", 0, user.id, "0");
            let conn = self.conn();
            conn.execute(
                "UPDATE mantis_user_table SET failed_login_count = 0 WHERE id = ?1",
                params![user.id],
            )
            .map_err(|e| e.to_string())?;
        }

        let is_pbkdf2 = user.password.starts_with("pbkdf2_sha256$");
        let pw_matches = crate::auth::verify_password(password, &user.password);
        if !is_pbkdf2 {
            // Equalize verification timing with PBKDF2 calculation for non-PBKDF2 legacy hashes
            crate::auth::verify_password(password, &crate::auth::dummy_password_hash());
        }

        if !pw_matches {
            let conn = self.conn();
            let new_count: i32 = conn
                .query_row(
                    "UPDATE mantis_user_table SET failed_login_count = failed_login_count + 1 WHERE id = ?1 RETURNING failed_login_count",
                    params![user.id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            drop(conn);

            if new_count >= max_failed_logins {
                self.set_config(
                    "login_locked_until",
                    0,
                    user.id,
                    &(now + lockout_secs).to_string(),
                )
                .map_err(|e| e.to_string())?;
                // Reset failed_login_count to 0 so when the lockout period expires,
                // the user gets a fresh set of attempts instead of locking on a single wrong attempt
                let conn = self.conn();
                conn.execute(
                    "UPDATE mantis_user_table SET failed_login_count = 0 WHERE id = ?1",
                    params![user.id],
                )
                .map_err(|e| e.to_string())?;
            }
            return Ok(None);
        }

        let new_cookie = crate::auth::generate_cookie_string();
        let rehashed = if crate::auth::needs_rehash(&user.password) {
            Some(crate::auth::hash_password(password))
        } else {
            None
        };

        {
            let conn = self.conn();
            let rows = if let Some(ref new_pw_hash) = rehashed {
                conn.execute(
                    "UPDATE mantis_user_table
                     SET login_count = login_count + 1, failed_login_count = 0, last_visit = ?1, cookie_string = ?2, password = ?3
                     WHERE id = ?4 AND password = ?5",
                    params![now, new_cookie, new_pw_hash, user.id, user.password],
                )
                .map_err(|e| e.to_string())?
            } else {
                conn.execute(
                    "UPDATE mantis_user_table
                     SET login_count = login_count + 1, failed_login_count = 0, last_visit = ?1, cookie_string = ?2
                     WHERE id = ?3 AND password = ?4",
                    params![now, new_cookie, user.id, user.password],
                )
                .map_err(|e| e.to_string())?
            };

            if rows == 0 {
                // Password was updated concurrently by an admin, do not issue session for old password
                return Ok(None);
            }
        }

        let _ = self.set_config("login_locked_until", 0, user.id, "0");

        let mut updated = user;
        updated.login_count += 1;
        updated.failed_login_count = 0;
        updated.last_visit = now;
        updated.cookie_string = new_cookie;
        if let Some(new_pw_hash) = rehashed {
            updated.password = new_pw_hash;
        }
        Ok(Some(updated))
    }

    pub fn list_users(
        &self,
        search: Option<&str>,
        enabled_only: bool,
    ) -> rusqlite::Result<Vec<User>> {
        self.list_users_filtered(search, enabled_only, None)
    }

    pub fn list_users_filtered(
        &self,
        search: Option<&str>,
        enabled_only: bool,
        prefix: Option<&str>,
    ) -> rusqlite::Result<Vec<User>> {
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

        if let Some(p) = prefix {
            let trimmed = p.trim();
            if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("all") {
                if trimmed == "0-9" {
                    conditions.push("substr(username, 1, 1) BETWEEN '0' AND '9'".to_string());
                } else if let Some(ch) = trimmed.chars().next() {
                    if ch.is_ascii_alphanumeric() {
                        let pattern = format!("{}%", ch);
                        conditions.push("username LIKE ? ESCAPE '\\'".to_string());
                        params_vec.push(Box::new(pattern));
                    }
                }
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY username ASC");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_user_row)?;
        rows.collect()
    }

    pub fn list_assignable_users_for_project(
        &self,
        project_id: i64,
        min_access: i32,
    ) -> rusqlite::Result<Vec<User>> {
        let conn = self.conn();
        if project_id <= 0 {
            let mut stmt = conn.prepare(
                "SELECT id, username, realname, email, password, enabled, protected, access_level,
                        login_count, lost_password_request_count, failed_login_count, cookie_string,
                        last_visit, date_created
                 FROM mantis_user_table
                 WHERE enabled = 1 AND access_level >= ?1
                 ORDER BY username ASC",
            )?;
            let rows = stmt.query_map(params![min_access], Self::map_user_row)?;
            return rows.collect();
        }

        let proj_row = conn
            .query_row(
                "SELECT enabled, view_state, access_min FROM mantis_project_table WHERE id = ?1",
                params![project_id],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                    ))
                },
            )
            .ok();
        let Some((enabled, view_state, access_min)) = proj_row else {
            return Ok(Vec::new());
        };

        let mut stmt = conn.prepare(
            "SELECT u.id, u.username, u.realname, u.email, u.password, u.enabled, u.protected, u.access_level,
                    u.login_count, u.lost_password_request_count, u.failed_login_count, u.cookie_string,
                    u.last_visit, u.date_created
             FROM mantis_user_table u
             LEFT JOIN mantis_project_user_list_table pul
               ON pul.user_id = u.id AND pul.project_id = ?1
             WHERE u.enabled = 1
               AND (
                   CASE
                       WHEN u.access_level >= 90 THEN 90
                       WHEN ?5 = 0 THEN 0
                       WHEN pul.access_level IS NOT NULL THEN pul.access_level
                       WHEN ?2 = 10 AND u.access_level >= ?3 THEN u.access_level
                       WHEN ?2 = 50 AND u.access_level >= 70 THEN u.access_level
                       ELSE 0
                   END
               ) >= ?4
             ORDER BY u.username ASC",
        )?;
        let rows = stmt.query_map(
            params![project_id, view_state, access_min, min_access, enabled],
            Self::map_user_row,
        )?;
        rows.collect()
    }

    pub fn list_project_user_access_map(
        &self,
        project_id: i64,
    ) -> rusqlite::Result<std::collections::HashMap<i64, i32>> {
        let conn = self.conn();
        let mut map = std::collections::HashMap::new();
        if project_id <= 0 {
            let mut stmt = conn.prepare("SELECT id, access_level FROM mantis_user_table")?;
            let rows =
                stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)))?;
            for r in rows {
                let (uid, lvl) = r?;
                map.insert(uid, lvl);
            }
            return Ok(map);
        }

        let proj_row = conn
            .query_row(
                "SELECT enabled, view_state, access_min FROM mantis_project_table WHERE id = ?1",
                params![project_id],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                    ))
                },
            )
            .ok();
        let Some((enabled, view_state, access_min)) = proj_row else {
            return Ok(map);
        };

        let mut stmt = conn.prepare(
            "SELECT u.id,
                    CASE
                        WHEN u.access_level >= 90 THEN 90
                        WHEN ?4 = 0 THEN 0
                        WHEN pul.access_level IS NOT NULL THEN pul.access_level
                        WHEN ?2 = 10 AND u.access_level >= ?3 THEN u.access_level
                        WHEN ?2 = 50 AND u.access_level >= 70 THEN u.access_level
                        ELSE 0
                    END AS eff_level
             FROM mantis_user_table u
             LEFT JOIN mantis_project_user_list_table pul
               ON pul.user_id = u.id AND pul.project_id = ?1",
        )?;
        let rows = stmt.query_map(
            params![project_id, view_state, access_min, enabled],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)),
        )?;
        for r in rows {
            let (uid, lvl) = r?;
            map.insert(uid, lvl);
        }
        Ok(map)
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0, ?8, ?9, ?10)",
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
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    pub fn unlock_user(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE mantis_user_table SET failed_login_count = 0 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        drop(conn);
        let _ = self.set_config("login_locked_until", 0, id, "0");
        Ok(())
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
        let hashed_pw_and_cookie = new_password.map(|pw| {
            (
                crate::auth::hash_password(pw),
                crate::auth::generate_cookie_string(),
            )
        });

        let conn = self.conn();
        let existing = match Self::query_user_by_id(&conn, id).map_err(|e| e.to_string())? {
            Some(u) => u,
            None => return Err(format!("User {} not found", id)),
        };

        // Guard: Protected user cannot have password or access_level changed
        if existing.protected == 1
            && (new_password.is_some() || access_level != existing.access_level)
        {
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

        if let Some((hashed_pw, new_cookie)) = hashed_pw_and_cookie {
            conn.execute(
                "UPDATE mantis_user_table
                 SET realname = ?1, email = ?2, password = ?3, access_level = ?4, enabled = ?5, protected = ?6, cookie_string = ?7, failed_login_count = 0
                 WHERE id = ?8",
                params![
                    realname,
                    email,
                    hashed_pw,
                    access_level,
                    enabled_val,
                    protected_val,
                    new_cookie,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            drop(conn);
            let _ = self.set_config("login_locked_until", 0, id, "0");
        } else {
            conn.execute(
                "UPDATE mantis_user_table
                 SET realname = ?1, email = ?2, access_level = ?3, enabled = ?4, protected = ?5, failed_login_count = 0
                 WHERE id = ?6",
                params![
                    realname,
                    email,
                    access_level,
                    enabled_val,
                    protected_val,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            drop(conn);
            let _ = self.set_config("login_locked_until", 0, id, "0");
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

        let candidates = [(project_id, user_id), (0, user_id), (project_id, 0), (0, 0)];

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

    pub fn delete_config(
        &self,
        config_id: &str,
        project_id: i64,
        user_id: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_config_table WHERE config_id = ?1 AND project_id = ?2 AND user_id = ?3",
            params![config_id, project_id, user_id],
        )?;
        Ok(())
    }

    pub fn get_my_view_config(&self, user_id: i64) -> MyViewConfig {
        if let Ok(Some(raw)) = self.get_config(MY_VIEW_CONFIG_ID, 0, user_id) {
            if let Ok(parsed) = serde_json::from_str::<MyViewConfig>(&raw) {
                return parsed.normalize();
            }
        }
        MyViewConfig::default()
    }

    pub fn set_my_view_config(&self, user_id: i64, cfg: &MyViewConfig) -> rusqlite::Result<()> {
        let normalized = cfg.clone().normalize();
        let json = serde_json::to_string(&normalized).unwrap_or_else(|_| "{}".to_string());
        self.set_config(MY_VIEW_CONFIG_ID, 0, user_id, &json)
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

    pub fn set_project_access_min(&self, project_id: i64, access_min: i32) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE mantis_project_table SET access_min = ?1 WHERE id = ?2",
            params![access_min, project_id],
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

        conn.execute(
            "DELETE FROM mantis_category_table WHERE project_id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "DELETE FROM mantis_project_table WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn add_subproject(
        &self,
        parent_id: i64,
        child_id: i64,
        inherit_parent: bool,
    ) -> Result<(), String> {
        let conn = self.conn();

        if parent_id == child_id {
            return Err("Cyclic project hierarchy detected".to_string());
        }

        if Self::query_project_by_id(&conn, parent_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Project {} not found", parent_id));
        }
        if Self::query_project_by_id(&conn, child_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
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

    /// Returns all ancestor project IDs of `project_id` (excluding `project_id` itself).
    pub fn list_project_ancestor_ids(&self, project_id: i64) -> rusqlite::Result<HashSet<i64>> {
        let conn = self.conn();
        let mut ancestors = HashSet::new();
        let mut queue = vec![project_id];
        let mut visited = HashSet::new();
        visited.insert(project_id);

        while let Some(curr) = queue.pop() {
            let mut stmt = conn.prepare(
                "SELECT parent_id FROM mantis_project_hierarchy_table WHERE child_id = ?1",
            )?;
            let parents = stmt.query_map(params![curr], |row| row.get::<_, i64>(0))?;
            for p in parents {
                let parent_id = p?;
                ancestors.insert(parent_id);
                if visited.insert(parent_id) {
                    queue.push(parent_id);
                }
            }
        }
        Ok(ancestors)
    }

    pub fn get_site_overview_stats(&self) -> rusqlite::Result<crate::models::SiteOverviewStats> {
        let conn = self.conn();
        let projects_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_project_table", [], |r| {
                r.get(0)
            })?;
        let users_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_user_table", [], |r| r.get(0))?;
        let issues_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_bug_table", [], |r| r.get(0))?;
        let bugnotes_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_bugnote_table", [], |r| {
                r.get(0)
            })?;
        let attachments_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_bug_file_table", [], |r| {
                r.get(0)
            })?;
        let plugins_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_plugin_table", [], |r| r.get(0))?;
        let pending_emails_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM mantis_email_table", [], |r| r.get(0))?;

        Ok(crate::models::SiteOverviewStats {
            projects_count,
            users_count,
            issues_count,
            bugnotes_count,
            attachments_count,
            plugins_count,
            pending_emails_count,
        })
    }

    pub fn set_project_user(
        &self,
        project_id: i64,
        user_id: i64,
        access_level: i32,
    ) -> Result<(), String> {
        let conn = self.conn();
        if Self::query_project_by_id(&conn, project_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Project {} not found", project_id));
        }
        if Self::query_user_by_id(&conn, user_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
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

    pub fn list_project_descendant_ids(&self, project_id: i64) -> rusqlite::Result<Vec<i64>> {
        let conn = self.conn();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut results = Vec::new();

        visited.insert(project_id);
        queue.push_back(project_id);
        results.push(project_id);

        let mut stmt = conn
            .prepare("SELECT child_id FROM mantis_project_hierarchy_table WHERE parent_id = ?1")?;

        while let Some(curr) = queue.pop_front() {
            let children = stmt.query_map(params![curr], |row| row.get::<_, i64>(0))?;
            for child in children {
                let cid = child?;
                if visited.insert(cid) {
                    results.push(cid);
                    queue.push_back(cid);
                }
            }
        }

        Ok(results)
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
            let user = Self::map_user_row(row)?;
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
        let conn = self.conn();
        Self::query_effective_access_level(&conn, user, project_id)
    }

    pub(crate) fn query_effective_access_level(
        conn: &Connection,
        user: &User,
        project_id: i64,
    ) -> rusqlite::Result<i32> {
        // 1. All Projects context
        if project_id == 0 {
            return Ok(user.access_level);
        }

        // 2. Administrator
        if user.access_level >= 90 {
            return Ok(90);
        }

        // 3. Project status & enabled check
        let mut stmt = conn.prepare(
            "SELECT enabled, view_state, access_min FROM mantis_project_table WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![project_id])?;
        let (enabled, view_state, access_min) = match rows.next()? {
            Some(row) => (
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ),
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
            if user.access_level < access_min {
                return Ok(0);
            }
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
            let mut stmt =
                conn.prepare("SELECT child_id, parent_id FROM mantis_project_hierarchy_table")?;
            let rows =
                stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
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
                    dfs(
                        child_id,
                        depth + 1,
                        visible_map,
                        children_map,
                        visited,
                        result,
                    );
                }
            }
        }

        let mut visited = HashSet::new();
        let mut result = Vec::new();

        for root_id in roots {
            dfs(
                root_id,
                0,
                &visible_map,
                &children_map,
                &mut visited,
                &mut result,
            );
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
            dfs(
                id,
                0,
                &visible_map,
                &children_map,
                &mut visited,
                &mut result,
            );
        }

        Ok(result)
    }

    pub fn add_category(&self, project_id: i64, name: &str, user_id: i64) -> Result<i64, String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        let conn = self.conn();
        if project_id != 0
            && Self::query_project_by_id(&conn, project_id)
                .map_err(|e| e.to_string())?
                .is_none()
        {
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

        conn.execute(
            "DELETE FROM mantis_category_table WHERE id = ?1",
            params![id],
        )
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

    pub fn list_all_categories(&self) -> rusqlite::Result<Vec<(i64, String)>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT id, name FROM mantis_category_table ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub(crate) fn query_effective_categories(
        conn: &Connection,
        project_id: i64,
    ) -> rusqlite::Result<Vec<Category>> {
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

    pub fn list_effective_categories(&self, project_id: i64) -> rusqlite::Result<Vec<Category>> {
        Self::query_effective_categories(&self.conn(), project_id)
    }

    pub fn add_version(
        &self,
        project_id: i64,
        version: &str,
        description: &str,
        released: bool,
        obsolete: bool,
    ) -> Result<i64, String> {
        self.add_version_with_order(project_id, version, description, released, obsolete, None)
    }

    pub fn add_version_with_order(
        &self,
        project_id: i64,
        version: &str,
        description: &str,
        released: bool,
        obsolete: bool,
        date_order: Option<i64>,
    ) -> Result<i64, String> {
        let trimmed_version = version.trim();
        if trimmed_version.is_empty() {
            return Err("Version cannot be empty".to_string());
        }

        let conn = self.conn();
        if Self::query_project_by_id(&conn, project_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Project {} not found", project_id));
        }

        let released_val = if released { 1 } else { 0 };
        let obsolete_val = if obsolete { 1 } else { 0 };
        let order_val = date_order.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

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
                order_val,
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
        self.update_version_with_order(id, version, description, released, obsolete, None)
    }

    pub fn update_version_with_order(
        &self,
        id: i64,
        version: &str,
        description: &str,
        released: bool,
        obsolete: bool,
        date_order: Option<i64>,
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
             SET version = ?1, description = ?2, released = ?3, obsolete = ?4,
                 date_order = COALESCE(?5, date_order)
             WHERE id = ?6",
            params![
                trimmed_version,
                description,
                released_val,
                obsolete_val,
                date_order,
                id
            ],
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

        conn.execute(
            "DELETE FROM mantis_project_version_table WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub(crate) fn query_versions(
        conn: &Connection,
        project_id: i64,
    ) -> rusqlite::Result<Vec<ProjectVersion>> {
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

    pub fn list_versions(&self, project_id: i64) -> rusqlite::Result<Vec<ProjectVersion>> {
        Self::query_versions(&self.conn(), project_id)
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
        self.create_bug_with_mentions(
            project_id,
            reporter_id,
            category_id,
            summary,
            description,
            steps_to_reproduce,
            additional_information,
            priority,
            severity,
            reproducibility,
            version,
            os,
            os_build,
            platform,
            view_state,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_bug_with_mentions(
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
        mentioned_usernames: &[String],
    ) -> Result<i64, String> {
        self.create_bug_full_with_mentions(
            project_id,
            reporter_id,
            category_id,
            summary,
            description,
            steps_to_reproduce,
            additional_information,
            priority,
            severity,
            reproducibility,
            version,
            os,
            os_build,
            platform,
            view_state,
            mentioned_usernames,
            BugCreateExtras::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_bug_full_with_mentions(
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
        mentioned_usernames: &[String],
        extras: BugCreateExtras<'_>,
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

        let mut initial_handler_id = 0i64;
        let mut initial_status = 10i32;

        if let Some(hid) = extras.handler_id.filter(|&h| h > 0) {
            let u = Self::query_user_by_id(&conn, hid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Assigned user {} not found", hid))?;
            if u.enabled != 1 {
                return Err("Assigned user is disabled".to_string());
            }
            let eff = Self::query_effective_access_level(&conn, &u, project_id).unwrap_or(0);
            if eff < 55 {
                return Err("Assigned user lacks Developer access".to_string());
            }
            initial_handler_id = hid;
            initial_status = 50;
        } else if category_id > 0 {
            let cat_user_id: Option<i64> = conn
                .query_row(
                    "SELECT user_id FROM mantis_category_table WHERE id = ?1",
                    params![category_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(uid) = cat_user_id {
                if uid > 0 {
                    if let Ok(Some(u)) = Self::query_user_by_id(&conn, uid) {
                        if u.enabled == 1 {
                            let eff = Self::query_effective_access_level(&conn, &u, project_id)
                                .unwrap_or(0);
                            if eff >= 55 {
                                initial_handler_id = uid;
                                initial_status = 50;
                            }
                        }
                    }
                }
            }
        }

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO mantis_bug_table (
                project_id, reporter_id, handler_id, duplicate_id,
                priority, severity, reproducibility, status, resolution,
                category_id, date_submitted, last_updated, os, os_build,
                platform, version, fixed_in_version, target_version, build,
                summary, view_state, sticky, due_date
            ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, 10, ?8, ?9, ?9, ?10, ?11, ?12, ?13, '', ?14, ?15, ?16, ?17, 0, ?18)",
            params![
                project_id,
                reporter_id,
                initial_handler_id,
                priority,
                severity,
                reproducibility,
                initial_status,
                category_id,
                now,
                os,
                os_build,
                platform,
                version,
                extras.target_version,
                extras.build,
                trimmed_summary,
                view_state,
                extras.due_date,
            ],
        )
        .map_err(|e| e.to_string())?;

        let bug_id = tx.last_insert_rowid();

        tx.execute(
            "INSERT INTO mantis_bug_text_table (
                id, description, steps_to_reproduce, additional_information
            ) VALUES (?1, ?2, ?3, ?4)",
            params![
                bug_id,
                description,
                steps_to_reproduce,
                additional_information
            ],
        )
        .map_err(|e| e.to_string())?;

        // Record new bug creation in history (type = 1)
        self.record_history(&tx, bug_id, reporter_id, "", "", "", 1)
            .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;

        let project_name = Self::helper_project_name(&conn, project_id);
        let action_header = "The following issue has been SUBMITTED.";
        if let Ok(body) = self.build_mantis_email_body(&conn, bug_id, action_header, None) {
            let subject = format!("[{} {:07}] New: {}", project_name, bug_id, trimmed_summary);
            self.notify_bug_event_filtered_with_exclude(
                &conn,
                bug_id,
                0,
                &[],
                mentioned_usernames,
                &subject,
                &body,
            );
        }

        Ok(bug_id)
    }

    pub fn get_bug(&self, id: i64) -> rusqlite::Result<Option<Bug>> {
        let conn = self.conn();
        Self::query_bug_by_id(&conn, id)
    }

    pub fn get_bugs_by_ids(&self, ids: &[i64]) -> rusqlite::Result<HashMap<i64, Bug>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        const BATCH_CHUNK_SIZE: usize = 200;
        let conn = self.conn();
        let mut map = HashMap::new();
        for chunk in ids.chunks(BATCH_CHUNK_SIZE) {
            // Only positional `?1..?N` placeholders are generated; no user values are interpolated into SQL.
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let sql = format!(
                "SELECT id, project_id, reporter_id, handler_id, duplicate_id,
                        priority, severity, reproducibility, status, resolution,
                        category_id, date_submitted, last_updated, os, os_build,
                        platform, version, fixed_in_version, target_version, build,
                        summary, view_state, sticky, due_date
                 FROM mantis_bug_table
                 WHERE id IN ({})",
                placeholders.join(",")
            );
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), Self::map_bug_row)?;
            for row in rows {
                let bug = row?;
                map.insert(bug.id, bug);
            }
        }
        Ok(map)
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

    /// Returns raw unfiltered bugs for a project (or all projects when project_id <= 0).
    ///
    /// Note: `list_bugs` is an unfiltered query; user-facing callers and `filter_bugs`
    /// must filter results using `Db::can_view_bug` to enforce visibility rules.
    pub fn list_bugs(
        &self,
        project_id: i64,
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
                 WHERE project_id = ?1
                 ORDER BY sticky DESC, last_updated DESC, id DESC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![project_id, limit, offset], Self::map_bug_row)?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, reporter_id, handler_id, duplicate_id,
                        priority, severity, reproducibility, status, resolution,
                        category_id, date_submitted, last_updated, os, os_build,
                        platform, version, fixed_in_version, target_version, build,
                        summary, view_state, sticky, due_date
                 FROM mantis_bug_table
                 ORDER BY sticky DESC, last_updated DESC, id DESC
                 LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit, offset], Self::map_bug_row)?;
            rows.collect()
        }
    }

    pub fn update_bug_fields(
        &self,
        id: i64,
        updater_id: i64,
        fields: BugUpdate,
    ) -> Result<Vec<BugHistory>, String> {
        let mut conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", id))?;

        if let Some(expected) = fields.expected_last_updated {
            if expected != bug.last_updated {
                return Err("CONFLICT: Issue has been modified by another user".to_string());
            }
        }

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

        let new_os = match fields.os {
            Some(ref v) => {
                if *v != bug.os {
                    changes.push(("os", bug.os.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.os.clone(),
        };

        let new_os_build = match fields.os_build {
            Some(ref v) => {
                if *v != bug.os_build {
                    changes.push(("os_build", bug.os_build.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.os_build.clone(),
        };

        let new_platform = match fields.platform {
            Some(ref v) => {
                if *v != bug.platform {
                    changes.push(("platform", bug.platform.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.platform.clone(),
        };

        let new_build = match fields.build {
            Some(ref v) => {
                if *v != bug.build {
                    changes.push(("build", bug.build.clone(), v.clone()));
                }
                v.clone()
            }
            None => bug.build.clone(),
        };

        let new_due_date = match fields.due_date {
            Some(v) => {
                if v != bug.due_date {
                    changes.push(("due_date", bug.due_date.to_string(), v.to_string()));
                }
                v
            }
            None => bug.due_date,
        };

        let new_sticky = match fields.sticky {
            Some(v) => {
                if v != bug.sticky {
                    changes.push(("sticky", bug.sticky.to_string(), v.to_string()));
                }
                v
            }
            None => bug.sticky,
        };

        let tx = conn.transaction().map_err(|e| e.to_string())?;

        if !changes.is_empty() {
            if new_desc != cur_desc {
                tx.execute(
                    "INSERT INTO mantis_bug_revision_table (bug_id, bugnote_id, user_id, timestamp, type, value)
                     VALUES (?1, 0, ?2, ?3, ?4, ?5)",
                    params![id, updater_id, now, RevisionType::Description as i32, cur_desc],
                )
                .map_err(|e| e.to_string())?;
            }
            if new_steps != cur_steps {
                tx.execute(
                    "INSERT INTO mantis_bug_revision_table (bug_id, bugnote_id, user_id, timestamp, type, value)
                     VALUES (?1, 0, ?2, ?3, ?4, ?5)",
                    params![id, updater_id, now, RevisionType::StepsToReproduce as i32, cur_steps],
                )
                .map_err(|e| e.to_string())?;
            }
            if new_add_info != cur_add_info {
                tx.execute(
                    "INSERT INTO mantis_bug_revision_table (bug_id, bugnote_id, user_id, timestamp, type, value)
                     VALUES (?1, 0, ?2, ?3, ?4, ?5)",
                    params![id, updater_id, now, RevisionType::AdditionalInfo as i32, cur_add_info],
                )
                .map_err(|e| e.to_string())?;
            }

            tx.execute(
                "UPDATE mantis_bug_table
                 SET summary = ?1, category_id = ?2, priority = ?3, severity = ?4,
                     reproducibility = ?5, version = ?6, target_version = ?7,
                     fixed_in_version = ?8, view_state = ?9, handler_id = ?10,
                     os = ?11, os_build = ?12, platform = ?13, build = ?14,
                     due_date = ?15, sticky = ?16, last_updated = ?17
                 WHERE id = ?18",
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
                    new_os,
                    new_os_build,
                    new_platform,
                    new_build,
                    new_due_date,
                    new_sticky,
                    now,
                    id,
                ],
            )
            .map_err(|e| e.to_string())?;

            tx.execute(
                "UPDATE mantis_bug_text_table
                 SET description = ?1, steps_to_reproduce = ?2, additional_information = ?3
                 WHERE id = ?4",
                params![new_desc, new_steps, new_add_info, id],
            )
            .map_err(|e| e.to_string())?;
        }

        let mut recorded = Vec::new();
        for (field_name, old_val, new_val) in changes {
            self.record_history(&tx, id, updater_id, field_name, &old_val, &new_val, 0)
                .map_err(|e| e.to_string())?;
            let hid = tx.last_insert_rowid();
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

        tx.commit().map_err(|e| e.to_string())?;

        Ok(recorded)
    }

    pub fn delete_bug(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM mantis_bug_table WHERE id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM mantis_bug_revision_table WHERE bug_id = ?1",
            params![id],
        )?;
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
        self.change_bug_status_with_exclude(bug_id, user_id, new_status, resolution, note, &[])
    }

    pub fn change_bug_status_with_exclude(
        &self,
        bug_id: i64,
        user_id: i64,
        new_status: i32,
        resolution: i32,
        note: Option<&str>,
        exclude_usernames: &[String],
    ) -> Result<(), String> {
        let conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", bug_id))?;

        // Closed bug (status == 90) can only transition to feedback (20) to reopen
        if bug.status == 90 && new_status != 20 && new_status != 90 {
            return Err("Closed bug can only be reopened to feedback (status 20)".to_string());
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

        let mut note_mentions: Vec<String> = Vec::new();
        let note_id_opt = if let Some(note_text) = note {
            let trimmed = note_text.trim();
            if !trimmed.is_empty() {
                note_mentions = crate::models::collect_mentions(&[trimmed]);
                Some(
                    Self::insert_bugnote_inner(
                        &conn,
                        self,
                        bug_id,
                        user_id,
                        trimmed,
                        10,
                        0,
                        0,
                        false,
                        &[],
                    )
                    .map_err(|e| e.to_string())?,
                )
            } else {
                None
            }
        } else {
            None
        };

        let project_name = Self::helper_project_name(&conn, bug.project_id);
        let actor = Self::helper_user_by_id(&conn, user_id);
        let actor_name = actor
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("(Unknown)");

        let (action_header, subject) = if bug.status != new_status {
            let new_status_name = BugStatus::from_i32(new_status)
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            let action_header = match new_status {
                80 => "The following issue has been RESOLVED.",
                90 => "The following issue has been CLOSED.",
                _ => "The following issue has been UPDATED.",
            };
            let subject = format!(
                "[{} {:07}] Status changed to {}: {}",
                project_name, bug_id, new_status_name, bug.summary
            );
            (action_header, subject)
        } else if bug.resolution != resolution {
            let res_name = BugResolution::from_i32(resolution)
                .map(|r| r.as_str())
                .unwrap_or("updated");
            (
                "The following issue has been UPDATED.",
                format!(
                    "[{} {:07}] Resolution changed to {}: {}",
                    project_name, bug_id, res_name, bug.summary
                ),
            )
        } else if note_id_opt.is_some() {
            (
                "The following issue has been UPDATED.",
                format!("[{} {:07}] Updated: {}", project_name, bug_id, bug.summary),
            )
        } else {
            // Nothing changed and no note added -> skip sending email
            return Ok(());
        };

        let note_ctx = if let (Some(nid), Some(nt)) = (note_id_opt, note) {
            let trimmed = nt.trim();
            if !trimmed.is_empty() {
                Some((nid, actor_name, now, trimmed))
            } else {
                None
            }
        } else {
            None
        };

        let mut combined_excludes: Vec<String> = exclude_usernames.to_vec();
        combined_excludes.extend(note_mentions.iter().cloned());

        if let Ok(body) = self.build_mantis_email_body(&conn, bug_id, action_header, note_ctx) {
            self.notify_bug_event_filtered_with_exclude(
                &conn,
                bug_id,
                0,
                &[],
                &combined_excludes,
                &subject,
                &body,
            );
        }

        if let (Some(_), Some(nt)) = (note_id_opt, note) {
            let trimmed = nt.trim();
            if !note_mentions.is_empty() {
                let mention_ctx = if bug.status != new_status {
                    let new_status_name = BugStatus::from_i32(new_status)
                        .map(|s| s.as_str())
                        .unwrap_or("unknown");
                    format!("[Status changed to {}]\n\n{}", new_status_name, trimmed)
                } else if bug.resolution != resolution {
                    let res_name = BugResolution::from_i32(resolution)
                        .map(|r| r.as_str())
                        .unwrap_or("updated");
                    format!("[Resolution changed to {}]\n\n{}", res_name, trimmed)
                } else {
                    trimmed.to_string()
                };
                if let Err(e) = self.notify_bug_mentions_with_conn(
                    &conn,
                    bug_id,
                    user_id,
                    &note_mentions,
                    false,
                    &mention_ctx,
                ) {
                    eprintln!(
                        "[mentions] failed to send mention notifications on change_bug_status: {}",
                        e
                    );
                }
            }
        }

        Ok(())
    }

    pub fn assign_bug(&self, bug_id: i64, user_id: i64, handler_id: i64) -> Result<(), String> {
        let conn = self.conn();
        let bug = Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", bug_id))?;

        let handler_user = if handler_id > 0 {
            let u = Self::query_user_by_id(&conn, handler_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("User {} not found", handler_id))?;
            Some(u)
        } else {
            None
        };

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

        let project_name = Self::helper_project_name(&conn, bug.project_id);

        let (action_header, subject) = if let Some(ref h) = handler_user {
            (
                "The following issue has been ASSIGNED.",
                format!(
                    "[{} {:07}] Assigned to {}: {}",
                    project_name, bug_id, h.username, bug.summary
                ),
            )
        } else {
            (
                "The following issue has been UNASSIGNED.",
                format!(
                    "[{} {:07}] Unassigned: {}",
                    project_name, bug_id, bug.summary
                ),
            )
        };

        let prev_handler_id = bug.handler_id;
        let extra_recipients = if prev_handler_id > 0 && prev_handler_id != handler_id {
            vec![prev_handler_id]
        } else {
            vec![]
        };

        if let Ok(body) = self.build_mantis_email_body(&conn, bug_id, action_header, None) {
            self.notify_bug_event_filtered(&conn, bug_id, 0, &extra_recipients, &subject, &body);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn clone_bug(
        &self,
        source_bug_id: i64,
        reporter_id: i64,
        target_project_id: i64,
        copy_relationships: bool,
        copy_files: bool,
        upload_dir: &str,
    ) -> Result<i64, String> {
        let mut cleanup_guard = DiskFileCleanupGuard(Vec::new());
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let source_bug = Self::query_bug_by_id(&tx, source_bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Source issue {} not found", source_bug_id))?;

        let (description, steps_to_reproduce, additional_information) = match tx.query_row(
            "SELECT description, steps_to_reproduce, additional_information
             FROM mantis_bug_text_table
             WHERE id = ?1",
            params![source_bug_id],
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
            Err(e) => return Err(e.to_string()),
        };

        let target_enabled: Option<i32> = tx
            .query_row(
                "SELECT enabled FROM mantis_project_table WHERE id = ?1",
                params![target_project_id],
                |row| row.get(0),
            )
            .ok();
        if target_enabled != Some(1) {
            return Err("Target project not found or disabled".to_string());
        }

        let effective_cats =
            Self::query_effective_categories(&tx, target_project_id).map_err(|e| e.to_string())?;
        let target_category_id = if effective_cats
            .iter()
            .any(|c| c.id == source_bug.category_id)
        {
            source_bug.category_id
        } else if let Some(first_cat) = effective_cats.first() {
            first_cat.id
        } else {
            1
        };

        let mut initial_handler_id = 0i64;
        let mut initial_status = 10i32;
        if target_category_id > 0 {
            let cat_user_id: Option<i64> = tx
                .query_row(
                    "SELECT user_id FROM mantis_category_table WHERE id = ?1",
                    params![target_category_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(uid) = cat_user_id {
                if uid > 0 {
                    if let Ok(Some(u)) = Self::query_user_by_id(&tx, uid) {
                        if u.enabled == 1 {
                            let eff =
                                Self::query_effective_access_level(&tx, &u, target_project_id)
                                    .unwrap_or(0);
                            if eff >= 55 {
                                initial_handler_id = uid;
                                initial_status = 50;
                            }
                        }
                    }
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        tx.execute(
            "INSERT INTO mantis_bug_table (
                project_id, reporter_id, handler_id, duplicate_id,
                priority, severity, reproducibility, status, resolution,
                category_id, date_submitted, last_updated, os, os_build,
                platform, version, fixed_in_version, target_version, build,
                summary, view_state, sticky, due_date
            ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, 10, ?8, ?9, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 0, ?19)",
            params![
                target_project_id,
                reporter_id,
                initial_handler_id,
                source_bug.priority,
                source_bug.severity,
                source_bug.reproducibility,
                initial_status,
                target_category_id,
                now,
                source_bug.os,
                source_bug.os_build,
                source_bug.platform,
                source_bug.version,
                source_bug.fixed_in_version,
                source_bug.target_version,
                source_bug.build,
                source_bug.summary,
                source_bug.view_state,
                source_bug.due_date,
            ],
        )
        .map_err(|e| e.to_string())?;

        let new_bug_id = tx.last_insert_rowid();

        tx.execute(
            "INSERT INTO mantis_bug_text_table (
                id, description, steps_to_reproduce, additional_information
            ) VALUES (?1, ?2, ?3, ?4)",
            params![
                new_bug_id,
                description,
                steps_to_reproduce,
                additional_information,
            ],
        )
        .map_err(|e| e.to_string())?;

        // Copy custom field values (for fields linked to target_project_id)
        {
            let mut stmt = tx
                .prepare(
                    "SELECT cfs.field_id, cfs.value, cfs.text
                     FROM mantis_custom_field_string_table cfs
                     JOIN mantis_custom_field_project_table cfp ON cfs.field_id = cfp.field_id
                     WHERE cfs.bug_id = ?1 AND cfp.project_id = ?2",
                )
                .map_err(|e| e.to_string())?;
            let cfs_rows = stmt
                .query_map(params![source_bug_id, target_project_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            let mut field_values = Vec::new();
            for val in cfs_rows.flatten() {
                field_values.push(val);
            }
            for (fid, val, txt) in field_values {
                tx.execute(
                    "INSERT INTO mantis_custom_field_string_table (field_id, bug_id, value, text)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (field_id, bug_id) DO UPDATE SET value = excluded.value, text = excluded.text",
                    params![fid, new_bug_id, val, txt],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Copy tags (mantis_bug_tag_table)
        {
            let mut stmt = tx
                .prepare("SELECT tag_id, user_id FROM mantis_bug_tag_table WHERE bug_id = ?1")
                .map_err(|e| e.to_string())?;
            let tag_rows = stmt
                .query_map(params![source_bug_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| e.to_string())?;
            let mut tags = Vec::new();
            for t in tag_rows.flatten() {
                tags.push(t);
            }
            for (tid, uid) in tags {
                tx.execute(
                    "INSERT OR IGNORE INTO mantis_bug_tag_table (bug_id, tag_id, user_id, date_attached)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![new_bug_id, tid, uid, now],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Always create RelatedTo relationship between cloned issue and source issue
        tx.execute(
            "INSERT INTO mantis_bug_relationship_table (
                source_bug_id, destination_bug_id, relationship_type
            ) VALUES (?1, ?2, 0)",
            params![new_bug_id, source_bug_id],
        )
        .map_err(|e| e.to_string())?;

        // If copy_relationships is true, copy existing source relationships cleanly
        if copy_relationships {
            let mut stmt = tx
                .prepare(
                    "SELECT destination_bug_id, relationship_type
                     FROM mantis_bug_relationship_table
                     WHERE source_bug_id = ?1",
                )
                .map_err(|e| e.to_string())?;
            let rel_rows = stmt
                .query_map(params![source_bug_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?))
                })
                .map_err(|e| e.to_string())?;
            let mut rels = Vec::new();
            for rel in rel_rows.flatten() {
                rels.push(rel);
            }
            for (dest_id, rel_type) in rels {
                if dest_id == new_bug_id || dest_id == source_bug_id {
                    continue;
                }
                if rel_type == 2 {
                    // An issue can only be a duplicate of one issue.
                    // If source issue has a duplicate (type 2), the clone itself should NOT claim
                    // the other issue as its duplicate. Skip type 2 entirely.
                    continue;
                }

                tx.execute(
                    "INSERT INTO mantis_bug_relationship_table (
                        source_bug_id, destination_bug_id, relationship_type
                    ) VALUES (?1, ?2, ?3)",
                    params![new_bug_id, dest_id, rel_type],
                )
                .map_err(|e| e.to_string())?;

                if rel_type == 1 {
                    // Cloned issue is duplicate of dest_id (matching add_bug_relationship)
                    tx.execute(
                        "UPDATE mantis_bug_table SET duplicate_id = ?1 WHERE id = ?2",
                        params![dest_id, new_bug_id],
                    )
                    .map_err(|e| e.to_string())?;
                    tx.execute(
                        "INSERT INTO mantis_bug_relationship_table (
                            source_bug_id, destination_bug_id, relationship_type
                        ) VALUES (?1, ?2, 2)",
                        params![dest_id, new_bug_id],
                    )
                    .map_err(|e| e.to_string())?;
                }
                // For type 0 (RelatedTo): only new_bug_id -> dest_id is inserted.
                // Do not create reciprocal reverse row on dest_id, consistent with MantisBT design.
            }
        }

        // If copy_files is true, duplicate attachments (separate copies)
        if copy_files {
            let mut stmt = tx
                .prepare(
                    "SELECT title, description, diskfile, filename, folder,
                            filesize, file_type, content
                     FROM mantis_bug_file_table
                     WHERE bug_id = ?1",
                )
                .map_err(|e| e.to_string())?;
            let file_rows = stmt
                .query_map(params![source_bug_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<Vec<u8>>>(7)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            let mut files = Vec::new();
            for f in file_rows.flatten() {
                files.push(f);
            }
            for (title, description, diskfile, filename, folder, filesize, file_type, blob_opt) in
                files
            {
                let (new_diskfile, new_folder, new_blob_opt): (String, String, Option<Vec<u8>>) =
                    if let Some(bytes) = blob_opt {
                        (String::new(), String::new(), Some(bytes))
                    } else if !diskfile.is_empty() {
                        let source_dir = if !upload_dir.trim().is_empty() {
                            upload_dir
                        } else if !folder.trim().is_empty() {
                            &folder
                        } else {
                            "."
                        };
                        let src_path = std::path::Path::new(source_dir).join(&diskfile);
                        if !src_path.exists() {
                            return Err(format!(
                                "Source attachment file not found on disk: {}",
                                diskfile
                            ));
                        }
                        let bytes = std::fs::read(&src_path).map_err(|e| {
                            format!("Failed to read attachment file {}: {}", diskfile, e)
                        })?;
                        let target_dir = if !upload_dir.trim().is_empty() {
                            upload_dir
                        } else if !folder.trim().is_empty() {
                            &folder
                        } else {
                            "."
                        };
                        std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
                        let fresh_name = format!(
                            "{:016x}{:016x}",
                            rand::random::<u64>(),
                            rand::random::<u64>()
                        );
                        let dst_path = std::path::Path::new(target_dir).join(&fresh_name);
                        cleanup_guard.0.push(dst_path.clone());
                        std::fs::write(&dst_path, &bytes).map_err(|e| e.to_string())?;
                        (fresh_name, target_dir.to_string(), None)
                    } else {
                        (String::new(), String::new(), None)
                    };

                tx.execute(
                    "INSERT INTO mantis_bug_file_table (
                        bug_id, title, description, diskfile, filename, folder,
                        filesize, file_type, content, date_added, user_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        new_bug_id,
                        title,
                        description,
                        new_diskfile,
                        filename,
                        new_folder,
                        filesize,
                        file_type,
                        new_blob_opt,
                        now,
                        reporter_id,
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Record creation + clone history entry
        self.record_history(&tx, new_bug_id, reporter_id, "", "", "", 1)
            .map_err(|e| e.to_string())?;
        self.record_history(
            &tx,
            new_bug_id,
            reporter_id,
            "created_from",
            "",
            &source_bug_id.to_string(),
            21,
        )
        .map_err(|e| e.to_string())?;
        self.record_history(
            &tx,
            source_bug_id,
            reporter_id,
            "cloned_to",
            "",
            &new_bug_id.to_string(),
            20,
        )
        .map_err(|e| e.to_string())?;

        // Commit transaction before sending notification
        tx.commit().map_err(|e| e.to_string())?;
        cleanup_guard.dismiss();

        // Queue standard new-issue notification email.
        // Intentionally do NOT re-notify historical @mentions copied from the source issue
        // to avoid notification spam during cloning.
        let project_name = Self::helper_project_name(&conn, target_project_id);
        let action_header = "The following issue has been SUBMITTED.";
        if let Ok(body) = self.build_mantis_email_body(&conn, new_bug_id, action_header, None) {
            let subject = format!(
                "[{} {:07}] New: {}",
                project_name,
                new_bug_id,
                source_bug.summary.trim()
            );
            self.notify_bug_event(&conn, new_bug_id, &subject, &body);
        }

        Ok(new_bug_id)
    }

    pub fn move_bug(
        &self,
        bug_id: i64,
        mover_id: i64,
        target_project_id: i64,
    ) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let bug = Self::query_bug_by_id(&tx, bug_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Bug {} not found", bug_id))?;

        if bug.project_id == target_project_id {
            return Ok(());
        }

        let target_enabled: Option<i32> = tx
            .query_row(
                "SELECT enabled FROM mantis_project_table WHERE id = ?1",
                params![target_project_id],
                |row| row.get(0),
            )
            .ok();
        if target_enabled != Some(1) {
            return Err("Target project not found or disabled".to_string());
        }

        let effective_cats =
            Self::query_effective_categories(&tx, target_project_id).map_err(|e| e.to_string())?;
        let cat_effective = effective_cats.iter().any(|c| c.id == bug.category_id);
        let (new_cat_id, cat_changed) = if cat_effective {
            (bug.category_id, false)
        } else if let Some(first_cat) = effective_cats.first() {
            (first_cat.id, true)
        } else {
            (1, true)
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        tx.execute(
            "UPDATE mantis_bug_table
             SET project_id = ?1, category_id = ?2, last_updated = ?3
             WHERE id = ?4",
            params![target_project_id, new_cat_id, now, bug_id],
        )
        .map_err(|e| e.to_string())?;

        self.record_history(
            &tx,
            bug_id,
            mover_id,
            "project_id",
            &bug.project_id.to_string(),
            &target_project_id.to_string(),
            0,
        )
        .map_err(|e| e.to_string())?;

        if cat_changed {
            self.record_history(
                &tx,
                bug_id,
                mover_id,
                "category_id",
                &bug.category_id.to_string(),
                &new_cat_id.to_string(),
                0,
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        let target_project_name = Self::helper_project_name(&conn, target_project_id);
        let action_header = "The following issue has been MOVED.";
        if let Ok(body) = self.build_mantis_email_body(&conn, bug_id, action_header, None) {
            let subject = format!(
                "[{} {:07}] Moved to {}: {}",
                target_project_name, bug_id, target_project_name, bug.summary
            );
            self.notify_bug_event(&conn, bug_id, &subject, &body);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_bugnote_inner(
        conn: &Connection,
        db: &Db,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
        note_type: i32,
        notify: bool,
        exclude_usernames: &[String],
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, '', ?7)",
            params![
                bug_id,
                reporter_id,
                text_id,
                view_state,
                now,
                note_type,
                safe_time
            ],
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

        if notify {
            let bug_opt = Self::query_bug_by_id(conn, bug_id).ok().flatten();
            let (project_id, bug_summary) = match bug_opt {
                Some(ref b) => (b.project_id, b.summary.clone()),
                None => (0, format!("Issue #{}", bug_id)),
            };
            let project_name = Self::helper_project_name(conn, project_id);

            let actor = Self::helper_user_by_id(conn, reporter_id);
            let actor_name = actor
                .as_ref()
                .map(|u| u.username.as_str())
                .unwrap_or("(Unknown)");

            let action_header = "A NOTE has been added to this issue.";
            let note_ctx = Some((note_id, actor_name, now, note));
            let body = db
                .build_mantis_email_body(conn, bug_id, action_header, note_ctx)
                .unwrap_or_default();

            let min_access = if view_state == 50 { 55 } else { 0 };
            db.notify_bug_event_filtered_with_exclude(
                conn,
                bug_id,
                min_access,
                &[],
                exclude_usernames,
                &format!(
                    "[{} {:07}] Note added by {}: {}",
                    project_name, bug_id, actor_name, bug_summary
                ),
                &body,
            );
        }

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

    pub fn add_bugnote_with_type(
        &self,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
        note_type: i32,
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

        let notify = note_type != 1;
        Self::insert_bugnote_inner(
            &conn,
            self,
            bug_id,
            reporter_id,
            trimmed,
            view_state,
            time_tracking,
            note_type,
            notify,
            &[],
        )
        .map_err(|e| e.to_string())
    }

    pub fn add_bugnote_with_time_and_mentions(
        &self,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
        mentioned_usernames: &[String],
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
            0,
            true,
            mentioned_usernames,
        )
        .map_err(|e| e.to_string())
    }

    pub fn add_bugnote_with_time(
        &self,
        bug_id: i64,
        reporter_id: i64,
        note: &str,
        view_state: i32,
        time_tracking: i32,
    ) -> Result<i64, String> {
        self.add_bugnote_with_type(bug_id, reporter_id, note, view_state, time_tracking, 0)
    }

    fn map_bugnote_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bugnote> {
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
            Self::map_bugnote_row,
        ) {
            Ok(n) => Ok(Some(n)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_bugnotes_by_ids(&self, ids: &[i64]) -> rusqlite::Result<HashMap<i64, Bugnote>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        const BATCH_CHUNK_SIZE: usize = 200;
        let conn = self.conn();
        let mut map = HashMap::new();
        for chunk in ids.chunks(BATCH_CHUNK_SIZE) {
            // Only positional `?1..?N` placeholders are generated; no user values are interpolated into SQL.
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let sql = format!(
                "SELECT n.id, n.bug_id, n.reporter_id, n.bugnote_text_id, n.view_state,
                        n.date_submitted, n.last_modified, n.note_type, n.note_attr,
                        n.time_tracking, COALESCE(t.note, '')
                 FROM mantis_bugnote_table n
                 LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
                 WHERE n.id IN ({})",
                placeholders.join(",")
            );
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), Self::map_bugnote_row)?;
            for row in rows {
                let note = row?;
                map.insert(note.id, note);
            }
        }
        Ok(map)
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

        conn.execute(
            "DELETE FROM mantis_bug_revision_table WHERE bugnote_id = ?1",
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
        let rows = stmt.query_map(params![bug_id, user_access], Self::map_bugnote_row)?;
        rows.collect()
    }

    pub fn update_bugnote(
        &self,
        note_id: i64,
        editor_id: i64,
        new_note: &str,
    ) -> Result<(), String> {
        let trimmed = new_note.trim();
        if trimmed.is_empty() {
            return Err("Bugnote cannot be empty".to_string());
        }

        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let note = match tx.query_row(
            "SELECT n.id, n.bug_id, n.reporter_id, n.bugnote_text_id, n.view_state,
                    n.date_submitted, n.last_modified, n.note_type, n.note_attr,
                    n.time_tracking, COALESCE(t.note, '')
             FROM mantis_bugnote_table n
             LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
             WHERE n.id = ?1",
            params![note_id],
            Self::map_bugnote_row,
        ) {
            Ok(n) => n,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(format!("Bugnote {} not found", note_id));
            }
            Err(e) => return Err(e.to_string()),
        };

        if new_note == note.note {
            return Ok(());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // 1. Save previous note text to mantis_bug_revision_table
        tx.execute(
            "INSERT INTO mantis_bug_revision_table (bug_id, bugnote_id, user_id, timestamp, type, value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                note.bug_id,
                note_id,
                editor_id,
                now,
                RevisionType::Bugnote as i32,
                note.note,
            ],
        )
        .map_err(|e| e.to_string())?;

        // 2. Update note text in mantis_bugnote_text_table (preserve new_note unmodified)
        tx.execute(
            "UPDATE mantis_bugnote_text_table SET note = ?1 WHERE id = ?2",
            params![new_note, note.bugnote_text_id],
        )
        .map_err(|e| e.to_string())?;

        // 3. Update bugnote last_modified
        tx.execute(
            "UPDATE mantis_bugnote_table SET last_modified = ?1 WHERE id = ?2",
            params![now, note_id],
        )
        .map_err(|e| e.to_string())?;

        // 4. Update bug last_updated
        tx.execute(
            "UPDATE mantis_bug_table SET last_updated = ?1 WHERE id = ?2",
            params![now, note.bug_id],
        )
        .map_err(|e| e.to_string())?;

        // 5. Record history
        self.record_history(
            &tx,
            note.bug_id,
            editor_id,
            "bugnote_updated",
            &note_id.to_string(),
            &note_id.to_string(),
            0,
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn set_bugnote_view_state(
        &self,
        note_id: i64,
        editor_id: i64,
        view_state: i32,
    ) -> Result<(), String> {
        if view_state != 10 && view_state != 50 {
            return Err("Invalid view_state: must be 10 (public) or 50 (private)".to_string());
        }

        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let note = match tx.query_row(
            "SELECT n.id, n.bug_id, n.reporter_id, n.bugnote_text_id, n.view_state,
                    n.date_submitted, n.last_modified, n.note_type, n.note_attr,
                    n.time_tracking, COALESCE(t.note, '')
             FROM mantis_bugnote_table n
             LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
             WHERE n.id = ?1",
            params![note_id],
            Self::map_bugnote_row,
        ) {
            Ok(n) => n,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(format!("Bugnote {} not found", note_id));
            }
            Err(e) => return Err(e.to_string()),
        };

        if note.view_state == view_state {
            return Ok(());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        tx.execute(
            "UPDATE mantis_bugnote_table SET view_state = ?1, last_modified = ?2 WHERE id = ?3",
            params![view_state, now, note_id],
        )
        .map_err(|e| e.to_string())?;

        self.record_history(
            &tx,
            note.bug_id,
            editor_id,
            "bugnote_view_state",
            &note.view_state.to_string(),
            &view_state.to_string(),
            0,
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn list_bug_revisions(
        &self,
        bug_id: i64,
        bugnote_id: Option<i64>,
    ) -> rusqlite::Result<Vec<BugRevision>> {
        let conn = self.conn();
        match bugnote_id {
            Some(nid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, bug_id, bugnote_id, user_id, timestamp, type, value
                     FROM mantis_bug_revision_table
                     WHERE bug_id = ?1 AND bugnote_id = ?2
                     ORDER BY timestamp DESC, id DESC",
                )?;
                let rows = stmt.query_map(params![bug_id, nid], Self::map_bug_revision_row)?;
                rows.collect()
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, bug_id, bugnote_id, user_id, timestamp, type, value
                     FROM mantis_bug_revision_table
                     WHERE bug_id = ?1
                     ORDER BY timestamp DESC, id DESC",
                )?;
                let rows = stmt.query_map(params![bug_id], Self::map_bug_revision_row)?;
                rows.collect()
            }
        }
    }

    pub fn get_bug_revision_by_id(
        &self,
        revision_id: i64,
    ) -> rusqlite::Result<Option<BugRevision>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, bug_id, bugnote_id, user_id, timestamp, type, value
             FROM mantis_bug_revision_table
             WHERE id = ?1",
            params![revision_id],
            Self::map_bug_revision_row,
        ) {
            Ok(rev) => Ok(Some(rev)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn drop_bug_revision(&self, revision_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_bug_revision_table WHERE id = ?1",
            params![revision_id],
        )?;
        Ok(())
    }

    fn map_bug_revision_row(row: &rusqlite::Row) -> rusqlite::Result<BugRevision> {
        Ok(BugRevision {
            id: row.get(0)?,
            bug_id: row.get(1)?,
            bugnote_id: row.get(2)?,
            user_id: row.get(3)?,
            timestamp: row.get(4)?,
            rev_type: row.get(5)?,
            value: row.get(6)?,
        })
    }

    pub fn query_bug_history(conn: &Connection, bug_id: i64) -> rusqlite::Result<Vec<BugHistory>> {
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

    pub fn list_bug_history(&self, bug_id: i64) -> rusqlite::Result<Vec<BugHistory>> {
        let conn = self.conn();
        Self::query_bug_history(&conn, bug_id)
    }

    pub fn list_all_categories_map(&self) -> rusqlite::Result<HashMap<i64, String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, name FROM mantis_category_table")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, name) = row?;
            map.insert(id, name);
        }
        Ok(map)
    }

    /// Batch-fetch visible note counts and attachment counts for a set of bug IDs without N+1 queries.
    /// Private notes (`view_state = 50`) are only counted when the user's effective access level on
    /// the bug's project is at least Developer (`55`).
    pub fn get_bug_note_and_file_counts(
        &self,
        bug_ids: &[i64],
        bug_access_levels: &HashMap<i64, i32>,
    ) -> rusqlite::Result<(HashMap<i64, usize>, HashMap<i64, usize>)> {
        let mut note_counts = HashMap::new();
        let mut file_counts = HashMap::new();
        if bug_ids.is_empty() {
            return Ok((note_counts, file_counts));
        }

        let conn = self.conn();
        for chunk in bug_ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(",");
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

            let note_sql = format!(
                "SELECT bug_id,
                        SUM(CASE WHEN view_state = 10 THEN 1 ELSE 0 END) AS pub_cnt,
                        COUNT(*) AS all_cnt
                 FROM mantis_bugnote_table
                 WHERE bug_id IN ({})
                 GROUP BY bug_id",
                in_clause
            );
            let mut note_stmt = conn.prepare(&note_sql)?;
            let note_rows = note_stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            for r in note_rows {
                let (bid, pub_cnt, all_cnt) = r?;
                let eff = bug_access_levels.get(&bid).copied().unwrap_or(0);
                let visible = if eff >= 55 { all_cnt } else { pub_cnt };
                if visible > 0 {
                    note_counts.insert(bid, visible as usize);
                }
            }

            let file_sql = format!(
                "SELECT bug_id, COUNT(*)
                 FROM mantis_bug_file_table
                 WHERE bug_id IN ({})
                 GROUP BY bug_id",
                in_clause
            );
            let mut file_stmt = conn.prepare(&file_sql)?;
            let file_rows = file_stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in file_rows {
                let (bid, cnt) = r?;
                if cnt > 0 {
                    file_counts.insert(bid, cnt as usize);
                }
            }
        }

        Ok((note_counts, file_counts))
    }

    /// List recent timeline activity visible to `user`, optionally scoped to `project_id`
    /// (including its subprojects). Strictly enforces project access, `Db::can_view_bug` for
    /// private issues, and Developer-only (`>= 55`) visibility for private notes.
    pub fn list_timeline_events(
        &self,
        user: &User,
        project_id: Option<i64>,
        limit: usize,
    ) -> rusqlite::Result<Vec<TimelineEvent>> {
        if limit == 0 || user.access_level < 10 {
            return Ok(Vec::new());
        }

        let mut visible_pids = Vec::new();
        let mut dev_pids = Vec::new();

        if let Some(pid) = project_id.filter(|&p| p > 0) {
            let eff = self.effective_access_level(user, pid)?;
            if eff < 10 {
                return Ok(Vec::new());
            }
            for sub_pid in self.list_project_descendant_ids(pid)? {
                let sub_eff = self.effective_access_level(user, sub_pid)?;
                if sub_eff >= 10 {
                    visible_pids.push(sub_pid);
                    if sub_eff >= 55 {
                        dev_pids.push(sub_pid);
                    }
                }
            }
        } else {
            for (proj, _) in self.list_visible_projects(user)? {
                let proj_eff = self.effective_access_level(user, proj.id)?;
                if proj_eff >= 10 {
                    visible_pids.push(proj.id);
                    if proj_eff >= 55 {
                        dev_pids.push(proj.id);
                    }
                }
            }
        }

        if visible_pids.is_empty() {
            return Ok(Vec::new());
        }

        let mut bound_values: Vec<i64> =
            Vec::with_capacity(2 + visible_pids.len() + dev_pids.len());
        bound_values.push(user.id);
        bound_values.push(limit as i64);

        let vis_start = 3;
        let vis_placeholders: Vec<String> = (vis_start..vis_start + visible_pids.len())
            .map(|i| format!("?{}", i))
            .collect();
        bound_values.extend(&visible_pids);

        let dev_cond = if dev_pids.is_empty() {
            "0 = 1".to_string()
        } else {
            let dev_start = vis_start + visible_pids.len();
            let dev_placeholders: Vec<String> = (dev_start..dev_start + dev_pids.len())
                .map(|i| format!("?{}", i))
                .collect();
            bound_values.extend(&dev_pids);
            format!("b.project_id IN ({})", dev_placeholders.join(","))
        };

        let sql = format!(
            "SELECT h.id, h.bug_id, b.summary, b.status, b.project_id,
                    h.user_id, COALESCE(u.username, ''), h.date_modified,
                    h.field_name, h.old_value, h.new_value, h.type
             FROM mantis_bug_history_table h
             JOIN mantis_bug_table b ON h.bug_id = b.id
             LEFT JOIN mantis_user_table u ON h.user_id = u.id
             LEFT JOIN mantis_bugnote_table n
               ON (h.field_name = 'bugnote_added' AND CAST(h.new_value AS INTEGER) > 0 AND n.id = CAST(h.new_value AS INTEGER))
               OR (h.field_name = 'bugnote_updated' AND CAST(h.old_value AS INTEGER) > 0 AND n.id = CAST(h.old_value AS INTEGER))
             WHERE b.project_id IN ({vis_in})
               AND (b.view_state = 10 OR b.reporter_id = ?1 OR b.handler_id = ?1 OR {dev_cond})
               AND (h.field_name != 'bugnote_view_state' OR {dev_cond})
               AND (
                   h.field_name != 'time_tracking'
                   OR {dev_cond}
                   OR EXISTS (
                       SELECT 1 FROM mantis_bugnote_table tn
                       WHERE tn.bug_id = h.bug_id
                         AND tn.reporter_id = h.user_id
                         AND ABS(tn.date_submitted - h.date_modified) <= 2
                         AND tn.time_tracking = CAST(h.new_value AS INTEGER)
                         AND tn.view_state = 10
                   )
               )
               AND (
                   h.field_name NOT IN ('bugnote_added', 'bugnote_updated')
                   OR (n.id IS NOT NULL AND (n.view_state = 10 OR {dev_cond}))
               )
             ORDER BY h.date_modified DESC, h.id DESC
             LIMIT ?2",
            vis_in = vis_placeholders.join(","),
            dev_cond = dev_cond
        );

        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = bound_values
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let actor_id: i64 = row.get(5)?;
            let username: String = row.get(6)?;
            Ok(TimelineEvent {
                id: row.get(0)?,
                bug_id: row.get(1)?,
                bug_summary: row.get(2)?,
                bug_status: row.get(3)?,
                project_id: row.get(4)?,
                user_id: actor_id,
                username: if username.is_empty() {
                    format!("user#{}", actor_id)
                } else {
                    username
                },
                date_modified: row.get(7)?,
                field_name: row.get(8)?,
                old_value: row.get(9)?,
                new_value: row.get(10)?,
                type_: row.get(11)?,
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

        self.record_history(
            &conn,
            bug_id,
            user_id,
            "file_added",
            "",
            trimmed_filename,
            2,
        )
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

    pub fn list_tags_with_counts(&self) -> rusqlite::Result<Vec<(Tag, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.user_id, t.name, t.description, t.date_created, t.date_updated, COUNT(bt.bug_id)
             FROM mantis_tag_table t
             LEFT JOIN mantis_bug_tag_table bt ON t.id = bt.tag_id
             GROUP BY t.id, t.user_id, t.name, t.description, t.date_created, t.date_updated
             ORDER BY t.name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                Tag {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    date_created: row.get(4)?,
                    date_updated: row.get(5)?,
                },
                row.get(6)?,
            ))
        })?;
        rows.collect()
    }

    pub fn get_tag_with_count_by_id(&self, id: i64) -> rusqlite::Result<Option<(Tag, i64)>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT t.id, t.user_id, t.name, t.description, t.date_created, t.date_updated, COUNT(bt.bug_id)
             FROM mantis_tag_table t
             LEFT JOIN mantis_bug_tag_table bt ON t.id = bt.tag_id
             WHERE t.id = ?1
             GROUP BY t.id, t.user_id, t.name, t.description, t.date_created, t.date_updated",
            params![id],
            |row| {
                Ok((
                    Tag {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                        date_created: row.get(4)?,
                        date_updated: row.get(5)?,
                    },
                    row.get(6)?,
                ))
            },
        ) {
            Ok(res) => Ok(Some(res)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn attach_tag_to_bug(&self, bug_id: i64, tag_id: i64, user_id: i64) -> Result<(), String> {
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
    pub fn create_custom_field_full(
        &self,
        name: &str,
        field_type: i32,
        possible_values: &str,
        default_value: &str,
        valid_regexp: &str,
        access_level_r: i32,
        access_level_rw: i32,
        length_min: i32,
        length_max: i32,
        require_report: bool,
        require_update: bool,
        display_report: bool,
        display_update: bool,
        require_resolved: bool,
        display_resolved: bool,
        display_closed: bool,
        require_closed: bool,
    ) -> Result<i64, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Custom field name cannot be empty".to_string());
        }
        crate::models::validate_regex_pattern(valid_regexp)
            .map_err(|e| format!("Invalid regular expression: {}", e))?;
        let trimmed_regexp = valid_regexp.trim();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_custom_field_table (
                name, type, possible_values, default_value, valid_regexp,
                access_level_r, access_level_rw,
                length_min, length_max, require_report, require_update,
                display_report, display_update,
                require_resolved, display_resolved, display_closed, require_closed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                trimmed,
                field_type,
                possible_values,
                default_value,
                trimmed_regexp,
                access_level_r,
                access_level_rw,
                length_min,
                length_max,
                if require_report { 1 } else { 0 },
                if require_update { 1 } else { 0 },
                if display_report { 1 } else { 0 },
                if display_update { 1 } else { 0 },
                if require_resolved { 1 } else { 0 },
                if display_resolved { 1 } else { 0 },
                if display_closed { 1 } else { 0 },
                if require_closed { 1 } else { 0 },
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
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
        self.create_custom_field_full(
            name,
            field_type,
            possible_values,
            default_value,
            valid_regexp,
            10,
            25,
            length_min,
            length_max,
            require_report,
            require_update,
            display_report,
            display_update,
            false,
            false,
            false,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_custom_field_full(
        &self,
        id: i64,
        name: &str,
        field_type: i32,
        possible_values: &str,
        default_value: &str,
        valid_regexp: &str,
        access_level_r: i32,
        access_level_rw: i32,
        length_min: i32,
        length_max: i32,
        require_report: bool,
        require_update: bool,
        display_report: bool,
        display_update: bool,
        require_resolved: bool,
        display_resolved: bool,
        display_closed: bool,
        require_closed: bool,
    ) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Custom field name cannot be empty".to_string());
        }
        crate::models::validate_regex_pattern(valid_regexp)
            .map_err(|e| format!("Invalid regular expression: {}", e))?;
        let trimmed_regexp = valid_regexp.trim();
        let conn = self.conn();
        let rows = conn
            .execute(
                "UPDATE mantis_custom_field_table
                 SET name = ?1, type = ?2, possible_values = ?3, default_value = ?4,
                     valid_regexp = ?5, access_level_r = ?6, access_level_rw = ?7,
                     length_min = ?8, length_max = ?9,
                     require_report = ?10, require_update = ?11,
                     display_report = ?12, display_update = ?13,
                     require_resolved = ?14, display_resolved = ?15,
                     display_closed = ?16, require_closed = ?17
                 WHERE id = ?18",
                params![
                    trimmed,
                    field_type,
                    possible_values,
                    default_value,
                    trimmed_regexp,
                    access_level_r,
                    access_level_rw,
                    length_min,
                    length_max,
                    if require_report { 1 } else { 0 },
                    if require_update { 1 } else { 0 },
                    if display_report { 1 } else { 0 },
                    if display_update { 1 } else { 0 },
                    if require_resolved { 1 } else { 0 },
                    if display_resolved { 1 } else { 0 },
                    if display_closed { 1 } else { 0 },
                    if require_closed { 1 } else { 0 },
                    id,
                ],
            )
            .map_err(|e| e.to_string())?;
        if rows == 0 {
            return Err(format!("Custom field {} not found", id));
        }
        Ok(())
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
        let (r, rw, req_res, disp_res, disp_close, req_close) = {
            let conn = self.conn();
            conn.query_row(
                "SELECT access_level_r, access_level_rw, require_resolved, display_resolved, display_closed, require_closed
                 FROM mantis_custom_field_table WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)? != 0,
                        row.get::<_, i32>(3)? != 0,
                        row.get::<_, i32>(4)? != 0,
                        row.get::<_, i32>(5)? != 0,
                    ))
                },
            )
            .map_err(|e| format!("Custom field {} not found: {}", id, e))?
        };

        self.update_custom_field_full(
            id,
            name,
            field_type,
            possible_values,
            default_value,
            valid_regexp,
            r,
            rw,
            length_min,
            length_max,
            require_report,
            require_update,
            display_report,
            display_update,
            req_res,
            disp_res,
            disp_close,
            req_close,
        )
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

        let bug = match Self::query_bug_by_id(&conn, bug_id).map_err(|e| e.to_string())? {
            Some(b) => b,
            None => return Err(format!("Bug {} not found", bug_id)),
        };

        let versions = Self::query_versions(&conn, bug.project_id).unwrap_or_default();
        let cats = Self::query_effective_categories(&conn, bug.project_id).unwrap_or_default();
        let resolved =
            crate::models::resolve_possible_values(&cf.possible_values, &versions, &cats);

        crate::models::validate_custom_field_value(&cf, value, &resolved, false)?;

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
            "SELECT cf.id, cf.name, cf.type, COALESCE(cfs.value, cf.default_value), cf.access_level_r, cf.access_level_rw
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
                access_level_r: row.get(4)?,
                access_level_rw: row.get(5)?,
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
        conn.execute(
            "DELETE FROM mantis_filters_table WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn add_bug_monitor_with_actor(
        &self,
        bug_id: i64,
        user_id: i64,
        actor_id: i64,
    ) -> Result<(), String> {
        let conn = self.conn();
        if Self::query_bug_by_id(&conn, bug_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err(format!("Bug {} not found", bug_id));
        }
        let target_user = Self::query_user_by_id(&conn, user_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("User {} not found", user_id))?;

        let rows = conn
            .execute(
                "INSERT OR IGNORE INTO mantis_bug_monitor_table (user_id, bug_id) VALUES (?1, ?2)",
                params![user_id, bug_id],
            )
            .map_err(|e| e.to_string())?;

        if rows > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let _ = conn.execute(
                "UPDATE mantis_bug_table SET last_updated = ?1 WHERE id = ?2",
                params![now, bug_id],
            );
            let _ = self.record_history(
                &conn,
                bug_id,
                actor_id,
                "monitor_added",
                "",
                &target_user.username,
                12,
            );
        }
        Ok(())
    }

    pub fn add_bug_monitor(&self, bug_id: i64, user_id: i64) -> Result<(), String> {
        self.add_bug_monitor_with_actor(bug_id, user_id, user_id)
    }

    pub fn remove_bug_monitor_with_actor(
        &self,
        bug_id: i64,
        user_id: i64,
        actor_id: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn();
        let target_user = Self::query_user_by_id(&conn, user_id).ok().flatten();
        let username = target_user
            .map(|u| u.username)
            .unwrap_or_else(|| user_id.to_string());

        let rows = conn.execute(
            "DELETE FROM mantis_bug_monitor_table WHERE user_id = ?1 AND bug_id = ?2",
            params![user_id, bug_id],
        )?;

        if rows > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let _ = conn.execute(
                "UPDATE mantis_bug_table SET last_updated = ?1 WHERE id = ?2",
                params![now, bug_id],
            );
            let _ = self.record_history(
                &conn,
                bug_id,
                actor_id,
                "monitor_removed",
                &username,
                "",
                13,
            );
        }
        Ok(())
    }

    pub fn remove_bug_monitor(&self, bug_id: i64, user_id: i64) -> rusqlite::Result<()> {
        self.remove_bug_monitor_with_actor(bug_id, user_id, user_id)
    }

    pub fn list_bug_monitor_users(&self, bug_id: i64) -> rusqlite::Result<Vec<User>> {
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
        let rows = stmt.query_map(params![bug_id], Self::map_user_row)?;
        rows.collect()
    }

    fn list_bugs_commented_candidates(
        &self,
        project_id: i64,
        commenter_id: i64,
    ) -> rusqlite::Result<Vec<Bug>> {
        let conn = self.conn();
        if project_id > 0 {
            let mut stmt = conn.prepare(
                "SELECT b.id, b.project_id, b.reporter_id, b.handler_id, b.duplicate_id,
                        b.priority, b.severity, b.reproducibility, b.status, b.resolution,
                        b.category_id, b.date_submitted, b.last_updated, b.os, b.os_build,
                        b.platform, b.version, b.fixed_in_version, b.target_version, b.build,
                        b.summary, b.view_state, b.sticky, b.due_date
                 FROM mantis_bug_table b
                 WHERE b.project_id = ?1
                   AND EXISTS (
                       SELECT 1 FROM mantis_bugnote_table n
                       WHERE n.bug_id = b.id AND n.reporter_id = ?2
                   )
                 ORDER BY b.sticky DESC, b.last_updated DESC, b.id DESC
                 LIMIT 50000",
            )?;
            let rows = stmt.query_map(params![project_id, commenter_id], Self::map_bug_row)?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT b.id, b.project_id, b.reporter_id, b.handler_id, b.duplicate_id,
                        b.priority, b.severity, b.reproducibility, b.status, b.resolution,
                        b.category_id, b.date_submitted, b.last_updated, b.os, b.os_build,
                        b.platform, b.version, b.fixed_in_version, b.target_version, b.build,
                        b.summary, b.view_state, b.sticky, b.due_date
                 FROM mantis_bug_table b
                 WHERE EXISTS (
                     SELECT 1 FROM mantis_bugnote_table n
                     WHERE n.bug_id = b.id AND n.reporter_id = ?1
                 )
                 ORDER BY b.sticky DESC, b.last_updated DESC, b.id DESC
                 LIMIT 50000",
            )?;
            let rows = stmt.query_map(params![commenter_id], Self::map_bug_row)?;
            rows.collect()
        }
    }

    pub fn filter_bugs(&self, filter: &BugFilter, user: &User) -> rusqlite::Result<Vec<Bug>> {
        let commenter_filter = filter.commented_by.filter(|&v| v > 0);
        let load_candidates_for_project = |sub_pid: i64| -> rusqlite::Result<Vec<Bug>> {
            if let Some(commenter_id) = commenter_filter {
                self.list_bugs_commented_candidates(sub_pid, commenter_id)
            } else {
                self.list_bugs(sub_pid, 0, 50_000)
            }
        };

        let all_candidates = if let Some(pid) = filter.project_id {
            if pid > 0 {
                let eff = self.effective_access_level(user, pid)?;
                if eff < 10 {
                    return Ok(Vec::new());
                }
                let pids = self.list_project_descendant_ids(pid)?;
                let mut candidates = Vec::new();
                for sub_pid in pids {
                    candidates.extend(load_candidates_for_project(sub_pid)?);
                }
                candidates.sort_by(|a, b| {
                    b.sticky
                        .cmp(&a.sticky)
                        .then_with(|| b.last_updated.cmp(&a.last_updated))
                        .then_with(|| b.id.cmp(&a.id))
                });
                candidates
            } else {
                load_candidates_for_project(0)?
            }
        } else {
            load_candidates_for_project(0)?
        };

        let search_kw = filter
            .search
            .as_ref()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());

        let mut results = Vec::new();
        for b in all_candidates {
            let eff = self.effective_access_level(user, b.project_id)?;
            if !Self::can_view_bug(user.id, eff, &b) {
                continue;
            }
            if let Some(commenter_id) = commenter_filter {
                if commenter_id != user.id
                    && eff < 55
                    && !self
                        .list_bugnotes(b.id, eff)?
                        .iter()
                        .any(|n| n.reporter_id == commenter_id)
                {
                    continue;
                }
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
                    let monitors = self.list_bug_monitor_users(b.id)?;
                    if !monitors.iter().any(|u| u.id == mid) {
                        continue;
                    }
                }
            }
            if let Some(ref tv) = filter.target_version {
                let tv_trimmed = tv.trim();
                if !tv_trimmed.is_empty() && b.target_version.trim() != tv_trimmed {
                    continue;
                }
            }
            if let Some(ref fv) = filter.fixed_in_version {
                let fv_trimmed = fv.trim();
                if !fv_trimmed.is_empty() && b.fixed_in_version.trim() != fv_trimmed {
                    continue;
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

    fn format_user_display(u: Option<&User>) -> String {
        match u {
            Some(user) => {
                let r = user.realname.trim();
                if !r.is_empty() && r != user.username {
                    format!("{} ({})", user.username, r)
                } else {
                    user.username.clone()
                }
            }
            None => "(Unassigned)".to_string(),
        }
    }

    fn helper_user_by_id(conn: &Connection, id: i64) -> Option<User> {
        if id <= 0 {
            return None;
        }
        Self::query_user_by_id(conn, id).ok().flatten()
    }

    fn helper_project_name(conn: &Connection, id: i64) -> String {
        match Self::query_project_by_id(conn, id) {
            Ok(Some(p)) => p.name,
            _ => format!("Project #{}", id),
        }
    }

    fn helper_category_name(conn: &Connection, category_id: i64) -> String {
        if category_id <= 0 {
            return "General".to_string();
        }
        conn.query_row(
            "SELECT name FROM mantis_category_table WHERE id = ?1",
            params![category_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "General".to_string())
    }

    fn format_transition<F>(old: &str, new: &str, label: F) -> String
    where
        F: Fn(i32) -> &'static str,
    {
        let old_name = old.parse::<i32>().ok().map(&label).unwrap_or(old);
        let new_name = new.parse::<i32>().ok().map(&label).unwrap_or(new);
        format!("{} => {}", old_name, new_name)
    }

    fn format_history_field_change(
        conn: &Connection,
        h: &BugHistory,
        user_cache: &mut HashMap<i64, String>,
        cat_cache: &HashMap<i64, String>,
    ) -> (String, String) {
        if h.type_ == 1 && (h.field_name.is_empty() || h.field_name == "new_bug") {
            return ("New Issue".to_string(), String::new());
        }

        match h.field_name.as_str() {
            "status" => (
                "Status".to_string(),
                Self::format_transition(
                    &h.old_value,
                    &h.new_value,
                    crate::views::bugs::status_label,
                ),
            ),
            "resolution" => (
                "Resolution".to_string(),
                Self::format_transition(
                    &h.old_value,
                    &h.new_value,
                    crate::views::bugs::resolution_label,
                ),
            ),
            "priority" => (
                "Priority".to_string(),
                Self::format_transition(
                    &h.old_value,
                    &h.new_value,
                    crate::views::bugs::priority_label,
                ),
            ),
            "severity" => (
                "Severity".to_string(),
                Self::format_transition(
                    &h.old_value,
                    &h.new_value,
                    crate::views::bugs::severity_label,
                ),
            ),
            "reproducibility" => (
                "Reproducibility".to_string(),
                Self::format_transition(
                    &h.old_value,
                    &h.new_value,
                    crate::views::bugs::reproducibility_label,
                ),
            ),
            "handler_id" => {
                let old_id = h.old_value.parse::<i64>().unwrap_or(0);
                let new_id = h.new_value.parse::<i64>().unwrap_or(0);
                let old_name = if old_id > 0 {
                    user_cache
                        .entry(old_id)
                        .or_insert_with(|| {
                            Self::helper_user_by_id(conn, old_id)
                                .map(|u| u.username)
                                .unwrap_or_else(|| format!("user#{}", old_id))
                        })
                        .clone()
                } else {
                    String::new()
                };
                let new_name = if new_id > 0 {
                    user_cache
                        .entry(new_id)
                        .or_insert_with(|| {
                            Self::helper_user_by_id(conn, new_id)
                                .map(|u| u.username)
                                .unwrap_or_else(|| format!("user#{}", new_id))
                        })
                        .clone()
                } else {
                    String::new()
                };

                let change = if old_name.is_empty() && !new_name.is_empty() {
                    format!("=> {}", new_name)
                } else if !old_name.is_empty() && new_name.is_empty() {
                    format!("{} =>", old_name)
                } else {
                    format!("{} => {}", old_name, new_name)
                };
                ("Assigned To".to_string(), change)
            }
            "file_added" => (format!("File Added: {}", h.new_value), String::new()),
            "file_deleted" => (format!("File Deleted: {}", h.old_value), String::new()),
            "bugnote_added" => {
                let note_id = h.new_value.parse::<i64>().unwrap_or(0);
                (format!("Note Added: {:07}", note_id), String::new())
            }
            "bugnote_updated" => {
                let note_id = h.new_value.parse::<i64>().unwrap_or(0);
                (format!("Note Edited: {:07}", note_id), String::new())
            }
            "bugnote_view_state" => {
                let old_str = if h.old_value == "50" {
                    "private"
                } else {
                    "public"
                };
                let new_str = if h.new_value == "50" {
                    "private"
                } else {
                    "public"
                };
                (
                    "Note Visibility".to_string(),
                    format!("{} => {}", old_str, new_str),
                )
            }
            "category_id" => {
                let old_id = h.old_value.parse::<i64>().unwrap_or(0);
                let new_id = h.new_value.parse::<i64>().unwrap_or(0);
                let old_cat = cat_cache
                    .get(&old_id)
                    .map(|s| s.as_str())
                    .unwrap_or("General");
                let new_cat = cat_cache
                    .get(&new_id)
                    .map(|s| s.as_str())
                    .unwrap_or("General");
                (
                    "Category".to_string(),
                    format!("{} => {}", old_cat, new_cat),
                )
            }
            _ => {
                let field = match h.field_name.as_str() {
                    "summary" => "Summary",
                    "view_state" => "View Status",
                    "target_version" => "Target Version",
                    "fixed_in_version" => "Fixed in Version",
                    "version" => "Product Version",
                    _ => &h.field_name,
                };
                (
                    field.to_string(),
                    format!("{} => {}", h.old_value, h.new_value),
                )
            }
        }
    }

    pub fn query_bug_history_for_email(
        conn: &Connection,
        bug_id: i64,
        max_rows: usize,
    ) -> rusqlite::Result<Vec<(BugHistory, String)>> {
        // Query private note IDs for this bug so they are excluded from the email history table
        let mut private_notes = HashSet::new();
        if let Ok(mut note_stmt) = conn
            .prepare("SELECT id FROM mantis_bugnote_table WHERE bug_id = ?1 AND view_state = 50")
        {
            if let Ok(note_rows) = note_stmt.query_map(params![bug_id], |row| row.get::<_, i64>(0))
            {
                for nid in note_rows.flatten() {
                    private_notes.insert(nid);
                }
            }
        }

        let mut stmt = conn.prepare(
            "SELECT h.id, h.user_id, h.bug_id, h.date_modified, h.field_name, h.old_value, h.new_value, h.type,
                    COALESCE(u.username, '')
             FROM mantis_bug_history_table h
             LEFT JOIN mantis_user_table u ON h.user_id = u.id
             WHERE h.bug_id = ?1
             ORDER BY h.date_modified DESC, h.id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![bug_id, max_rows as i64], |row| {
            Ok((
                BugHistory {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    bug_id: row.get(2)?,
                    date_modified: row.get(3)?,
                    field_name: row.get(4)?,
                    old_value: row.get(5)?,
                    new_value: row.get(6)?,
                    type_: row.get(7)?,
                },
                row.get::<_, String>(8)?,
            ))
        })?;

        let mut result = Vec::new();
        for item in rows.flatten() {
            // Hide private note additions to prevent leaking private note IDs/activity
            if item.0.field_name == "bugnote_added" {
                if let Ok(nid) = item.0.new_value.parse::<i64>() {
                    if private_notes.contains(&nid) {
                        continue;
                    }
                }
            }
            result.push(item);
        }
        result.reverse();
        Ok(result)
    }

    pub fn build_mantis_email_body(
        &self,
        conn: &Connection,
        bug_id: i64,
        action_header: &str,
        current_note: Option<(i64, &str, i64, &str)>,
    ) -> rusqlite::Result<String> {
        let bug = match Self::query_bug_by_id(conn, bug_id)? {
            Some(b) => b,
            None => return Ok(String::new()),
        };

        let (description, steps_to_reproduce, additional_information) = conn
            .query_row(
                "SELECT description, steps_to_reproduce, additional_information
                 FROM mantis_bug_text_table
                 WHERE id = ?1",
                params![bug_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap_or_else(|_| (String::new(), String::new(), String::new()));

        let project_name = Self::helper_project_name(conn, bug.project_id);
        let reporter = Self::helper_user_by_id(conn, bug.reporter_id);
        let reporter_display = Self::format_user_display(reporter.as_ref());

        let handler = if bug.handler_id > 0 {
            Self::helper_user_by_id(conn, bug.handler_id)
        } else {
            None
        };
        let handler_display = handler
            .as_ref()
            .map(|u| Self::format_user_display(Some(u)))
            .unwrap_or_default();

        let category_name = Self::helper_category_name(conn, bug.category_id);
        let repro_str = BugReproducibility::from_i32(bug.reproducibility)
            .map(|r| r.as_str())
            .unwrap_or("always");
        let sev_str = BugSeverity::from_i32(bug.severity)
            .map(|s| s.as_str())
            .unwrap_or("minor");
        let prio_str = BugPriority::from_i32(bug.priority)
            .map(|p| p.as_str())
            .unwrap_or("normal");
        let status_str = BugStatus::from_i32(bug.status)
            .map(|s| s.as_str())
            .unwrap_or("new");
        let res_str = BugResolution::from_i32(bug.resolution)
            .map(|r| r.as_str())
            .unwrap_or("open");

        let date_submitted_str = crate::views::format_timestamp(bug.date_submitted);
        let last_updated_str = crate::views::format_timestamp(bug.last_updated);

        let bug_url = format!(
            "{}/view_bug_page/{}",
            self.base_url.trim_end_matches('/'),
            bug.id
        );

        const SEP: &str = "======================================================================";

        let mut body = String::new();
        body.push_str(action_header);
        body.push('\n');
        body.push_str(SEP);
        body.push('\n');
        body.push_str(&bug_url);
        body.push('\n');
        body.push_str(SEP);
        body.push('\n');

        body.push_str(&format!("{:<28}{}\n", "Reported By:", reporter_display));
        body.push_str(&format!("{:<28}{}\n", "Assigned To:", handler_display));
        body.push_str(SEP);
        body.push('\n');

        body.push_str(&format!("{:<28}{}\n", "Project:", project_name));
        body.push_str(&format!("{:<28}{}\n", "Issue ID:", bug.id));
        body.push_str(&format!("{:<28}{}\n", "Category:", category_name));
        body.push_str(&format!("{:<28}{}\n", "Reproducibility:", repro_str));
        body.push_str(&format!("{:<28}{}\n", "Severity:", sev_str));
        body.push_str(&format!("{:<28}{}\n", "Priority:", prio_str));
        body.push_str(&format!("{:<28}{}\n", "Status:", status_str));
        if bug.resolution > 0 {
            body.push_str(&format!("{:<28}{}\n", "Resolution:", res_str));
        }
        body.push_str(SEP);
        body.push('\n');

        body.push_str(&format!(
            "{:<28}{}\n",
            "Date Submitted:", date_submitted_str
        ));
        body.push_str(&format!("{:<28}{}\n", "Last Modified:", last_updated_str));
        body.push_str(SEP);
        body.push('\n');

        body.push_str(&format!("{:<28}{}\n", "Summary:", bug.summary));
        body.push_str("Description:\n");
        body.push_str(&description);
        body.push('\n');

        if !steps_to_reproduce.trim().is_empty() {
            body.push_str("\nSteps to Reproduce:\n");
            body.push_str(&steps_to_reproduce);
            body.push('\n');
        }

        if !additional_information.trim().is_empty() {
            body.push_str("\nAdditional Information:\n");
            body.push_str(&additional_information);
            body.push('\n');
        }

        if let Some((note_id, author_name, note_date, note_text)) = current_note {
            let note_sep = "----------------------------------------------------------------------";
            let date_str = crate::views::format_timestamp(note_date);
            body.push_str(SEP);
            body.push_str("\n\n");
            body.push_str(note_sep);
            body.push('\n');
            body.push_str(&format!(" ({:07}) {} {}\n", note_id, author_name, date_str));
            body.push_str(note_sep);
            body.push('\n');
            body.push_str(note_text);
            body.push('\n');
        }

        let history = Self::query_bug_history_for_email(conn, bug_id, 100).unwrap_or_default();
        if !history.is_empty() {
            // Pre-fetch categories for the project to avoid per-row queries
            let mut cat_cache = HashMap::new();
            if let Ok(mut cat_stmt) = conn
                .prepare("SELECT id, name FROM mantis_category_table WHERE project_id IN (0, ?1)")
            {
                if let Ok(cat_rows) = cat_stmt.query_map(params![bug.project_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                }) {
                    for (cid, cname) in cat_rows.flatten() {
                        cat_cache.insert(cid, cname);
                    }
                }
            }

            let mut user_cache = HashMap::new();

            body.push_str(SEP);
            body.push_str("\nIssue History\n");
            body.push_str(&format!(
                "{:<17}{:<15}{:<25}{}\n",
                "Date Modified", "Username", "Field", "Change"
            ));
            body.push_str(SEP);
            body.push('\n');

            for (h, username) in history {
                let uname = if !username.is_empty() {
                    &username
                } else {
                    "(Unknown)"
                };
                let date_str = crate::views::format_timestamp(h.date_modified);

                let (field, change) =
                    Self::format_history_field_change(conn, &h, &mut user_cache, &cat_cache);
                if change.is_empty() {
                    body.push_str(&format!("{:<17}{:<15}{}\n", date_str, uname, field));
                } else {
                    body.push_str(&format!(
                        "{:<17}{:<15}{:<25}{}\n",
                        date_str, uname, field, change
                    ));
                }
            }
            body.push_str(SEP);
            body.push('\n');
        }

        Ok(body)
    }

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
        let clean_subject: String = subject
            .chars()
            .filter(|&c| c != '\r' && c != '\n')
            .collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_email_table (email, subject, metadata, body, submitted)
             VALUES (?1, ?2, '', ?3, ?4)",
            params![trimmed, clean_subject, body, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn notify_users(
        &self,
        conn: &Connection,
        bug_id: i64,
        user_ids: &[i64],
        min_access: i32,
        event_subject: &str,
        event_body: &str,
    ) {
        let bug = match Self::query_bug_by_id(conn, bug_id) {
            Ok(Some(b)) => b,
            _ => return,
        };

        let needs_access_check = bug.view_state == 50 || min_access > 10;
        let can_receive = |u: &User| -> bool {
            if u.enabled == 0 || u.email.trim().is_empty() {
                return false;
            }
            if needs_access_check {
                let eff_access =
                    Self::query_effective_access_level(conn, u, bug.project_id).unwrap_or(0);
                if !Self::can_view_bug(u.id, eff_access, &bug) {
                    return false;
                }
                if min_access > 10 && eff_access < min_access {
                    return false;
                }
            }
            true
        };

        let mut recipients = HashSet::new();
        for &uid in user_ids {
            if uid > 0 {
                if let Ok(Some(u)) = Self::query_user_by_id(conn, uid) {
                    if can_receive(&u) {
                        recipients.insert(u.email.trim().to_string());
                    }
                }
            }
        }

        let mut sorted: Vec<String> = recipients.into_iter().collect();
        sorted.sort();
        for addr in sorted {
            let _ = self.enqueue_email(conn, &addr, event_subject, event_body);
        }
    }

    pub fn notify_bug_event(
        &self,
        conn: &Connection,
        bug_id: i64,
        event_subject: &str,
        event_body: &str,
    ) {
        self.notify_bug_event_filtered(conn, bug_id, 0, &[], event_subject, event_body);
    }

    pub fn notify_bug_event_filtered(
        &self,
        conn: &Connection,
        bug_id: i64,
        min_access: i32,
        extra_recipients: &[i64],
        event_subject: &str,
        event_body: &str,
    ) {
        self.notify_bug_event_filtered_with_exclude(
            conn,
            bug_id,
            min_access,
            extra_recipients,
            &[],
            event_subject,
            event_body,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn notify_bug_event_filtered_with_exclude(
        &self,
        conn: &Connection,
        bug_id: i64,
        min_access: i32,
        extra_recipients: &[i64],
        exclude_usernames: &[String],
        event_subject: &str,
        event_body: &str,
    ) {
        let bug = match Self::query_bug_by_id(conn, bug_id) {
            Ok(Some(b)) => b,
            _ => return,
        };

        let exclude_set: HashSet<String> = exclude_usernames
            .iter()
            .map(|u| u.to_ascii_lowercase())
            .collect();

        let needs_access_check = bug.view_state == 50 || min_access > 10;
        let can_receive = |u: &User| -> bool {
            if u.enabled == 0 || u.email.trim().is_empty() {
                return false;
            }
            if exclude_set.contains(&u.username.to_ascii_lowercase()) {
                return false;
            }
            if needs_access_check {
                let eff_access =
                    Self::query_effective_access_level(conn, u, bug.project_id).unwrap_or(0);
                if !Self::can_view_bug(u.id, eff_access, &bug) {
                    return false;
                }
                if min_access > 10 && eff_access < min_access {
                    return false;
                }
            }
            true
        };

        let mut recipients = HashSet::new();
        if let Ok(Some(reporter)) = Self::query_user_by_id(conn, bug.reporter_id) {
            if can_receive(&reporter) {
                recipients.insert(reporter.email.trim().to_string());
            }
        }
        if bug.handler_id > 0 {
            if let Ok(Some(handler)) = Self::query_user_by_id(conn, bug.handler_id) {
                if can_receive(&handler) {
                    recipients.insert(handler.email.trim().to_string());
                }
            }
        }

        // Include any extra recipients (such as previous handler during reassignment)
        for &uid in extra_recipients {
            if uid > 0 {
                if let Ok(Some(extra_u)) = Self::query_user_by_id(conn, uid) {
                    if can_receive(&extra_u) {
                        recipients.insert(extra_u.email.trim().to_string());
                    }
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare(
            "SELECT u.id, u.username, u.realname, u.email, u.password, u.enabled, u.protected,
                    u.access_level, u.login_count, u.lost_password_request_count, u.failed_login_count,
                    u.cookie_string, u.last_visit, u.date_created
             FROM mantis_bug_monitor_table bm
             JOIN mantis_user_table u ON bm.user_id = u.id
             WHERE bm.bug_id = ?1 AND u.enabled != 0",
        ) {
            if let Ok(rows) = stmt.query_map(params![bug_id], Self::map_user_row) {
                for u in rows.flatten() {
                    if can_receive(&u) {
                        recipients.insert(u.email.trim().to_string());
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

    pub fn notify_bug_mentions(
        &self,
        bug_id: i64,
        actor_id: i64,
        mentioned_usernames: &[String],
        is_private_note: bool,
        context_text: &str,
    ) -> rusqlite::Result<()> {
        if mentioned_usernames.is_empty() {
            return Ok(());
        }
        let conn = self.conn();
        self.notify_bug_mentions_with_conn(
            &conn,
            bug_id,
            actor_id,
            mentioned_usernames,
            is_private_note,
            context_text,
        )
    }

    pub fn notify_bug_mentions_with_conn(
        &self,
        conn: &Connection,
        bug_id: i64,
        actor_id: i64,
        mentioned_usernames: &[String],
        is_private_note: bool,
        context_text: &str,
    ) -> rusqlite::Result<()> {
        if mentioned_usernames.is_empty() {
            return Ok(());
        }
        let bug = match Self::query_bug_by_id(conn, bug_id)? {
            Some(b) => b,
            None => return Ok(()),
        };
        let actor_name = Self::query_user_by_id(conn, actor_id)?
            .map(|u| u.username)
            .unwrap_or_else(|| "(Unknown)".to_string());
        let project_name = Self::helper_project_name(conn, bug.project_id);
        let subject = format!(
            "[{} {:07}] Mentioned by {}: {}",
            project_name, bug.id, actor_name, bug.summary
        );
        let truncated_context = if context_text.len() > 2000 {
            let mut end = 2000;
            while !context_text.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            format!("{}...", &context_text[..end])
        } else {
            context_text.to_string()
        };
        let body = format!(
            "You were mentioned by {} in issue #{:07} ({}).\n\n{}\n\nView issue: {}/view_bug_page/{}",
            actor_name, bug.id, bug.summary, truncated_context, self.base_url, bug.id
        );

        let mut notified_emails = HashSet::new();

        for uname in mentioned_usernames.iter().take(30) {
            let user_res = conn.query_row(
                "SELECT id, username, realname, email, password, enabled, protected,
                        access_level, login_count, lost_password_request_count, failed_login_count,
                        cookie_string, last_visit, date_created
                 FROM mantis_user_table
                 WHERE LOWER(username) = LOWER(?1) AND enabled != 0",
                params![uname],
                Self::map_user_row,
            );

            let u = match user_res {
                Ok(u) => u,
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => {
                    eprintln!(
                        "[mentions] error querying user for mention '{}': {}",
                        uname, e
                    );
                    continue;
                }
            };

            if u.id == actor_id {
                continue;
            }
            let email = u.email.trim();
            let email_lower = email.to_ascii_lowercase();
            if email.is_empty() || notified_emails.contains(&email_lower) {
                continue;
            }
            let eff = Self::query_effective_access_level(conn, &u, bug.project_id).unwrap_or(0);
            if !Self::can_view_bug(u.id, eff, &bug) {
                continue;
            }
            if is_private_note && eff < crate::models::ACCESS_DEVELOPER {
                continue;
            }
            if let Err(e) = self.enqueue_email(conn, email, &subject, &body) {
                eprintln!(
                    "[mailer] failed to enqueue mention email to {}: {}",
                    email, e
                );
            } else {
                notified_emails.insert(email_lower);
            }
        }
        Ok(())
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

    pub fn list_bugs_commented_by_user(
        &self,
        user: &User,
        project_id: Option<i64>,
    ) -> rusqlite::Result<Vec<Bug>> {
        self.filter_bugs(
            &BugFilter {
                project_id,
                commented_by: Some(user.id),
                ..Default::default()
            },
            user,
        )
    }

    /// Returns `(history_counts, resolved_at_map)` in two chunked parameterized queries.
    pub fn get_bug_history_summary_stats(
        &self,
        bug_ids: &[i64],
    ) -> rusqlite::Result<(HashMap<i64, usize>, HashMap<i64, i64>)> {
        let mut history_counts = HashMap::new();
        let mut resolved_at_map = HashMap::new();
        if bug_ids.is_empty() {
            return Ok((history_counts, resolved_at_map));
        }

        let conn = self.conn();
        for chunk in bug_ids.chunks(200) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(",");
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

            let count_sql = format!(
                "SELECT bug_id, COUNT(*) FROM mantis_bug_history_table WHERE bug_id IN ({}) GROUP BY bug_id",
                in_clause
            );
            let mut count_stmt = conn.prepare(&count_sql)?;
            let count_rows = count_stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in count_rows {
                let (bid, cnt) = r?;
                history_counts.insert(bid, cnt.max(0) as usize);
            }

            let res_sql = format!(
                "SELECT bug_id, MAX(date_modified)
                 FROM mantis_bug_history_table
                 WHERE bug_id IN ({}) AND field_name = 'status' AND CAST(new_value AS INTEGER) >= 80
                 GROUP BY bug_id",
                in_clause
            );
            let mut res_stmt = conn.prepare(&res_sql)?;
            let res_rows = res_stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in res_rows {
                let (bid, ts) = r?;
                if ts > 0 {
                    resolved_at_map.insert(bid, ts);
                }
            }
        }

        Ok((history_counts, resolved_at_map))
    }

    pub fn list_time_tracking(&self, project_id: i64) -> rusqlite::Result<Vec<TimeTrackingEntry>> {
        let conn = self.conn();
        if project_id > 0 {
            let mut stmt = conn.prepare(
                "SELECT n.bug_id, b.summary, n.reporter_id, COALESCE(u.username, 'unknown'),
                        n.time_tracking, n.date_submitted, COALESCE(t.note, ''),
                        b.project_id, COALESCE(p.name, '')
                 FROM mantis_bugnote_table n
                 JOIN mantis_bug_table b ON n.bug_id = b.id
                 LEFT JOIN mantis_project_table p ON b.project_id = p.id
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
                    project_id: row.get(7)?,
                    project_name: row.get(8)?,
                })
            })?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT n.bug_id, b.summary, n.reporter_id, COALESCE(u.username, 'unknown'),
                        n.time_tracking, n.date_submitted, COALESCE(t.note, ''),
                        b.project_id, COALESCE(p.name, '')
                 FROM mantis_bugnote_table n
                 JOIN mantis_bug_table b ON n.bug_id = b.id
                 LEFT JOIN mantis_project_table p ON b.project_id = p.id
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
                    project_id: row.get(7)?,
                    project_name: row.get(8)?,
                })
            })?;
            rows.collect()
        }
    }

    pub fn list_time_tracking_filtered(
        &self,
        user: &User,
        project_id: i64,
        include_subprojects: bool,
        from_ts: Option<i64>,
        to_ts: Option<i64>,
    ) -> rusqlite::Result<Vec<TimeTrackingEntry>> {
        let mut target_pids = Vec::new();
        if project_id > 0 {
            let root_enabled =
                matches!(self.get_project_by_id(project_id)?, Some(p) if p.enabled == 1);
            if !root_enabled || self.effective_access_level(user, project_id)? < 55 {
                return Ok(Vec::new());
            }
            if include_subprojects {
                for sub_pid in self.list_project_descendant_ids(project_id)? {
                    let enabled =
                        matches!(self.get_project_by_id(sub_pid)?, Some(p) if p.enabled == 1);
                    if enabled && self.effective_access_level(user, sub_pid)? >= 55 {
                        target_pids.push(sub_pid);
                    }
                }
            } else {
                target_pids.push(project_id);
            }
        } else {
            for (p, _) in self.list_visible_projects(user)? {
                if p.enabled == 1 && self.effective_access_level(user, p.id)? >= 55 {
                    target_pids.push(p.id);
                }
            }
        }

        if target_pids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn();
        let mut entries = Vec::new();
        for chunk in target_pids.chunks(200) {
            let mut next_idx = chunk.len() + 1;
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let mut sql = format!(
                "SELECT n.bug_id, b.summary, n.reporter_id, COALESCE(u.username, 'unknown'),
                        n.time_tracking, n.date_submitted, COALESCE(t.note, ''),
                        b.project_id, COALESCE(p.name, '')
                 FROM mantis_bugnote_table n
                 JOIN mantis_bug_table b ON n.bug_id = b.id
                 LEFT JOIN mantis_project_table p ON b.project_id = p.id
                 LEFT JOIN mantis_user_table u ON n.reporter_id = u.id
                 LEFT JOIN mantis_bugnote_text_table t ON n.bugnote_text_id = t.id
                 WHERE n.time_tracking > 0 AND b.project_id IN ({})",
                placeholders.join(",")
            );
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = chunk
                .iter()
                .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>)
                .collect();
            if let Some(ft) = from_ts {
                sql.push_str(&format!(" AND n.date_submitted >= ?{}", next_idx));
                params.push(Box::new(ft));
                next_idx += 1;
            }
            if let Some(tt) = to_ts {
                sql.push_str(&format!(" AND n.date_submitted <= ?{}", next_idx));
                params.push(Box::new(tt));
            }
            sql.push_str(" ORDER BY n.date_submitted DESC, n.id DESC");

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(TimeTrackingEntry {
                    bug_id: row.get(0)?,
                    bug_summary: row.get(1)?,
                    user_id: row.get(2)?,
                    username: row.get(3)?,
                    minutes: row.get(4)?,
                    date_submitted: row.get(5)?,
                    note: row.get(6)?,
                    project_id: row.get(7)?,
                    project_name: row.get(8)?,
                })
            })?;
            for r in rows {
                entries.push(r?);
            }
        }

        entries.sort_by(|a, b| {
            b.date_submitted
                .cmp(&a.date_submitted)
                .then_with(|| b.bug_id.cmp(&a.bug_id))
        });
        Ok(entries)
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

    pub fn get_news(&self, id: i64) -> rusqlite::Result<Option<NewsItem>> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, project_id, poster_id, date_posted, last_modified,
                    view_state, announcement, headline, body
             FROM mantis_news_table
             WHERE id = ?1",
            params![id],
            |row| {
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
            },
        ) {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_news(
        &self,
        id: i64,
        headline: &str,
        body: &str,
        view_state: i32,
        announcement: i32,
    ) -> Result<(), String> {
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
        let updated = conn
            .execute(
                "UPDATE mantis_news_table
                 SET headline = ?1, body = ?2, view_state = ?3, announcement = ?4, last_modified = ?5
                 WHERE id = ?6",
                params![trimmed_headline, trimmed_body, view_state, announcement, now, id],
            )
            .map_err(|e| e.to_string())?;
        if updated == 0 {
            return Err(format!("News item {} not found", id));
        }
        Ok(())
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

    pub fn list_all_api_tokens(&self) -> rusqlite::Result<Vec<(ApiToken, String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.user_id, t.name, t.hash, t.date_created, t.date_used,
                    COALESCE(u.username, ''), COALESCE(u.realname, '')
             FROM mantis_api_token_table t
             LEFT JOIN mantis_user_table u ON t.user_id = u.id
             ORDER BY t.date_created DESC, t.id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let token = ApiToken {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                hash: row.get(3)?,
                date_created: row.get(4)?,
                date_used: row.get(5)?,
            };
            let username: String = row.get(6)?;
            let realname: String = row.get(7)?;
            Ok((token, username, realname))
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

    pub fn revoke_api_token_by_id(&self, id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "DELETE FROM mantis_api_token_table WHERE id = ?1",
            params![id],
        )?;
        Ok(rows > 0)
    }

    pub fn authenticate_api_token(&self, raw_token: &str) -> rusqlite::Result<Option<User>> {
        Ok(self.authenticate_api_token_full(raw_token)?.map(|(_, u)| u))
    }

    pub fn authenticate_api_token_full(
        &self,
        raw_token: &str,
    ) -> rusqlite::Result<Option<(i64, User)>> {
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

        Ok(Some((token_id, user)))
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

    fn set_plugin_enabled_internal(
        &self,
        basename: &str,
        enabled: bool,
        allow_toggleable_core: bool,
    ) -> Result<(), String> {
        let conn = self.conn();
        let protected: i32 = conn
            .query_row(
                "SELECT protected FROM mantis_plugin_table WHERE basename = ?1",
                params![basename],
                |row| row.get(0),
            )
            .map_err(|_| format!("Plugin '{}' not found", basename))?;
        if protected != 0
            && !enabled
            && (!allow_toggleable_core
                || !crate::models::TOGGLEABLE_CORE_PLUGINS.contains(&basename))
        {
            return Err(format!(
                "Protected plugin '{}' cannot be disabled",
                basename
            ));
        }
        conn.execute(
            "UPDATE mantis_plugin_table SET enabled = ?1 WHERE basename = ?2",
            params![if enabled { 1 } else { 0 }, basename],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_plugin_enabled(&self, basename: &str, enabled: bool) -> Result<(), String> {
        self.set_plugin_enabled_internal(basename, enabled, false)
    }

    pub fn set_plugin_enabled_admin(&self, basename: &str, enabled: bool) -> Result<(), String> {
        self.set_plugin_enabled_internal(basename, enabled, true)
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

    // ─── Jira Integration (`mantis_jira_links_table`) ────────────────────────

    pub fn create_jira_link(
        &self,
        bug_id: i64,
        jira_key: &str,
        jira_url: &str,
        issue_type: &str,
        jira_status: &str,
        user_id: i64,
    ) -> rusqlite::Result<i64> {
        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO mantis_jira_links_table (bug_id, jira_key, jira_url, issue_type, jira_status, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![bug_id, jira_key, jira_url, issue_type, jira_status, user_id, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_jira_link_by_bug_id(&self, bug_id: i64) -> rusqlite::Result<Option<JiraLink>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, bug_id, jira_key, jira_url, issue_type, jira_status, created_by, created_at
             FROM mantis_jira_links_table WHERE bug_id = ?1 AND jira_key != '__PENDING__'",
        )?;
        let mut rows = stmt.query(params![bug_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(JiraLink {
                id: row.get(0)?,
                bug_id: row.get(1)?,
                jira_key: row.get(2)?,
                jira_url: row.get(3)?,
                issue_type: row.get(4)?,
                jira_status: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn claim_jira_link_pending(&self, bug_id: i64, user_id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let affected = conn.execute(
            "INSERT INTO mantis_jira_links_table (bug_id, jira_key, jira_url, issue_type, jira_status, created_by, created_at)
             VALUES (?1, '__PENDING__', '', '', '', ?2, ?3)
             ON CONFLICT(bug_id) DO UPDATE SET
                 created_by = excluded.created_by,
                 created_at = excluded.created_at
             WHERE mantis_jira_links_table.jira_key = '__PENDING__' AND (excluded.created_at - mantis_jira_links_table.created_at) > 60",
            params![bug_id, user_id, now],
        )?;
        Ok(affected > 0)
    }

    pub fn release_jira_link_pending(&self, bug_id: i64) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM mantis_jira_links_table WHERE bug_id = ?1 AND jira_key = '__PENDING__'",
            params![bug_id],
        )?;
        Ok(())
    }

    pub fn delete_jira_link_by_bug_id(&self, bug_id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let affected = conn.execute(
            "DELETE FROM mantis_jira_links_table WHERE bug_id = ?1",
            params![bug_id],
        )?;
        Ok(affected > 0)
    }

    fn resolve_jira_status_internal(conn: &rusqlite::Connection, jira_status: &str) -> i32 {
        let trimmed = jira_status.trim();
        if trimmed.is_empty() {
            return 0;
        }
        let mapped: Option<i32> = conn
            .query_row(
                "SELECT mantis_status FROM mantis_jira_status_mapping_table WHERE jira_status = ?1",
                params![trimmed],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);
        mapped.unwrap_or_else(|| crate::jira::map_jira_status_to_mantis(trimmed))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_jira_sync_success(
        &self,
        bug_id: i64,
        jira_key: &str,
        jira_url: &str,
        issue_type: &str,
        jira_status: &str,
        user_id: i64,
        username: &str,
        is_new_link: bool,
    ) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Query existing link to track changes and previous status
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT jira_key, jira_status FROM mantis_jira_links_table WHERE bug_id = ?1 AND jira_key != '__PENDING__'",
                params![bug_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let (existing_key, existing_status) = match existing {
            Some((k, s)) => (k, s),
            None => (String::new(), String::new()),
        };

        // 1. Insert or update jira link via UPSERT to prevent concurrency duplicate inserts
        tx.execute(
            "INSERT INTO mantis_jira_links_table (bug_id, jira_key, jira_url, issue_type, jira_status, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(bug_id) DO UPDATE SET
                 jira_key = excluded.jira_key,
                 jira_url = excluded.jira_url,
                 issue_type = excluded.issue_type,
                 jira_status = excluded.jira_status,
                 created_by = CASE WHEN mantis_jira_links_table.jira_key = '__PENDING__' THEN excluded.created_by ELSE mantis_jira_links_table.created_by END,
                 created_at = CASE WHEN mantis_jira_links_table.jira_key = '__PENDING__' THEN excluded.created_at ELSE mantis_jira_links_table.created_at END",
            params![bug_id, jira_key, jira_url, issue_type, jira_status, user_id, now],
        )
        .map_err(|e| e.to_string())?;

        // 2. Map status to mantis and update bug if status changes
        let old_status: i32 = tx
            .query_row(
                "SELECT status FROM mantis_bug_table WHERE id = ?1",
                params![bug_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let trimmed_jira_status = jira_status.trim();
        let mut status_changed = false;
        if !trimmed_jira_status.is_empty() {
            let mapped_status = Self::resolve_jira_status_internal(&tx, trimmed_jira_status);
            // Prevent reopening/downgrading resolved or closed bugs (status >= 80) if mapped status is lower
            let is_resolved_or_closed_downgrade = old_status >= 80 && mapped_status < old_status;
            if !is_resolved_or_closed_downgrade && old_status != mapped_status {
                tx.execute(
                    "UPDATE mantis_bug_table SET status = ?1, last_updated = ?2 WHERE id = ?3",
                    params![mapped_status, now, bug_id],
                )
                .map_err(|e| e.to_string())?;

                tx.execute(
                    "INSERT INTO mantis_bug_history_table (user_id, bug_id, date_modified, field_name, old_value, new_value, type)
                     VALUES (?1, ?2, ?3, 'status', ?4, ?5, 0)",
                    params![user_id, bug_id, now, old_status.to_string(), mapped_status.to_string()],
                )
                .map_err(|e| e.to_string())?;

                status_changed = true;
            }
        }

        // 3. Insert bugnote ONLY if something changed or it's a new link
        let key_changed = existing_key != jira_key;
        let jira_status_changed = existing_status != jira_status;
        let should_add_note = is_new_link || status_changed || key_changed || jira_status_changed;

        if should_add_note {
            let note_text = if is_new_link {
                format!(
                    "Linked to Jira issue [{}]({}) (Status: {}) by {}.",
                    jira_key, jira_url, jira_status, username
                )
            } else {
                format!(
                    "Synced with Jira issue [{}]({}) (Status: {}) by {}.",
                    jira_key, jira_url, jira_status, username
                )
            };

            tx.execute(
                "INSERT INTO mantis_bugnote_text_table (note) VALUES (?1)",
                params![note_text],
            )
            .map_err(|e| e.to_string())?;
            let text_id = tx.last_insert_rowid();

            tx.execute(
                "INSERT INTO mantis_bugnote_table (bug_id, reporter_id, bugnote_text_id, view_state, date_submitted, last_modified, note_type, note_attr, time_tracking)
                 VALUES (?1, ?2, ?3, 10, ?4, ?4, 0, '', 0)",
                params![bug_id, user_id, text_id, now],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_jira_link_with_note(
        &self,
        bug_id: i64,
        user_id: i64,
        username: &str,
    ) -> Result<bool, String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let existing_key: Option<String> = tx
            .query_row(
                "SELECT jira_key FROM mantis_jira_links_table WHERE bug_id = ?1 AND jira_key != '__PENDING__'",
                params![bug_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let key_str = match existing_key {
            Some(k) => k,
            None => return Ok(false),
        };

        let affected = tx
            .execute(
                "DELETE FROM mantis_jira_links_table WHERE bug_id = ?1",
                params![bug_id],
            )
            .map_err(|e| e.to_string())?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let note_text = format!("Unlinked Jira issue {} by {}.", key_str, username);
        tx.execute(
            "INSERT INTO mantis_bugnote_text_table (note) VALUES (?1)",
            params![note_text],
        )
        .map_err(|e| e.to_string())?;
        let text_id = tx.last_insert_rowid();

        tx.execute(
            "INSERT INTO mantis_bugnote_table (bug_id, reporter_id, bugnote_text_id, view_state, date_submitted, last_modified, note_type, note_attr, time_tracking)
             VALUES (?1, ?2, ?3, 10, ?4, ?4, 0, '', 0)",
            params![bug_id, user_id, text_id, now],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }

    pub fn resolve_jira_api_token(env_token: Option<&str>, db_token: Option<String>) -> String {
        if let Some(tok) = env_token {
            let trimmed = tok.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        db_token.unwrap_or_default()
    }

    pub fn get_jira_config(&self, project_id: i64) -> rusqlite::Result<JiraResolvedConfig> {
        let enabled_val = self
            .get_config("jira_enabled", 0, 0)?
            .unwrap_or_else(|| "0".to_string());
        let enabled = enabled_val == "1" || enabled_val.eq_ignore_ascii_case("true");
        let base_url = self.get_config("jira_base_url", 0, 0)?.unwrap_or_default();
        let email = self.get_config("jira_email", 0, 0)?.unwrap_or_default();
        let env_token = std::env::var("MANTIS_JIRA_API_TOKEN").ok();
        let db_token = self.get_config("jira_api_token", 0, 0)?;
        let api_token = Self::resolve_jira_api_token(env_token.as_deref(), db_token);
        let default_issue_type = self
            .get_config("jira_default_issue_type", 0, 0)?
            .unwrap_or_else(|| "Bug".to_string());
        let default_project_key = self
            .get_config("jira_default_project_key", 0, 0)?
            .unwrap_or_default();

        let mut project_key = String::new();
        if project_id > 0 {
            if let Some(pk) = self.get_config("jira_project_key", project_id, 0)? {
                project_key = pk;
            }
        }
        if project_key.is_empty() {
            project_key = default_project_key;
        }

        Ok(JiraResolvedConfig {
            enabled,
            base_url,
            email,
            api_token,
            default_issue_type,
            project_key,
        })
    }

    pub fn save_jira_global_config(
        &self,
        enabled: bool,
        base_url: &str,
        email: &str,
        api_token: Option<&str>,
        default_issue_type: &str,
        default_project_key: &str,
    ) -> rusqlite::Result<()> {
        self.set_config("jira_enabled", 0, 0, if enabled { "1" } else { "0" })?;
        self.set_config("jira_base_url", 0, 0, base_url)?;
        self.set_config("jira_email", 0, 0, email)?;
        if let Some(tok) = api_token {
            if !tok.is_empty() {
                self.set_config("jira_api_token", 0, 0, tok)?;
            }
        }
        self.set_config("jira_default_issue_type", 0, 0, default_issue_type)?;
        self.set_config("jira_default_project_key", 0, 0, default_project_key)?;
        Ok(())
    }

    pub fn set_jira_project_mapping(
        &self,
        project_id: i64,
        jira_project_key: &str,
    ) -> rusqlite::Result<()> {
        self.set_config("jira_project_key", project_id, 0, jira_project_key)
    }

    pub fn save_jira_epics_cache(
        &self,
        project_key: &str,
        epics: &[crate::models::JiraEpic],
    ) -> rusqlite::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.save_jira_epics_cache_with_ttl(project_key, epics, now, now + 86400)
    }

    pub fn save_jira_epics_cache_with_timestamp(
        &self,
        project_key: &str,
        epics: &[crate::models::JiraEpic],
        updated_at: i64,
    ) -> rusqlite::Result<()> {
        self.save_jira_epics_cache_with_ttl(project_key, epics, updated_at, updated_at + 86400)
    }

    pub fn save_jira_epics_cache_with_ttl(
        &self,
        project_key: &str,
        epics: &[crate::models::JiraEpic],
        updated_at: i64,
        expires_at: i64,
    ) -> rusqlite::Result<()> {
        let epics_json = serde_json::to_string(epics).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn();
        conn.execute(
            "INSERT INTO mantis_jira_epics_cache_table (project_key, epics_json, updated_at, expires_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_key) DO UPDATE SET epics_json = excluded.epics_json, updated_at = excluded.updated_at, expires_at = excluded.expires_at",
            params![project_key, epics_json, updated_at, expires_at],
        )?;
        Ok(())
    }

    pub fn get_cached_jira_epics(
        &self,
        project_key: &str,
        max_age_secs: i64,
    ) -> rusqlite::Result<Option<Vec<crate::models::JiraEpic>>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn();
        let res: Option<(String, i64, i64)> = conn
            .query_row(
                "SELECT epics_json, updated_at, expires_at FROM mantis_jira_epics_cache_table WHERE project_key = ?1",
                params![project_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        match res {
            Some((json, updated_at, expires_at)) => {
                let is_fresh = if expires_at > 0 {
                    now < expires_at
                } else {
                    now.saturating_sub(updated_at) <= max_age_secs
                };
                if is_fresh || max_age_secs == i64::MAX {
                    let epics: Vec<crate::models::JiraEpic> =
                        serde_json::from_str(&json).unwrap_or_default();
                    Ok(Some(epics))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    pub fn list_cached_jira_epics_summary(&self) -> rusqlite::Result<Vec<(String, usize, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT project_key, epics_json, updated_at FROM mantis_jira_epics_cache_table ORDER BY project_key ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let json: String = row.get(1)?;
            let updated_at: i64 = row.get(2)?;
            let count = serde_json::from_str::<Vec<crate::models::JiraEpic>>(&json)
                .map(|v| v.len())
                .unwrap_or(0);
            Ok((key, count, updated_at))
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_jira_status_mappings(&self) -> rusqlite::Result<Vec<(String, i32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT jira_status, mantis_status FROM mantis_jira_status_mapping_table ORDER BY mantis_status ASC, jira_status ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let j: String = row.get(0)?;
            let m: i32 = row.get(1)?;
            Ok((j, m))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn save_jira_status_mappings(&self, mappings: &[(String, i32)]) -> rusqlite::Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM mantis_jira_status_mapping_table", [])?;
        let mut seen = std::collections::HashSet::new();
        for (j_status, m_status) in mappings {
            let trimmed = j_status.trim();
            if !trimmed.is_empty() {
                let lower = trimmed.to_lowercase();
                if seen.insert(lower) {
                    tx.execute(
                        "INSERT INTO mantis_jira_status_mapping_table (jira_status, mantis_status) VALUES (?1, ?2)",
                        params![trimmed, m_status],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn resolve_jira_status_to_mantis(&self, jira_status: &str) -> i32 {
        Self::resolve_jira_status_internal(&self.conn(), jira_status)
    }
}

#[cfg(test)]
#[path = "db_test.rs"]
mod tests;
