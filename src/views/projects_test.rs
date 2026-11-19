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
