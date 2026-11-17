use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraLink {
    pub id: i64,
    pub bug_id: i64,
    pub jira_key: String,
    pub jira_url: String,
    pub issue_type: String,
    pub created_by: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraResolvedConfig {
    pub enabled: bool,
    pub base_url: String,
    pub email: String,
    pub api_token: String,
    pub default_issue_type: String,
    pub project_key: String,
}
