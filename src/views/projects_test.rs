use super::*;
use crate::models::{Category, Project, ProjectVersion, User};

fn dummy_auth_ctx() -> AuthContext {
    let user = User {
        id: 1,
        username: "admin".to_string(),
        realname: "Admin User".to_string(),
        email: "admin@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 90,
        login_count: 1,
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
fn test_render_manage_proj_page() {
    let ctx = dummy_auth_ctx();
    let project = Project {
        id: 1,
        name: "Test Project".to_string(),
        status: 10,
        enabled: 1,
        view_state: 10,
        access_min: 10,
        description: "Project Description".to_string(),
        category_id: 0,
        inherit_global: 1,
    };
    let projects_hierarchy = vec![(project, 0)];
    let global_cats: Vec<(Category, String)> = vec![];
    let users: Vec<User> = vec![];

    let out = render_manage_proj_page(&ctx, &projects_hierarchy, &global_cats, &users);
    let html = out.0;
    assert!(html.contains("Test Project"));
    assert!(html.contains("Project Description"));
    assert!(html.contains("enabled"));
}

#[test]
fn test_render_proj_create_page() {
    let ctx = dummy_auth_ctx();
    let out = render_proj_create_page(&ctx);
    let html = out.0;
    assert!(html.contains("Create Project"));
    assert!(html.contains(r#"action="/manage_proj_create""#));
}

#[test]
fn test_render_proj_edit_page() {
    let ctx = dummy_auth_ctx();
    let project = Project {
        id: 42,
        name: "Alpha Project".to_string(),
        status: 50,
        enabled: 1,
        view_state: 10,
        access_min: 10,
        description: "Alpha details".to_string(),
        category_id: 0,
        inherit_global: 1,
    };
    let subprojects = vec![];
    let candidate_subprojs = vec![];
    let direct_cats: Vec<(Category, String)> = vec![];
    let versions: Vec<ProjectVersion> = vec![];
    let proj_users: Vec<(User, i32)> = vec![];
    let all_users: Vec<User> = vec![];

    let out = render_proj_edit_page(
        &ctx,
        &project,
        &subprojects,
        &candidate_subprojs,
        &direct_cats,
        &versions,
        &proj_users,
        &all_users,
    );
    let html = out.0;
    assert!(html.contains("Alpha Project"));
    assert!(html.contains("Alpha details"));
    assert!(html.contains(r#"action="/manage_proj_update/42""#));
}

#[test]
fn test_render_version_error() {
    let ctx = dummy_auth_ctx();
    let (status, out) = render_version_error(&ctx, 42, "Duplicate version name");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let html = out.0;
    assert!(html.contains("Failed to process version: Duplicate version name"));
    assert!(html.contains("/manage_proj_edit_page/42"));
}

#[test]
fn test_ui_m4_proj_add_version_released_unchecked_and_clean_status_labels() {
    let ctx = dummy_auth_ctx();
    let create_html = render_proj_create_page(&ctx).0;
    assert!(create_html.contains(">development</option>"));
    assert!(!create_html.contains("development (10)"));

    let project = Project {
        id: 42,
        name: "Alpha".to_string(),
        status: 10,
        enabled: 1,
        view_state: 10,
        access_min: 10,
        description: "".to_string(),
        category_id: 0,
        inherit_global: 1,
    };
    let edit_html = render_proj_edit_page(&ctx, &project, &[], &[], &[], &[], &[], &[]).0;
    assert!(edit_html.contains(r#"<input type="checkbox" name="released" value="1" /> Released"#));
    assert!(!edit_html.contains(r#"name="released" value="1" checked"#));
}

fn assert_no_nested_forms(html: &str) {
    let mut depth = 0usize;
    let mut rest = html;
    while let Some(pos) = rest.find('<') {
        rest = &rest[pos + 1..];
        let lower = rest.get(..6).unwrap_or(rest).to_ascii_lowercase();
        if lower.starts_with("form")
            && rest
                .as_bytes()
                .get(4)
                .is_none_or(|b| b.is_ascii_whitespace() || *b == b'>')
        {
            depth += 1;
            assert!(depth <= 1, "Nested <form> detected in HTML");
        } else if lower.starts_with("/form>") {
            depth = depth.saturating_sub(1);
        }
    }
}

#[test]
fn test_projects_views_no_inline_styles_or_js_and_wcag_buttons() {
    let ctx = dummy_auth_ctx();
    let project = Project {
        id: 42,
        name: "Alpha".to_string(),
        status: 10,
        enabled: 1,
        view_state: 10,
        access_min: 10,
        description: "Desc".to_string(),
        category_id: 0,
        inherit_global: 1,
    };
    let cat = Category {
        id: 5,
        project_id: 42,
        user_id: 1,
        name: "Backend".to_string(),
        status: 0,
    };
    let ver = ProjectVersion {
        id: 7,
        project_id: 42,
        version: "1.0.0".to_string(),
        description: "Initial".to_string(),
        date_order: 1700000000,
        released: 0,
        obsolete: 0,
    };

    let manage_html = render_manage_proj_page(
        &ctx,
        &[(project.clone(), 0)],
        &[(cat.clone(), "admin".to_string())],
        std::slice::from_ref(&ctx.user),
    )
    .0;
    assert!(!manage_html.contains("style="));
    assert!(!manage_html.contains("onsubmit="));
    assert!(manage_html.contains("btn btn-danger btn-white btn-round btn-sm"));
    assert!(manage_html.contains(r#"data-confirm="Delete category?""#));
    assert_no_nested_forms(&manage_html);

    let edit_html = render_proj_edit_page(
        &ctx,
        &project,
        &[(project.clone(), true)],
        &[],
        &[(cat, "admin".to_string())],
        &[ver],
        &[(ctx.user.clone(), 90)],
        std::slice::from_ref(&ctx.user),
    )
    .0;
    assert!(!edit_html.contains("style="));
    assert!(!edit_html.contains("onsubmit="));
    assert!(edit_html.contains("btn btn-danger btn-white btn-round btn-sm"));
    assert!(edit_html.contains("fa-trash-o"));
    assert_no_nested_forms(&edit_html);
}
