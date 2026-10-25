use super::*;
use crate::models::{Tag, User};

fn dummy_ctx(access_level: i32) -> AuthContext {
    let user = User {
        id: 1,
        username: "tester".to_string(),
        realname: "Tester".to_string(),
        email: "t@x.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level,
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
fn test_render_manage_tags_page_empty() {
    let ctx = dummy_ctx(25);
    let out = render_manage_tags_page(&ctx, &[], None, None);
    let html = out.0;
    assert!(html.contains("Manage Tags (0)"));
    assert!(html.contains("No tags defined."));
    assert!(html.contains("Create Tag"));
}

#[test]
fn test_render_manage_tags_page_with_tags_and_actions() {
    let ctx = dummy_ctx(70); // Manager
    let tags = vec![Tag {
        id: 3,
        user_id: 1,
        name: "security".to_string(),
        description: "Security relevant issues".to_string(),
        date_created: 1700000000,
        date_updated: 1700000000,
    }];
    let out = render_manage_tags_page(&ctx, &tags, Some("Tag created"), Some("Duplicate tag name"));
    let html = out.0;
    assert!(html.contains("security"));
    assert!(html.contains("Security relevant issues"));
    assert!(html.contains(r#"action="/tag_delete/3""#));
    assert!(html.contains("Duplicate tag name"));
    assert!(html.contains("Tag created"));
}
