use crate::models::JiraResolvedConfig;
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
pub struct JiraFields {
    pub project: JiraProjectField,
    pub summary: String,
    pub description: String,
    pub issuetype: JiraIssueTypeField,
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
            Self::ApiError { status, message } => write!(f, "Jira API error (HTTP {}): {}", status, message),
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
            return Err(JiraError::ConfigError("Jira Base URL cannot be empty".to_string()));
        }
        if email.trim().is_empty() || api_token.trim().is_empty() {
            return Err(JiraError::ConfigError("Email and API token must be configured".to_string()));
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
            return Err(JiraError::AuthError("Invalid credentials or insufficient permissions".to_string()));
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
    ) -> Result<JiraCreateIssueResponse, JiraError> {
        let trimmed_url = config.base_url.trim().trim_end_matches('/');
        if trimmed_url.is_empty() {
            return Err(JiraError::ConfigError("Jira Base URL is not configured".to_string()));
        }
        if config.project_key.trim().is_empty() {
            return Err(JiraError::ConfigError("Jira Project Key is not configured for this project".to_string()));
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
            return Err(JiraError::AuthError("Invalid credentials or insufficient permissions on Jira project".to_string()));
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
    format!("Basic {}", base64_encode(format!("{}:{}", email, token).as_bytes()))
}

pub fn build_create_issue_payload(
    project_key: &str,
    summary: &str,
    description: &str,
    issue_type: &str,
) -> JiraIssueCreatePayload {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_header_format() {
        let auth = build_basic_auth_header("test@example.com", "my-api-token");
        assert_eq!(auth, "Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=");
    }

    #[test]
    fn test_issue_payload_serialization() {
        let payload = build_create_issue_payload("PROJ", "Issue Summary", "Detailed description", "Bug");
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""project":{"key":"PROJ"}"#));
        assert!(json.contains(r#""summary":"Issue Summary""#));
        assert!(json.contains(r#""description":"Detailed description""#));
        assert!(json.contains(r#""issuetype":{"name":"Bug"}"#));
    }

    #[test]
    fn test_response_deserialization() {
        let raw_json = r#"{
            "id": "10024",
            "key": "DEV-42",
            "self": "https://jira.example.com/rest/api/2/issue/10024"
        }"#;
        let resp = parse_create_issue_response(raw_json, "https://jira.example.com").unwrap();
        assert_eq!(resp.id, "10024");
        assert_eq!(resp.key, "DEV-42");
        assert_eq!(resp.browse_url, "https://jira.example.com/browse/DEV-42");
    }
}
