use crate::models::{JiraEpic, JiraIssueDetails, JiraResolvedConfig};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraMyselfResponse {
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(rename = "emailAddress", default)]
    pub email_address: Option<String>,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraProjectField {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueTypeField {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraParentField {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraFields {
    pub project: JiraProjectField,
    pub summary: String,
    pub description: String,
    pub issuetype: JiraIssueTypeField,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<JiraParentField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueCreatePayload {
    pub fields: JiraFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraCreateIssueRawResponse {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraCreateIssueResponse {
    pub id: String,
    pub key: String,
    pub self_url: String,
    pub browse_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraStatusField {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraIssueDetailsFields {
    pub summary: String,
    pub issuetype: JiraIssueTypeField,
    pub status: JiraStatusField,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraIssueDetailsRawResponse {
    pub id: String,
    pub key: String,
    pub fields: JiraIssueDetailsFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraEpicFields {
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraEpicIssueRaw {
    pub id: String,
    pub key: String,
    pub fields: JiraEpicFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraSearchEpicsRawResponse {
    pub issues: Vec<JiraEpicIssueRaw>,
}

#[derive(Debug, Clone)]
pub enum JiraError {
    ConfigError(String),
    NetworkError(String),
    AuthError(String),
    ApiError { status: u16, message: String },
    ParseError(String),
}

impl std::fmt::Display for JiraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigError(msg) => write!(f, "Jira config error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Jira network error: {}", msg),
            Self::AuthError(msg) => write!(f, "Jira authentication failed: {}", msg),
            Self::ApiError { status, message } => {
                write!(f, "Jira API error (HTTP {}): {}", status, message)
            }
            Self::ParseError(msg) => write!(f, "Jira response parse error: {}", msg),
        }
    }
}

impl std::error::Error for JiraError {}

#[derive(Clone)]
pub struct JiraClient {
    client: reqwest::Client,
}

impl Default for JiraClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JiraClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn test_connection(
        &self,
        base_url: &str,
        email: &str,
        api_token: &str,
    ) -> Result<JiraMyselfResponse, JiraError> {
        let trimmed_url = base_url.trim().trim_end_matches('/');
        if trimmed_url.is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Base URL cannot be empty".to_string(),
            ));
        }
        if email.trim().is_empty() || api_token.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Email and API token must be configured".to_string(),
            ));
        }

        let url = format!("{}/rest/api/3/myself", trimmed_url);
        let resp = self
            .client
            .get(&url)
            .basic_auth(email.trim(), Some(api_token.trim()))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(JiraError::AuthError(
                "Invalid credentials or insufficient permissions".to_string(),
            ));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(JiraError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        resp.json::<JiraMyselfResponse>()
            .await
            .map_err(|e| JiraError::ParseError(e.to_string()))
    }

    pub async fn create_issue(
        &self,
        config: &JiraResolvedConfig,
        summary: &str,
        description: &str,
        issue_type: &str,
        epic_key: Option<&str>,
    ) -> Result<JiraCreateIssueResponse, JiraError> {
        let trimmed_url = config.base_url.trim().trim_end_matches('/');
        if trimmed_url.is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Base URL is not configured".to_string(),
            ));
        }
        if config.project_key.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Project Key is not configured for this project".to_string(),
            ));
        }
        if config.email.trim().is_empty() || config.api_token.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Email and API token must be configured".to_string(),
            ));
        }

        let payload = build_create_issue_payload(
            &config.project_key,
            summary,
            description,
            if issue_type.trim().is_empty() {
                &config.default_issue_type
            } else {
                issue_type
            },
            epic_key,
        );

        let url = format!("{}/rest/api/2/issue", trimmed_url);
        let resp = self
            .client
            .post(&url)
            .basic_auth(config.email.trim(), Some(config.api_token.trim()))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(JiraError::AuthError(
                "Invalid credentials or insufficient permissions on Jira project".to_string(),
            ));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(JiraError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        parse_create_issue_response(&body_text, trimmed_url)
    }

    pub async fn get_issue(
        &self,
        config: &JiraResolvedConfig,
        key: &str,
    ) -> Result<JiraIssueDetails, JiraError> {
        let trimmed_url = config.base_url.trim().trim_end_matches('/');
        if trimmed_url.is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Base URL is not configured".to_string(),
            ));
        }
        if config.email.trim().is_empty() || config.api_token.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Email and API token must be configured".to_string(),
            ));
        }
        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            return Err(JiraError::ConfigError(
                "Jira key cannot be empty".to_string(),
            ));
        }
        if !is_valid_jira_key(trimmed_key) {
            return Err(JiraError::ConfigError(format!(
                "Invalid Jira issue key format: '{}'",
                trimmed_key
            )));
        }

        let url = format!(
            "{}/rest/api/2/issue/{}?fields=summary,issuetype,status",
            trimmed_url, trimmed_key
        );
        let resp = self
            .client
            .get(&url)
            .basic_auth(config.email.trim(), Some(config.api_token.trim()))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(JiraError::AuthError(
                "Invalid credentials or insufficient permissions on Jira project".to_string(),
            ));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(JiraError::ApiError {
                status: 404,
                message: format!("Jira issue '{}' not found", trimmed_key),
            });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(JiraError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        parse_issue_details_response(&body_text)
    }

    pub async fn list_epics(
        &self,
        config: &JiraResolvedConfig,
    ) -> Result<Vec<JiraEpic>, JiraError> {
        let trimmed_url = config.base_url.trim().trim_end_matches('/');
        if trimmed_url.is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Base URL is not configured".to_string(),
            ));
        }
        if config.project_key.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Jira Project Key is not configured for this project".to_string(),
            ));
        }
        if config.email.trim().is_empty() || config.api_token.trim().is_empty() {
            return Err(JiraError::ConfigError(
                "Email and API token must be configured".to_string(),
            ));
        }

        let jql = format!(
            "project = \"{}\" AND issuetype in ('Epic', 'epic') ORDER BY key DESC",
            config.project_key.trim()
        );
        let url = format!("{}/rest/api/3/search/jql", trimmed_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("jql", jql.as_str()),
                ("fields", "summary"),
                ("maxResults", "50"),
            ])
            .basic_auth(config.email.trim(), Some(config.api_token.trim()))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(JiraError::AuthError(
                "Invalid credentials or insufficient permissions to search Jira epics".to_string(),
            ));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(JiraError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| JiraError::NetworkError(e.to_string()))?;

        parse_search_epics_response(&body_text)
    }
}

pub fn base64_encode(input: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < input.len() {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

pub fn build_basic_auth_header(email: &str, token: &str) -> String {
    format!(
        "Basic {}",
        base64_encode(format!("{}:{}", email, token).as_bytes())
    )
}

pub fn build_create_issue_payload(
    project_key: &str,
    summary: &str,
    description: &str,
    issue_type: &str,
    epic_key: Option<&str>,
) -> JiraIssueCreatePayload {
    let parent = epic_key
        .map(|k| k.trim())
        .filter(|k| is_valid_jira_key(k))
        .map(|k| JiraParentField { key: k.to_string() });

    JiraIssueCreatePayload {
        fields: JiraFields {
            project: JiraProjectField {
                key: project_key.trim().to_string(),
            },
            summary: summary.trim().to_string(),
            description: description.to_string(),
            issuetype: JiraIssueTypeField {
                name: issue_type.trim().to_string(),
            },
            parent,
        },
    }
}

pub fn parse_create_issue_response(
    raw_json: &str,
    base_url: &str,
) -> Result<JiraCreateIssueResponse, JiraError> {
    let raw: JiraCreateIssueRawResponse =
        serde_json::from_str(raw_json).map_err(|e| JiraError::ParseError(e.to_string()))?;
    let trimmed_url = base_url.trim().trim_end_matches('/');
    let browse_url = format!("{}/browse/{}", trimmed_url, raw.key);
    Ok(JiraCreateIssueResponse {
        id: raw.id,
        key: raw.key,
        self_url: raw.self_url,
        browse_url,
    })
}

pub fn parse_issue_details_response(raw_json: &str) -> Result<JiraIssueDetails, JiraError> {
    let raw: JiraIssueDetailsRawResponse =
        serde_json::from_str(raw_json).map_err(|e| JiraError::ParseError(e.to_string()))?;
    Ok(JiraIssueDetails {
        id: raw.id,
        key: raw.key,
        summary: raw.fields.summary,
        issue_type: raw.fields.issuetype.name,
        status: raw.fields.status.name,
    })
}

pub fn parse_search_epics_response(raw_json: &str) -> Result<Vec<JiraEpic>, JiraError> {
    let raw: JiraSearchEpicsRawResponse =
        serde_json::from_str(raw_json).map_err(|e| JiraError::ParseError(e.to_string()))?;
    Ok(raw
        .issues
        .into_iter()
        .map(|i| JiraEpic {
            key: i.key,
            summary: i.fields.summary,
        })
        .collect())
}

pub fn is_valid_jira_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() != 2 {
        return false;
    }
    let project = parts[0];
    let num = parts[1];
    if project.is_empty() || num.is_empty() {
        return false;
    }
    let mut chars = project.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return false;
    }
    num.chars().all(|c| c.is_ascii_digit())
}

pub fn normalize_jira_key(input: &str, default_project_key: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let candidate = if let Some(idx) = trimmed.find("/browse/") {
        let after = &trimmed[idx + "/browse/".len()..];
        let key_part = after.split(['?', '#', '/']).next().unwrap_or("").trim();
        key_part.to_uppercase()
    } else if trimmed.chars().all(|c| c.is_ascii_digit()) {
        let prefix = default_project_key.trim().to_uppercase();
        if !prefix.is_empty() {
            format!("{}-{}", prefix, trimmed)
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_uppercase()
    };

    if is_valid_jira_key(&candidate) {
        candidate
    } else {
        String::new()
    }
}

pub fn map_jira_status_to_mantis(jira_status: &str) -> i32 {
    let s = jira_status.trim().to_lowercase();
    if s.contains("closed") {
        90 // closed
    } else if s.contains("done") || s.contains("resolved") || s.contains("completed") {
        80 // resolved
    } else if s.contains("review")
        || s.contains("testing")
        || s.contains("qa")
        || s.contains("feedback")
    {
        20 // feedback
    } else if s.contains("in progress") || s.contains("in development") || s.contains("dev") {
        50 // assigned
    } else {
        30 // acknowledged (default for To Do / Open / Backlog)
    }
}

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
