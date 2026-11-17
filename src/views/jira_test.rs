use super::*;
use crate::models::{JiraResolvedConfig, User};

fn dummy_ctx() -> AuthContext {
    let user = User {
        id: 1,
        username: "admin".to_string(),
        realname: "Admin".to_string(),
        email: "admin@x.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 1,
        access_level: 90,
        login_count: 1,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "c".to_string(),
        last_visit: 0,
        date_created: 0,
    };
    AuthContext {
        user,
        active_project_id: 0,
        visible_projects: vec![],
    }
}

#[test]
fn test_render_manage_jira_page() {
    let ctx = dummy_ctx();
    let config = JiraResolvedConfig {
        enabled: true,
        base_url: "https://test.atlassian.net".to_string(),
        email: "jira-bot@example.com".to_string(),
        api_token: "secret_jira_token".to_string(),
        default_issue_type: "Task".to_string(),
        project_key: "GLOBAL".to_string(),
    };
    let mappings = vec![(10, "Billing Service".to_string(), "BILL".to_string())];

    let out = render_manage_jira_page(
        &ctx,
        &config,
        &[],
        &mappings,
        Some("Configuration saved successfully"),
        None,
    );
    let html = out.0;
    assert!(html.contains("Jira Cloud Integration Configuration"));
    assert!(html.contains("https://test.atlassian.net"));
    assert!(html.contains("jira-bot@example.com"));
    assert!(html.contains("Configuration saved successfully"));
    assert!(html.contains("Billing Service"));
    assert!(html.contains("BILL"));
    assert!(html.contains(r#"action="/manage_jira_config""#));
    assert!(html.contains(r#"action="/manage_jira_test""#));
}

#[test]
fn test_render_manage_jira_page_with_error() {
    let ctx = dummy_ctx();
    let config = JiraResolvedConfig {
        enabled: false,
        base_url: "".to_string(),
        email: "".to_string(),
        api_token: "".to_string(),
        default_issue_type: "Bug".to_string(),
        project_key: "".to_string(),
    };

    let out = render_manage_jira_page(
        &ctx,
        &config,
        &[],
        &[],
        None,
        Some("Jira connection failed"),
    );
    let html = out.0;
    assert!(html.contains("flash-error"));
    assert!(html.contains("Jira connection failed"));
}
