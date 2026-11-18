use super::*;
use crate::models::{JiraEpic, JiraLink, JiraResolvedConfig, User};

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
    let cached_epics = vec![("BILL".to_string(), 3, 1700000000)];

    let out = render_manage_jira_page(
        &ctx,
        &config,
        &[],
        &mappings,
        &cached_epics,
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
    assert!(html.contains(r#"action="/manage_jira_refresh_epics""#));
    assert!(html.contains("Cached Jira Epics"));
    assert!(html.contains("3 epic(s)"));
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
        &[],
        None,
        Some("Jira connection failed"),
    );
    let html = out.0;
    assert!(html.contains("flash-error"));
    assert!(html.contains("Jira connection failed"));
}

#[test]
fn test_render_jira_section_disabled() {
    let ctx = dummy_ctx();
    let html = render_jira_section(&ctx, 42, None, &[], false, 55);
    assert!(html.is_empty());
}

#[test]
fn test_render_jira_section_unlinked_developer() {
    let ctx = dummy_ctx();
    let epics = vec![JiraEpic {
        key: "JBC-10".to_string(),
        summary: "Auth Epic".to_string(),
    }];
    let html = render_jira_section(&ctx, 42, None, &epics, true, 55);
    assert!(html.contains("Jira Integration"));
    assert!(html.contains(r#"action="/view_bug_page/42/jira_sync""#));
    assert!(html.contains(r#"name="jira_key""#));
    assert!(html.contains(r#"name="epic_key""#));
    assert!(html.contains("JBC-10 - Auth Epic"));
    assert!(html.contains(r#"name="issue_type""#));
    assert!(html.contains("Sync"));
}

#[test]
fn test_render_jira_section_unlinked_viewer() {
    let ctx = dummy_ctx();
    let html = render_jira_section(&ctx, 42, None, &[], true, 10);
    assert!(html.contains("Jira Integration"));
    assert!(html.contains("No Jira ticket linked"));
    assert!(!html.contains(r#"name="jira_key""#));
}

#[test]
fn test_render_jira_section_linked() {
    let ctx = dummy_ctx();
    let link = JiraLink {
        id: 1,
        bug_id: 42,
        jira_key: "JBC-101".to_string(),
        jira_url: "https://jira.example.com/browse/JBC-101".to_string(),
        issue_type: "Story".to_string(),
        jira_status: "In Progress".to_string(),
        created_by: 1,
        created_at: 12345678,
    };
    let html = render_jira_section(&ctx, 42, Some(&link), &[], true, 55);
    assert!(html.contains("Jira Integration"));
    assert!(html.contains("JBC-101"));
    assert!(html.contains("https://jira.example.com/browse/JBC-101"));
    assert!(html.contains("Story"));
    assert!(html.contains("In Progress"));
    assert!(html.contains(r#"action="/view_bug_page/42/jira_sync""#));
    assert!(html.contains(r#"action="/view_bug_page/42/jira_unlink""#));
    assert!(html.contains("Unlink"));
}
