use super::*;
use crate::models::{Bug, EmailMessage, User};

fn dummy_ctx() -> AuthContext {
    let user = User {
        id: 1,
        username: "lead".to_string(),
        realname: "Tech Lead".to_string(),
        email: "lead@x.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 70, // Manager
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
fn test_planning_labels() {
    assert_eq!(status_label(10), "new");
    assert_eq!(status_label(80), "resolved");
    assert_eq!(status_label(999), "new");

    assert_eq!(priority_label(30), "normal");
    assert_eq!(priority_label(999), "normal");

    assert_eq!(severity_label(50), "minor");
    assert_eq!(severity_label(999), "minor");

    assert_eq!(resolution_label(10), "open");
    assert_eq!(resolution_label(999), "open");
}

#[test]
fn test_render_roadmap_page_empty() {
    let ctx = dummy_ctx();
    let by_target = BTreeMap::new();
    let users_map = HashMap::new();

    let out = render_roadmap_page(&ctx, &by_target, &users_map);
    let html = out.0;
    assert!(html.contains("Roadmap"));
    assert!(html.contains("No roadmap items with a Target Version found."));
}

#[test]
fn test_render_roadmap_page_with_items() {
    let ctx = dummy_ctx();
    let mut by_target = BTreeMap::new();
    let bug = Bug {
        id: 12,
        project_id: 1,
        reporter_id: 1,
        handler_id: 1,
        duplicate_id: 0,
        priority: 40,
        severity: 60,
        reproducibility: 10,
        status: 80, // resolved
        resolution: 20,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        build: "".to_string(),
        view_state: 10,
        summary: "Release v1.0.0 feature".to_string(),
        sticky: 0,
        target_version: "1.0.0".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000000,
    };
    by_target.insert("1.0.0".to_string(), vec![bug]);
    let mut users_map = HashMap::new();
    users_map.insert(1, "alice".to_string());

    let out = render_roadmap_page(&ctx, &by_target, &users_map);
    let html = out.0;
    assert!(html.contains("1.0.0"));
    assert!(html.contains("Release v1.0.0 feature"));
    assert!(html.contains("100%"));
    assert!(html.contains("(alice)"));
}

#[test]
fn test_render_manage_email_queue_page() {
    let ctx = dummy_ctx();
    let emails = vec![EmailMessage {
        email_id: 1,
        email: "user@example.com".to_string(),
        subject: "Issue #1 assigned".to_string(),
        body: "You have been assigned to bug #1".to_string(),
        submitted: 1700000000,
        metadata: "".to_string(),
    }];

    let out = render_manage_email_queue_page(&ctx, &emails, Some("Queue checked"));
    let html = out.0;
    assert!(html.contains("Email Notification Queue (1)"));
    assert!(html.contains("user@example.com"));
    assert!(html.contains("Issue #1 assigned"));
    assert!(html.contains(r#"action="/manage_email_delete/1""#));
}
