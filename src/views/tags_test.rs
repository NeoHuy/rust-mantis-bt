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
    let tags = vec![(
        Tag {
            id: 3,
            user_id: 1,
            name: "security".to_string(),
            description: "Security relevant issues".to_string(),
            date_created: 1700000000,
            date_updated: 1700000000,
        },
        5, // 5 attached issues
    )];
    let out = render_manage_tags_page(&ctx, &tags, Some("Tag created"), Some("Duplicate tag name"));
    let html = out.0;
    assert!(html.contains("security"));
    assert!(html.contains("Security relevant issues"));
    assert!(html.contains(r#"href="/tag_view_page/3""#));
    assert!(html.contains(r#"action="/tag_delete/3""#));
    assert!(html.contains(">5<"));
    assert!(html.contains("Duplicate tag name"));
    assert!(html.contains("Tag created"));
}

#[test]
fn test_render_tag_view_page_manager_and_reporter() {
    let tag = Tag {
        id: 7,
        user_id: 1,
        name: "ui-bug".to_string(),
        description: "User interface problems".to_string(),
        date_created: 1700000000,
        date_updated: 1700000000,
    };

    // 1. Manager view (includes Edit form)
    let mgr_ctx = dummy_ctx(70);
    let out_mgr = render_tag_view_page(&mgr_ctx, &tag, "alice", 12, Some("Saved"), None);
    let html_mgr = out_mgr.0;
    assert!(html_mgr.contains("ui-bug"));
    assert!(html_mgr.contains("User interface problems"));
    assert!(html_mgr.contains("alice"));
    assert!(html_mgr.contains("12 issue(s)"));
    assert!(html_mgr.contains(r#"action="/tag_update/7""#));
    assert!(html_mgr.contains(r#"href="/view_all_bug_page?tag_id=7""#));

    // 2. Reporter view (does NOT include Edit form)
    let rep_ctx = dummy_ctx(25);
    let out_rep = render_tag_view_page(&rep_ctx, &tag, "alice", 12, None, None);
    let html_rep = out_rep.0;
    assert!(html_rep.contains("ui-bug"));
    assert!(!html_rep.contains(r#"action="/tag_update/7""#));
}
