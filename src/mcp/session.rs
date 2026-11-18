use crate::models::User;
use axum::response::sse::Event;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

pub struct McpSession {
    pub id: String,
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
        user: User,
    ) -> (String, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel(32);
        let id = format!("{:016x}{:016x}", rand::random::<u64>(), rand::random::<u64>());
        let session = McpSession {
            id: id.clone(),
            user,
            sender: tx,
            last_active: Instant::now(),
        };
        let mut map = self.sessions.write().await;
        map.insert(id.clone(), session);
        (id, rx)
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
        let map = self.sessions.read().await;
        if let Some(session) = map.get(session_id) {
            session
                .sender
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

    pub async fn cleanup_stale(&self, timeout: Duration) -> usize {
        let mut map = self.sessions.write().await;
        let now = Instant::now();
        let initial_count = map.len();
        map.retain(|_, session| now.duration_since(session.last_active) < timeout);
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

        let (session_id, mut rx) = manager.create_session(user.clone()).await;
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
        let (session_id, _rx) = manager.create_session(user).await;

        // Cleanup with 1 hour timeout shouldn't remove it
        let cleaned = manager.cleanup_stale(Duration::from_secs(3600)).await;
        assert_eq!(cleaned, 0);
        assert_eq!(manager.count().await, 1);

        // Cleanup with 0 timeout removes stale session
        tokio::time::sleep(Duration::from_millis(5)).await;
        let cleaned = manager.cleanup_stale(Duration::from_millis(1)).await;
        assert_eq!(cleaned, 1);
        assert_eq!(manager.count().await, 0);
        assert!(manager.get_user(&session_id).await.is_none());
    }
}
