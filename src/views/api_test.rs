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
    let out = render_api_tokens_page(&ctx, &[], &[], None, None, None);
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
        scope: "api".to_string(),
        hash: "hashval".to_string(),
        date_created: 1700000000,
        date_used: 1700000000,
    }];
    let out = render_api_tokens_page(&ctx, &tokens, &[], Some("secret_token_123"), None, None);
    let html = out.0;
    assert!(html.contains("New API Token Created!"));
    assert!(html.contains("secret_token_123"));
    assert!(html.contains(r#"id="new-api-token""#));
    assert!(html.contains(r#"data-copy-target="new-api-token""#));
    assert!(html.contains("Copy Token"));
    assert!(html.contains("test-token"));
    assert!(html.contains(r#"value="rss""#));
    assert!(html.contains(r#"action="/api_token_revoke/10""#));
    assert!(html.contains("btn btn-danger btn-white btn-round btn-sm"));
    assert_eq!(html.matches(r#"class="sub-nav-tabs""#).count(), 1);
    assert!(html.contains(r#"href="/account_page""#));
    assert!(!html.contains("style="));
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
            basename: "CoreSecurity".to_string(),
            enabled: 1,
            protected: 1,
            priority: 0,
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
    assert!(html.contains("Installed Plugins (3)"));
    assert!(html.contains("EventAuditLog Dispatched Events: 42"));
    assert!(html.contains("MantisCoreFormatting"));
    // MantisCoreFormatting is in TOGGLEABLE_CORE_PLUGINS so it renders a toggle button despite protected=1
    assert!(html.contains(r#"action="/manage_plugin_toggle/MantisCoreFormatting""#));
    // Non-toggleable protected plugin renders Protected without a toggle form
    assert!(html.contains("CoreSecurity"));
    assert!(html.contains("<em>Protected</em>"));
    assert!(!html.contains(r#"action="/manage_plugin_toggle/CoreSecurity""#));
    assert!(html.contains("/manage_plugin_toggle/EventAuditLog"));
    assert!(html.contains("Disable"));

    // Disabled MantisCoreFormatting renders Enable button
    let disabled_plugins = vec![PluginItem {
        basename: "MantisCoreFormatting".to_string(),
        enabled: 0,
        protected: 1,
        priority: 1,
    }];
    let out_disabled = render_manage_plugin_page(&ctx, &disabled_plugins, "0", None, None);
    assert!(out_disabled
        .0
        .contains(r#"action="/manage_plugin_toggle/MantisCoreFormatting""#));
    assert!(out_disabled.0.contains(r#"value="1""#));
    assert!(out_disabled.0.contains(">Enable</button>"));
}
