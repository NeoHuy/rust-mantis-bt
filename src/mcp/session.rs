use crate::models::User;
use axum::response::sse::Event;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

pub struct McpSession {
    pub id: String,
    pub token_id: i64,
    pub user_id: i64,
    pub user: User,
    pub sender: mpsc::Sender<Event>,
    pub last_active: Instant,
}

#[derive(Clone, Default)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, McpSession>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(
        &self,
        token_id: i64,
        user: User,
    ) -> (String, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel(32);
        let id = format!(
            "{:016x}{:016x}",
            rand::random::<u64>(),
            rand::random::<u64>()
        );
        let session = McpSession {
            id: id.clone(),
            token_id,
            user_id: user.id,
            user,
            sender: tx,
            last_active: Instant::now(),
        };
        let mut map = self.sessions.write().await;
        map.insert(id.clone(), session);
        (id, rx)
    }

    /// Fetches the user after re-validating that the token still exists in the database
    /// and that the user account is still enabled. If revoked or disabled, automatically
    /// terminates the session and returns None.
    pub async fn get_user_validated(&self, db: &crate::db::Db, session_id: &str) -> Option<User> {
        let (token_id, user_id) = {
            let mut map = self.sessions.write().await;
            let session = map.get_mut(session_id)?;
            session.last_active = Instant::now();
            (session.token_id, session.user_id)
        };

        // Re-validate against database using a blocking task to avoid stalling executor
        let db_clone = db.clone();
        let is_valid = tokio::task::spawn_blocking(move || {
            let token_exists = {
                let conn = db_clone.conn();
                conn.query_row(
                    "SELECT 1 FROM mantis_api_token_table WHERE id = ?1 AND user_id = ?2",
                    rusqlite::params![token_id, user_id],
                    |_| Ok(()),
                )
                .is_ok()
            };
            if !token_exists {
                return None;
            }
            match db_clone.get_user_by_id(user_id) {
                Ok(Some(u)) if u.enabled == 1 => Some(u),
                _ => None,
            }
        })
        .await
        .ok()
        .flatten();

        if let Some(fresh_user) = is_valid {
            let mut map = self.sessions.write().await;
            if let Some(session) = map.get_mut(session_id) {
                session.user = fresh_user.clone();
            }
            Some(fresh_user)
        } else {
            // Token was revoked or user was disabled! Eject session immediately
            let _ = self.remove_session(session_id).await;
            None
        }
    }

    pub async fn get_user(&self, session_id: &str) -> Option<User> {
        let mut map = self.sessions.write().await;
        if let Some(session) = map.get_mut(session_id) {
            session.last_active = Instant::now();
            Some(session.user.clone())
        } else {
            None
        }
    }

    pub async fn send_event(&self, session_id: &str, event: Event) -> Result<(), String> {
        // Clone the sender and drop the lock BEFORE awaiting send to prevent stalling
        let sender = {
            let map = self.sessions.read().await;
            map.get(session_id).map(|s| s.sender.clone())
        };

        if let Some(sender) = sender {
            sender
                .send(event)
                .await
                .map_err(|e| format!("Failed to send SSE event: {}", e))
        } else {
            Err("Session not found".to_string())
        }
    }

    pub async fn remove_session(&self, session_id: &str) -> Option<User> {
        let mut map = self.sessions.write().await;
        map.remove(session_id).map(|s| s.user)
    }

    pub async fn cleanup_stale(&self, _timeout: Duration) -> usize {
        let mut map = self.sessions.write().await;
        let now = Instant::now();
        let initial_count = map.len();
        map.retain(|_, session| {
            if session.sender.is_closed() {
                false
            } else {
                // If stream is still open, keep it alive up to 24 hours of total idle time
                now.duration_since(session.last_active) < Duration::from_secs(86400)
            }
        });
        initial_count - map.len()
    }

    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    fn dummy_user(id: i64, username: &str) -> User {
        User {
            id,
            username: username.to_string(),
            realname: username.to_string(),
            email: format!("{}@example.com", username),
            password: "hash".to_string(),
            enabled: 1,
            protected: 0,
            access_level: 25,
            login_count: 1,
            lost_password_request_count: 0,
            failed_login_count: 0,
            cookie_string: "cookie".to_string(),
            last_visit: 0,
            date_created: 0,
        }
    }

    #[tokio::test]
    async fn test_session_lifecycle_and_channel() {
        let manager = SessionManager::new();
        let user = dummy_user(10, "alice");

        let (session_id, mut rx) = manager.create_session(1, user.clone()).await;
        assert_eq!(manager.count().await, 1);

        // Get user back
        let retrieved = manager.get_user(&session_id).await.unwrap();
        assert_eq!(retrieved.id, 10);
        assert_eq!(retrieved.username, "alice");

        // Send an event
        manager
            .send_event(&session_id, Event::default().data("hello"))
            .await
            .unwrap();
        let received = rx.recv().await;
        assert!(received.is_some());

        // Remove session
        let removed = manager.remove_session(&session_id).await;
        assert_eq!(removed.unwrap().id, 10);
        assert_eq!(manager.count().await, 0);
        assert!(manager.get_user(&session_id).await.is_none());
    }

    #[tokio::test]
    async fn test_session_cleanup_stale() {
        let manager = SessionManager::new();
        let user = dummy_user(20, "bob");
        let (session_id, rx) = manager.create_session(2, user).await;

        // Cleanup while SSE connection is active shouldn't remove it even after time passes
        let cleaned = manager.cleanup_stale(Duration::from_secs(3600)).await;
        assert_eq!(cleaned, 0);
        assert_eq!(manager.count().await, 1);

        // When the client disconnects (SSE receiver dropped), cleanup_stale detects is_closed and evicts it
        drop(rx);
        let cleaned = manager.cleanup_stale(Duration::from_secs(0)).await;
        assert_eq!(cleaned, 1);
        assert_eq!(manager.count().await, 0);
        assert!(manager.get_user(&session_id).await.is_none());
    }
}
