use super::*;
use crate::models::{ApiToken, PluginItem, User};

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
fn test_render_api_tokens_page_empty() {
    let ctx = dummy_ctx();
    let out = render_api_tokens_page(&ctx, &[], None, None, None);
    let html = out.0;
    assert!(html.contains("My API Tokens (0)"));
    assert!(html.contains("No API tokens created yet."));
    assert!(html.contains(r#"action="/api_token_create""#));
}

#[test]
fn test_render_api_tokens_page_with_token_banner() {
    let ctx = dummy_ctx();
    let tokens = vec![ApiToken {
        id: 10,
        user_id: 1,
        name: "test-token".to_string(),
        hash: "hashval".to_string(),
        date_created: 1700000000,
        date_used: 1700000000,
    }];
    let out = render_api_tokens_page(&ctx, &tokens, Some("secret_token_123"), None, None);
    let html = out.0;
    assert!(html.contains("New API Token Created!"));
    assert!(html.contains("secret_token_123"));
    assert!(html.contains("test-token"));
    assert!(html.contains(r#"action="/api_token_revoke/10""#));
}

#[test]
fn test_render_manage_plugin_page() {
    let ctx = dummy_ctx();
    let plugins = vec![
        PluginItem {
            basename: "MantisCoreFormatting".to_string(),
            enabled: 1,
            protected: 1,
            priority: 1,
        },
        PluginItem {
            basename: "EventAuditLog".to_string(),
            enabled: 1,
            protected: 0,
            priority: 5,
        },
    ];

    let out = render_manage_plugin_page(&ctx, &plugins, "42", Some("Plugin toggled"), None);
    let html = out.0;
    assert!(html.contains("Installed Plugins (2)"));
    assert!(html.contains("EventAuditLog Dispatched Events: 42"));
    assert!(html.contains("MantisCoreFormatting"));
    assert!(html.contains("<em>Protected</em>"));
    assert!(html.contains("/manage_plugin_toggle/EventAuditLog"));
    assert!(html.contains("Disable"));
}
