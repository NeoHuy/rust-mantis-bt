use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: String,
    pub date_created: i64,
    pub date_updated: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugTag {
    pub bug_id: i64,
    pub tag_id: i64,
    pub user_id: i64,
    pub date_attached: i64,
    pub tag_name: String,
}
