use super::*;
use crate::models::User;

#[test]
fn test_create_user_form_deserialize() {
    let form: CreateUserForm = serde_json::from_str(
        r#"{
            "username": "alice",
            "realname": "Alice Dev",
            "email": "alice@x.com",
            "password": "pass123",
            "access_level": 55,
            "enabled": "1",
            "protected": "0"
        }"#,
    )
    .unwrap();
    assert_eq!(form.username, "alice");
    assert_eq!(form.realname.as_deref(), Some("Alice Dev"));
    assert_eq!(form.email.as_deref(), Some("alice@x.com"));
    assert_eq!(form.password, "pass123");
    assert_eq!(form.access_level, 55);
    assert_eq!(form.enabled.as_deref(), Some("1"));
}

#[test]
fn test_update_user_form_deserialize() {
    let form: UpdateUserForm = serde_json::from_str(
        r#"{
            "realname": "Bob Updated",
            "email": "bob@x.com",
            "access_level": 70,
            "enabled": "on"
        }"#,
    )
    .unwrap();
    assert_eq!(form.realname.as_deref(), Some("Bob Updated"));
    assert_eq!(form.email.as_deref(), Some("bob@x.com"));
    assert_eq!(form.password, None);
    assert_eq!(form.access_level, 70);
    assert_eq!(form.enabled.as_deref(), Some("on"));
}

#[test]
fn test_require_admin_guard() {
    let user = User {
        id: 1,
        username: "dev".to_string(),
        realname: "Dev".to_string(),
        email: "dev@x.com".to_string(),
        password: "hash".to_string(),
        access_level: 70, // Manager < 90
        enabled: 1,
        protected: 0,
        cookie_string: "c".to_string(),
        failed_login_count: 0,
        lost_password_request_count: 0,
        login_count: 1,
        last_visit: 0,
        date_created: 0,
    };
    let ctx = AuthContext {
        user,
        active_project_id: 0,
        visible_projects: vec![],
    };
    assert!(require_admin(&ctx).is_some());

    let admin_user = User {
        id: 2,
        username: "admin".to_string(),
        realname: "Admin".to_string(),
        email: "a@x.com".to_string(),
        password: "hash".to_string(),
        access_level: 90, // Administrator >= 90
        enabled: 1,
        protected: 0,
        cookie_string: "c2".to_string(),
        failed_login_count: 0,
        lost_password_request_count: 0,
        login_count: 1,
        last_visit: 0,
        date_created: 0,
    };
    let admin_ctx = AuthContext {
        user: admin_user,
        active_project_id: 0,
        visible_projects: vec![],
    };
    assert!(require_admin(&admin_ctx).is_none());
}
