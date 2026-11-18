use super::*;

#[test]
fn test_basic_auth_header_format() {
    let auth = build_basic_auth_header("test@example.com", "my-api-token");
    assert_eq!(auth, "Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=");
}

#[test]
fn test_issue_payload_serialization() {
    let payload =
        build_create_issue_payload("PROJ", "Issue Summary", "Detailed description", "Bug", None);
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains(r#""project":{"key":"PROJ"}"#));
    assert!(json.contains(r#""summary":"Issue Summary""#));
    assert!(json.contains(r#""description":"Detailed description""#));
    assert!(json.contains(r#""issuetype":{"name":"Bug"}"#));
    assert!(!json.contains(r#""parent""#));
}

#[test]
fn test_issue_payload_serialization_with_epic() {
    let payload = build_create_issue_payload(
        "PROJ",
        "Issue Summary",
        "Detailed description",
        "Story",
        Some("PROJ-10"),
    );
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains(r#""parent":{"key":"PROJ-10"}"#));
    assert!(json.contains(r#""issuetype":{"name":"Story"}"#));
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

#[test]
fn test_normalize_jira_key() {
    assert_eq!(normalize_jira_key("123", "JBC"), "JBC-123");
    assert_eq!(normalize_jira_key("JBC-123", "JBC"), "JBC-123");
    assert_eq!(normalize_jira_key("PROJ-456", "JBC"), "PROJ-456");
    assert_eq!(
        normalize_jira_key("https://myjira.atlassian.net/browse/JBC-789", "JBC"),
        "JBC-789"
    );
    assert_eq!(normalize_jira_key("  jbc-99  ", "JBC"), "JBC-99");
    assert_eq!(normalize_jira_key("", "JBC"), "");
}

#[test]
fn test_map_jira_status_to_mantis() {
    assert_eq!(map_jira_status_to_mantis("To Do"), 30);
    assert_eq!(map_jira_status_to_mantis("Open"), 30);
    assert_eq!(map_jira_status_to_mantis("In Progress"), 50);
    assert_eq!(map_jira_status_to_mantis("In Development"), 50);
    assert_eq!(map_jira_status_to_mantis("In Review"), 20);
    assert_eq!(map_jira_status_to_mantis("QA Testing"), 20);
    assert_eq!(map_jira_status_to_mantis("Done"), 80);
    assert_eq!(map_jira_status_to_mantis("Resolved"), 80);
    assert_eq!(map_jira_status_to_mantis("Closed"), 90);
    assert_eq!(map_jira_status_to_mantis("Something Random"), 30);
}

#[test]
fn test_issue_details_deserialization() {
    let raw_json = r#"{
        "id": "20001",
        "key": "JBC-55",
        "fields": {
            "summary": "Fix billing calculation",
            "issuetype": {
                "name": "Story"
            },
            "status": {
                "name": "In Progress"
            }
        }
    }"#;
    let details = parse_issue_details_response(raw_json).unwrap();
    assert_eq!(details.id, "20001");
    assert_eq!(details.key, "JBC-55");
    assert_eq!(details.summary, "Fix billing calculation");
    assert_eq!(details.issue_type, "Story");
    assert_eq!(details.status, "In Progress");
}

#[test]
fn test_epics_search_deserialization() {
    let raw_json = r#"{
        "issues": [
            {
                "id": "1001",
                "key": "JBC-10",
                "fields": {
                    "summary": "Auth and Permissions"
                }
            },
            {
                "id": "1002",
                "key": "JBC-20",
                "fields": {
                    "summary": "Billing Module"
                }
            }
        ]
    }"#;
    let epics = parse_search_epics_response(raw_json).unwrap();
    assert_eq!(epics.len(), 2);
    assert_eq!(epics[0].key, "JBC-10");
    assert_eq!(epics[0].summary, "Auth and Permissions");
    assert_eq!(epics[1].key, "JBC-20");
    assert_eq!(epics[1].summary, "Billing Module");
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
        .create_issue(&conf, "Summary", "Desc", "Bug", None)
        .await
        .unwrap_err();
    match err {
        JiraError::ConfigError(msg) => assert!(msg.contains("Email and API token")),
        other => panic!("Expected ConfigError, got {:?}", other),
    }
}
