use super::*;
use crate::models::User;

fn dummy_admin_ctx() -> AuthContext {
    let user = User {
        id: 1,
        username: "admin".to_string(),
        realname: "Administrator".to_string(),
        email: "admin@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 1,
        access_level: 90,
        login_count: 10,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "c1".to_string(),
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
fn test_render_manage_users_page() {
    let ctx = dummy_admin_ctx();
    let sample_user = User {
        id: 2,
        username: "bob".to_string(),
        realname: "Bob Builder".to_string(),
        email: "bob@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 25, // Reporter
        login_count: 1,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "c2".to_string(),
        last_visit: 0,
        date_created: 0,
    };
    let users = vec![sample_user];

    let out = render_manage_users_page(&ctx, &users, "", false);
    let html = out.0;
    assert!(html.contains("Bob Builder"));
    assert!(html.contains("bob@example.com"));
    assert!(html.contains("reporter"));
    assert!(html.contains("/manage_user_edit_page/2"));
}

#[test]
fn test_render_user_create_page() {
    let ctx = dummy_admin_ctx();
    let out = render_user_create_page(&ctx);
    let html = out.0;
    assert!(html.contains("Create User"));
    assert!(html.contains(r#"action="/manage_user_create""#));
}

#[test]
fn test_render_user_edit_page() {
    let ctx = dummy_admin_ctx();
    let target = User {
        id: 5,
        username: "developer".to_string(),
        realname: "Senior Dev".to_string(),
        email: "dev@corp.local".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 55,
        login_count: 2,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "c5".to_string(),
        last_visit: 0,
        date_created: 0,
    };

    let out = render_user_edit_page(&ctx, &target);
    let html = out.0;
    assert!(html.contains("Edit User: developer"));
    assert!(html.contains("Senior Dev"));
    assert!(html.contains(r#"action="/manage_user_update/5""#));
    assert!(html.contains("/manage_user_delete/5"));
}

#[test]
fn test_render_user_create_error() {
    let ctx = dummy_admin_ctx();
    let (status, out) = render_user_create_error(&ctx, "Username already exists");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let html = out.0;
    assert!(html.contains("Username already exists"));
    assert!(html.contains("/manage_user_create_page"));
}
