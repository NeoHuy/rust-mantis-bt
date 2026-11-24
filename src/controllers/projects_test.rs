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

#[test]
fn test_parse_date_order() {
    assert_eq!(parse_date_order(None), None);
    assert_eq!(parse_date_order(Some("")), None);
    assert_eq!(parse_date_order(Some("   ")), None);
    assert_eq!(parse_date_order(Some("invalid")), None);
    // Integer unix timestamp string
    assert_eq!(parse_date_order(Some("1700000000")), Some(1700000000));
    // YYYY-MM-DD format
    let epoch_2026_11_24 = views::parse_date_to_epoch("2026-11-24");
    assert!(epoch_2026_11_24 > 0);
    assert_eq!(parse_date_order(Some("2026-11-24")), Some(epoch_2026_11_24));
}

#[test]
fn test_ver_forms_deserialize() {
    let form: AddVerForm = serde_json::from_str(
        r#"{"version":"1.0.0","description":"Desc","released":"1","date_order":"2026-11-24"}"#,
    )
    .unwrap();
    assert_eq!(form.version, "1.0.0");
    assert_eq!(form.date_order.as_deref(), Some("2026-11-24"));

    let form_empty_date: AddVerForm =
        serde_json::from_str(r#"{"version":"1.0.0","date_order":""}"#).unwrap();
    assert_eq!(form_empty_date.date_order.as_deref(), Some(""));
    assert_eq!(
        parse_date_order(form_empty_date.date_order.as_deref()),
        None
    );

    let form_no_date: AddVerForm = serde_json::from_str(r#"{"version":"1.0.0"}"#).unwrap();
    assert_eq!(form_no_date.date_order, None);

    let uform: UpdateVerForm =
        serde_json::from_str(r#"{"project_id":1,"version":"2.0.0","date_order":"1750000000"}"#)
            .unwrap();
    assert_eq!(uform.project_id, 1);
    assert_eq!(uform.version, "2.0.0");
    assert_eq!(
        parse_date_order(uform.date_order.as_deref()),
        Some(1750000000)
    );
}
