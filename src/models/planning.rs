use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailMessage {
    pub email_id: i64,
    pub email: String,
    pub subject: String,
    pub metadata: String,
    pub body: String,
    pub submitted: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: i64,
    pub project_id: i64,
    pub poster_id: i64,
    pub date_posted: i64,
    pub last_modified: i64,
    pub view_state: i32,
    pub announcement: i32,
    pub headline: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeTrackingEntry {
    pub bug_id: i64,
    pub bug_summary: String,
    pub user_id: i64,
    pub username: String,
    pub minutes: i32,
    pub date_submitted: i64,
    pub note: String,
}
