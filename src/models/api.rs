use serde::{Deserialize, Serialize};

fn default_api_token_scope() -> String {
    "api".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    #[serde(default = "default_api_token_scope")]
    pub scope: String,
    pub hash: String,
    pub date_created: i64,
    pub date_used: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginItem {
    pub basename: String,
    pub enabled: i32,
    pub protected: i32,
    pub priority: i32,
}
