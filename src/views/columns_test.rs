use super::*;
use crate::models::User;

fn dummy_ctx(access_level: i32) -> AuthContext {
    AuthContext {
        user: User {
            id: 1,
            username: "tester".to_string(),
            realname: "Tester".to_string(),
            email: "t@example.com".to_string(),
            password: String::new(),
            enabled: 1,
            protected: 0,
            access_level,
            login_count: 0,
            lost_password_request_count: 0,
            failed_login_count: 0,
            cookie_string: "c".to_string(),
            last_visit: 0,
            date_created: 0,
        },
        active_project_id: 0,
        visible_projects: Vec::new(),
    }
}

#[test]
fn test_render_manage_columns_page_contexts_and_no_nested_forms() {
    let ctx = dummy_ctx(25);
    let cf = CustomField {
        id: 7,
        name: "Environment".to_string(),
        field_type: 0,
        possible_values: String::new(),
        default_value: String::new(),
        valid_regexp: String::new(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 0,
        require_report: 0,
        require_update: 0,
        display_report: 1,
        display_update: 1,
        require_resolved: 0,
        display_resolved: 0,
        display_closed: 0,
        require_closed: 0,
        filter_by: 1,
    };

    let Html(html) = render_manage_columns_page(
        &ctx,
        0,
        &[cf],
        &["id".to_string(), "summary".to_string()],
        &["id".to_string(), "status".to_string()],
        &["id".to_string(), "custom_field_7".to_string()],
        &["id".to_string(), "priority".to_string()],
        Some("Updated"),
        None,
    );

    assert!(html.contains("Manage Columns"));
    assert!(html.contains(r#"name="view_issues_columns""#));
    assert!(html.contains(r#"name="print_issues_columns""#));
    assert!(html.contains(r#"name="csv_columns""#));
    assert!(html.contains(r#"name="excel_columns""#));
    assert!(html.contains("custom_field_7"));
    assert!(html.contains("Environment"));
    assert!(html.contains(r#"action="/account_manage_columns_update""#));
    assert!(html.contains(r#"action="/account_manage_columns_reset""#));
    assert!(!html.contains("<form><form"));
}
