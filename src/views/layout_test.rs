use super::*;
use crate::models::User;

#[test]
fn test_render_unauthenticated_layout() {
    let out = render_unauthenticated_layout("Login", Some("Bad credentials"), "<p>Form</p>");
    let html = out.0;
    assert!(html.contains("<title>Login - MantisBT</title>"));
    assert!(html.contains("Bad credentials"));
    assert!(html.contains("<p>Form</p>"));
    assert!(html.contains("Mantis"));
}

#[test]
fn test_render_layout_authenticated() {
    let user = User {
        id: 1,
        username: "jdoe".to_string(),
        realname: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 90, // Administrator
        login_count: 5,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "cookiestr".to_string(),
        last_visit: 0,
        date_created: 0,
    };
    let ctx = AuthContext {
        user,
        active_project_id: 0,
        visible_projects: vec![],
    };

    let out = render_layout(
        "Manage Users",
        &ctx,
        Some("Action successful"),
        "<p>User List</p>",
    );
    let html = out.0;
    assert!(html.contains("<title>Manage Users - MantisBT</title>"));
    assert!(html.contains("Action successful"));
    assert!(html.contains("<p>User List</p>"));
    assert!(html.contains("jdoe"));
    assert!(html.contains("administrator"));
    assert!(html.contains("Manage Users"));
}
