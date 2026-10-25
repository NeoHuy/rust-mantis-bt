use super::*;

#[test]
fn test_basic_auth_header_format() {
    let auth = build_basic_auth_header("test@example.com", "my-api-token");
    assert_eq!(auth, "Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=");
}

#[test]
fn test_issue_payload_serialization() {
    let payload =
        build_create_issue_payload("PROJ", "Issue Summary", "Detailed description", "Bug");
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

#[tokio::test]
async fn test_create_issue_missing_credentials_fails_fast() {
    let client = JiraClient::new();
    let conf = JiraResolvedConfig {
        enabled: true,
        base_url: "https://test.jira.com".to_string(),
        email: "".to_string(),
        api_token: "".to_string(),
        default_issue_type: "Bug".to_string(),
        project_key: "PROJ".to_string(),
    };
    let err = client
        .create_issue(&conf, "Summary", "Desc", "Bug")
        .await
        .unwrap_err();
    match err {
        JiraError::ConfigError(msg) => assert!(msg.contains("Email and API token")),
        other => panic!("Expected ConfigError, got {:?}", other),
    }
}
