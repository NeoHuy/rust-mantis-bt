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

#[test]
fn test_render_custom_field_input_all_11_types_and_readonly() {
    fn make_test_cf(id: i64, field_type: i32, possible_values: &str) -> CustomField {
        CustomField {
            id,
            name: format!("Field_{}", id),
            field_type,
            possible_values: possible_values.to_string(),
            default_value: "".to_string(),
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
        }
    }

    let versions = vec![];
    let categories = vec![];

    // Readonly test: read_only = true
    let cf_ro = make_test_cf(1, 0, "");
    let html_ro =
        render_custom_field_input(&cf_ro, "ReadOnlyVal", false, true, &versions, &categories);
    assert!(html_ro.contains(r#"<span class="readonly-custom-field">ReadOnlyVal</span>"#));

    // Type 0: String
    let cf0 = make_test_cf(1, 0, "");
    let html0 = render_custom_field_input(&cf0, "SampleText", false, false, &versions, &categories);
    assert!(html0.contains(r#"type="text""#));
    assert!(html0.contains(r#"name="custom_field_1""#));
    assert!(html0.contains(r#"value="SampleText""#));

    // Type 1: Numeric
    let cf1 = make_test_cf(2, 1, "");
    let html1 = render_custom_field_input(&cf1, "42", false, false, &versions, &categories);
    assert!(html1.contains(r#"type="number""#));
    assert!(html1.contains(r#"step="1""#));
    assert!(html1.contains(r#"name="custom_field_2""#));

    // Type 2: Float
    let cf2 = make_test_cf(3, 2, "");
    let html2 = render_custom_field_input(&cf2, "3.14", false, false, &versions, &categories);
    assert!(html2.contains(r#"type="number""#));
    assert!(html2.contains(r#"step="any""#));
    assert!(html2.contains(r#"name="custom_field_3""#));

    // Type 3: Enum
    let cf3 = make_test_cf(4, 3, "High|Medium|Low");
    let html3 = render_custom_field_input(&cf3, "Medium", false, false, &versions, &categories);
    assert!(html3.contains(r#"<select"#));
    assert!(html3.contains(r#"<option value="Medium" selected>Medium</option>"#));

    // Type 4: Email
    let cf4 = make_test_cf(5, 4, "");
    let html4 = render_custom_field_input(
        &cf4,
        "dev@example.com",
        false,
        false,
        &versions,
        &categories,
    );
    assert!(html4.contains(r#"type="email""#));
    assert!(html4.contains(r#"value="dev@example.com""#));

    // Type 5: Checkbox
    let cf5 = make_test_cf(6, 5, "Alpha|Beta|Gamma");
    let html5 =
        render_custom_field_input(&cf5, "Alpha|Gamma", false, false, &versions, &categories);
    assert!(html5.contains(r#"type="checkbox""#));
    assert!(html5.contains(r#"name="custom_field_6""#));
    assert!(html5.contains(r#"name="custom_field_6_present""#));
    assert!(html5.contains(r#"value="Alpha" checked"#));
    assert!(html5.contains(r#"value="Gamma" checked"#));
    assert!(html5.contains(r#"value="Beta""#));
    assert!(!html5.contains(r#"value="Beta" checked"#));

    // Type 6: List
    let cf6 = make_test_cf(7, 6, "First|Second");
    let html6 = render_custom_field_input(&cf6, "First", false, false, &versions, &categories);
    assert!(html6.contains(r#"<select"#));
    assert!(html6.contains(r#"<option value="First" selected>First</option>"#));

    // Type 7: Multilist
    let cf7 = make_test_cf(8, 7, "A|B|C");
    let html7 = render_custom_field_input(&cf7, "A|C", false, false, &versions, &categories);
    assert!(html7.contains("<select"));
    assert!(html7.contains("multiple"));
    assert!(html7.contains(r#"name="custom_field_8""#));
    assert!(html7.contains(r#"name="custom_field_8_present""#));
    assert!(html7.contains(r#"<option value="A" selected>A</option>"#));
    assert!(html7.contains(r#"<option value="C" selected>C</option>"#));

    // Type 8: Date
    let cf8 = make_test_cf(9, 8, "");
    let html8 = render_custom_field_input(&cf8, "2024-05-15", false, false, &versions, &categories);
    assert!(html8.contains(r#"type="date""#));
    assert!(html8.contains(r#"value="2024-05-15""#));

    // Type 9: Radio
    let cf9 = make_test_cf(10, 9, "Opt1|Opt2");
    let html9 = render_custom_field_input(&cf9, "Opt2", false, false, &versions, &categories);
    assert!(html9.contains(r#"type="radio""#));
    assert!(html9.contains(r#"value="Opt2" checked"#));
    assert!(html9.contains(r#"value="Opt1""#));

    // Type 10: Textarea
    let cf10 = make_test_cf(11, 10, "");
    let html10 = render_custom_field_input(
        &cf10,
        "Line 1\nLine 2",
        false,
        false,
        &versions,
        &categories,
    );
    assert!(html10.contains(r#"<textarea"#));
    assert!(html10.contains("Line 1\nLine 2"));
}

#[test]
fn test_render_custom_fields_form_rows_access_and_required() {
    let cf1 = CustomField {
        id: 1,
        name: "Secret Field".to_string(),
        field_type: 0,
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 55,  // Developer only
        access_level_rw: 70, // Manager
        length_min: 0,
        length_max: 255,
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

    let mut cf2 = cf1.clone();
    cf2.id = 2;
    cf2.name = "Public Field".to_string();
    cf2.access_level_r = 10;
    cf2.access_level_rw = 25;
    cf2.require_report = 0;
    cf2.require_update = 1;

    let fields = vec![cf1, cf2];
    let values = std::collections::HashMap::new();
    let versions = vec![];
    let categories = vec![];

    // Reporter (25) reporting:
    // cf1 hidden because 25 < access_level_r (55)
    // cf2 shown, not required on report (no "*")
    let html_rep = render_custom_fields_form_rows(
        &fields,
        &values,
        25,
        CustomFieldFormMode::Report,
        &versions,
        &categories,
    );
    assert!(!html_rep.contains("Secret Field"));
    assert!(html_rep.contains("Public Field"));
    assert!(!html_rep.contains("Public Field *"));

    // Manager (70) reporting:
    // cf1 visible and required on report (has "*")
    let html_mgr = render_custom_fields_form_rows(
        &fields,
        &values,
        70,
        CustomFieldFormMode::Report,
        &versions,
        &categories,
    );
    assert!(html_mgr.contains("Secret Field *"));

    // Developer (55) updating:
    // cf1 visible as read-only (eff 55 < rw 70)
    // cf2 is required on update (has "*")
    let html_upd = render_custom_fields_form_rows(
        &fields,
        &values,
        55,
        CustomFieldFormMode::Update,
        &versions,
        &categories,
    );
    assert!(html_upd.contains("Secret Field"));
    assert!(html_upd.contains("readonly-custom-field"));
    assert!(html_upd.contains("Public Field *"));
}

#[test]
fn test_render_custom_fields_form_rows_resolve_and_close() {
    let cf_res = CustomField {
        id: 10,
        name: "Resolution Reason".to_string(),
        field_type: 0, // String
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 255,
        display_report: 0,
        display_update: 0,
        display_resolved: 1,
        display_closed: 0,
        require_report: 0,
        require_update: 0,
        require_resolved: 1,
        require_closed: 0,
        filter_by: 0,
    };

    let cf_close = CustomField {
        id: 11,
        name: "Closing Verification".to_string(),
        field_type: 0,
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 255,
        display_report: 0,
        display_update: 0,
        display_resolved: 0,
        display_closed: 1,
        require_report: 0,
        require_update: 0,
        require_resolved: 0,
        require_closed: 1,
        filter_by: 0,
    };

    let fields = vec![cf_res, cf_close];
    let values = std::collections::HashMap::new();
    let versions = vec![];
    let categories = vec![];

    // Resolve mode: shows Resolution Reason *
    let html_res = render_custom_fields_form_rows(
        &fields,
        &values,
        55,
        CustomFieldFormMode::Resolve,
        &versions,
        &categories,
    );
    assert!(html_res.contains("Resolution Reason *"));
    assert!(!html_res.contains("Closing Verification"));

    // Close mode: shows Closing Verification *
    let html_close = render_custom_fields_form_rows(
        &fields,
        &values,
        70,
        CustomFieldFormMode::Close,
        &versions,
        &categories,
    );
    assert!(!html_close.contains("Resolution Reason"));
    assert!(html_close.contains("Closing Verification *"));
}

#[test]
fn test_readonly_checkbox_multilist_renders_comma_separated() {
    let cf = CustomField {
        id: 20,
        name: "Platforms".to_string(),
        field_type: 5, // Checkbox
        possible_values: "Win|Mac|Linux".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 70,
        length_min: 0,
        length_max: 0,
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

    let rendered = render_custom_field_input(&cf, "Win|Linux", false, true, &[], &[]);
    assert!(rendered.contains("Win, Linux"));
    assert!(!rendered.contains("Win|Linux"));
}
