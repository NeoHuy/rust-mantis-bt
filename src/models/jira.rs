use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraLink {
    pub id: i64,
    pub bug_id: i64,
    pub jira_key: String,
    pub jira_url: String,
    pub issue_type: String,
    pub jira_status: String,
    pub created_by: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraEpic {
    pub key: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraIssueDetails {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub issue_type: String,
    pub status: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiraStatusMapping {
    pub jira_status: String,
    pub mantis_status: i32,
}
