use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFilter {
    pub id: i64,
    pub user_id: i64,
    pub project_id: i64,
    pub is_public: i32,
    pub name: String,
    pub filter_string: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugFilter {
    pub project_id: Option<i64>,
    pub search: Option<String>,
    pub status: Option<i32>,
    pub hide_status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub category_id: Option<i64>,
    pub reporter_id: Option<i64>,
    pub handler_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub monitored_by: Option<i64>,
}
