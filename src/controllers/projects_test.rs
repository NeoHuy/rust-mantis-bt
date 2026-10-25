use super::*;
use crate::models::User;

#[test]
fn test_create_proj_form_deserialize() {
    let form: CreateProjForm = serde_json::from_str(
        r#"{
            "name": "New Project",
            "status": 10,
            "view_state": 10,
            "description": "Desc",
            "inherit_global": "1"
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "New Project");
    assert_eq!(form.status, Some(10));
    assert_eq!(form.view_state, Some(10));
    assert_eq!(form.description.as_deref(), Some("Desc"));
    assert_eq!(form.inherit_global.as_deref(), Some("1"));
}

#[test]
fn test_update_proj_form_deserialize() {
    let form: UpdateProjForm = serde_json::from_str(
        r#"{
            "name": "Updated Project",
            "status": 50,
            "enabled": "on",
            "view_state": 50,
            "description": "NewDesc",
            "inherit_global": "1"
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "Updated Project");
    assert_eq!(form.status, 50);
    assert_eq!(form.enabled.as_deref(), Some("on"));
    assert_eq!(form.view_state, 50);
    assert_eq!(form.description.as_deref(), Some("NewDesc"));
}

#[test]
fn test_require_manager_guard() {
    let dev_user = User {
        id: 1,
        username: "dev".to_string(),
        realname: "Dev".to_string(),
        email: "dev@x.com".to_string(),
        password: "hash".to_string(),
        access_level: 55, // Developer < 70
        enabled: 1,
        protected: 0,
        cookie_string: "c".to_string(),
        failed_login_count: 0,
        lost_password_request_count: 0,
        login_count: 1,
        last_visit: 0,
        date_created: 0,
    };
    let dev_ctx = AuthContext {
        user: dev_user,
        active_project_id: 0,
        visible_projects: vec![],
    };
    assert!(require_manager(&dev_ctx).is_some());

    let mgr_user = User {
        id: 2,
        username: "manager".to_string(),
        realname: "Manager".to_string(),
        email: "mgr@x.com".to_string(),
        password: "hash".to_string(),
        access_level: 70, // Manager >= 70
        enabled: 1,
        protected: 0,
        cookie_string: "c2".to_string(),
        failed_login_count: 0,
        lost_password_request_count: 0,
        login_count: 1,
        last_visit: 0,
        date_created: 0,
    };
    let mgr_ctx = AuthContext {
        user: mgr_user,
        active_project_id: 0,
        visible_projects: vec![],
    };
    assert!(require_manager(&mgr_ctx).is_none());
}
