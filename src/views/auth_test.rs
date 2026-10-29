use super::*;
use crate::models::User;

#[test]
fn test_render_login_page_default() {
    let out = render_login_page(None);
    let html = out.0;
    assert!(html.contains("<title>Login - MantisBT</title>"));
    assert!(html.contains(r#"action="/login""#));
    assert!(html.contains("Remember my login on this browser"));
}

#[test]
fn test_render_login_page_with_error() {
    let out = render_login_page(Some("Invalid username or password."));
    let html = out.0;
    assert!(html.contains("<title>Login Failed - MantisBT</title>"));
    assert!(html.contains("Invalid username or password."));
}

#[test]
fn test_render_account_page() {
    let user = User {
        id: 1,
        username: "alice".to_string(),
        realname: "Alice Smith".to_string(),
        email: "alice@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 55, // Developer
        login_count: 3,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "c1".to_string(),
        last_visit: 0,
        date_created: 0,
    };
    let ctx = AuthContext {
        user,
        active_project_id: 0,
        visible_projects: vec![],
    };

    let out = render_account_page(&ctx, Some("Account updated"));
    let html = out.0;
    assert!(html.contains("Edit Account"));
    assert!(html.contains("Alice Smith"));
    assert!(html.contains("alice@example.com"));
    assert!(html.contains("developer"));
    assert!(html.contains("Account updated"));
}
