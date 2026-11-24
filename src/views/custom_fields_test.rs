use super::*;
use crate::models::{CustomField, Project, User};

fn dummy_ctx() -> AuthContext {
    let user = User {
        id: 1,
        username: "admin".to_string(),
        realname: "Admin".to_string(),
        email: "admin@x.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
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
fn test_render_manage_custom_field_page_empty() {
    let ctx = dummy_ctx();
    let out = render_manage_custom_field_page(&ctx, &[], None, None);
    let html = out.0;
    assert!(html.contains("Custom Fields (0)"));
    assert!(html.contains("No custom fields defined."));
    assert!(html.contains("Create Custom Field"));
}

#[test]
fn test_render_manage_custom_field_page_with_items() {
    let ctx = dummy_ctx();
    let cf = CustomField {
        id: 1,
        name: "OS".to_string(),
        field_type: 0, // String
        possible_values: "Win|Mac|Linux".to_string(),
        default_value: "Linux".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 255,
        display_report: 1,
        display_update: 1,
        display_resolved: 0,
        display_closed: 0,
        require_report: 0,
        require_update: 0,
        require_resolved: 0,
        require_closed: 0,
        filter_by: 0,
    };
    let fields = vec![(cf, 2)];

    let out = render_manage_custom_field_page(&ctx, &fields, Some("Field added"), None);
    let html = out.0;
    assert!(html.contains("Custom Fields (1)"));
    assert!(html.contains("OS"));
    assert!(html.contains("Win|Mac|Linux"));
    assert!(html.contains("Field added"));
}

#[test]
fn test_render_manage_custom_field_edit_page() {
    let ctx = dummy_ctx();
    let cf = CustomField {
        id: 4,
        name: "Browser".to_string(),
        field_type: 0,
        possible_values: "Chrome|Firefox".to_string(),
        default_value: "Chrome".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 100,
        display_report: 1,
        display_update: 1,
        display_resolved: 0,
        display_closed: 0,
        require_report: 1,
        require_update: 0,
        require_resolved: 0,
        require_closed: 0,
        filter_by: 0,
    };
    let linked = vec![(
        Project {
            id: 10,
            name: "Web Portal".to_string(),
            status: 10,
            enabled: 1,
            view_state: 10,
            access_min: 10,
            description: "".to_string(),
            category_id: 0,
            inherit_global: 1,
        },
        1,
    )];

    let out = render_manage_custom_field_edit_page(&ctx, &cf, &linked, None, None);
    let html = out.0;
    assert!(html.contains("Edit Custom Field: Browser"));
    assert!(html.contains("Chrome|Firefox"));
    assert!(html.contains("Web Portal"));
    assert!(html.contains(r#"action="/manage_custom_field_update/4""#));
}
