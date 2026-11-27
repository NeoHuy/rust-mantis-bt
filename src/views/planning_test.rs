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
    let groups: Vec<RoadmapVersionGroup> = Vec::new();
    let users_map = HashMap::new();

    let out = render_roadmap_page(&ctx, &groups, &users_map);
    let html = out.0;
    assert!(html.contains("Roadmap"));
    assert!(html.contains("No roadmap items with a Target Version found."));
}

#[test]
fn test_render_roadmap_page_with_items() {
    let ctx = dummy_ctx();
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
    let groups = vec![
        RoadmapVersionGroup {
            version: "1.0.0".to_string(),
            released: true,
            bugs: vec![bug],
            project_name: None,
            project_id: None,
            date_order: 0,
        },
        RoadmapVersionGroup {
            version: "2.0.0".to_string(),
            released: false,
            bugs: vec![],
            project_name: None,
            project_id: None,
            date_order: 0,
        },
    ];
    let mut users_map = HashMap::new();
    users_map.insert(1, "alice".to_string());

    let out = render_roadmap_page(&ctx, &groups, &users_map);
    let html = out.0;
    assert!(html.contains("1.0.0"));
    assert!(html.contains("Release v1.0.0 feature"));
    assert!(html.contains("100%"));
    assert!(html.contains("(alice)"));
    assert!(html.contains("Released"));
    assert!(html.contains("Planned"));
    assert!(!html.contains("badge-info"));
}

#[test]
fn test_render_roadmap_page_with_multiple_projects_shows_badge() {
    let ctx = dummy_ctx();
    let groups = vec![
        RoadmapVersionGroup {
            version: "1.0.0".to_string(),
            released: true,
            bugs: vec![],
            project_name: Some("Project Alpha".to_string()),
            project_id: Some(1),
            date_order: 0,
        },
        RoadmapVersionGroup {
            version: "1.0.0".to_string(),
            released: false,
            bugs: vec![],
            project_name: Some("Project Beta".to_string()),
            project_id: Some(2),
            date_order: 0,
        },
    ];
    let users_map = HashMap::new();

    let out = render_roadmap_page(&ctx, &groups, &users_map);
    let html = out.0;
    assert!(html.contains("Project Alpha"));
    assert!(html.contains("Project Beta"));
    assert!(html.contains("badge-info"));
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
    assert!(html.contains("btn btn-danger btn-white btn-round btn-sm"));
    assert!(html.contains("confirm('Delete queued email?')"));
}

#[test]
fn test_render_news_edit_page_and_news_page_edit_link() {
    let ctx = dummy_ctx();
    let news = NewsItem {
        id: 42,
        project_id: 1,
        poster_id: 1,
        date_posted: 1700000000,
        last_modified: 1700000000,
        view_state: 10,
        announcement: 1,
        headline: "Server Migration".to_string(),
        body: "Migration scheduled tonight.".to_string(),
    };

    // 1. News edit page renders prefilled form
    let out_edit = render_news_edit_page(&ctx, &news, Some("Saved"), None);
    let html_edit = out_edit.0;
    assert!(html_edit.contains(r#"action="/news_update/42""#));
    assert!(html_edit.contains("Server Migration"));
    assert!(html_edit.contains("Migration scheduled tonight."));
    assert!(html_edit.contains("checked")); // Announcement checkbox checked

    // 2. News list renders Edit link for Manager (eff >= 70)
    let mut users_map = HashMap::new();
    users_map.insert(1, "admin".to_string());
    let out_list_mgr = render_news_page(
        &ctx,
        std::slice::from_ref(&news),
        &users_map,
        70,
        None,
        None,
    );
    assert!(out_list_mgr.0.contains(r#"href="/news_edit_page/42""#));

    // 3. News list does NOT render Edit link for Reporter (eff = 25)
    let out_list_rep = render_news_page(&ctx, &[news], &users_map, 25, None, None);
    assert!(!out_list_rep.0.contains(r#"href="/news_edit_page/42""#));
    assert!(out_list_mgr.0.contains(r#"class="widget-toolbar""#));
    assert!(out_list_mgr.0.contains("btn-white btn-round btn-sm"));
    assert!(out_list_mgr
        .0
        .contains("btn btn-danger btn-white btn-round btn-sm"));
}

#[test]
fn test_render_summary_page_ratio_tables() {
    let ctx = dummy_ctx();
    let mut by_status = BTreeMap::new();
    by_status.insert(
        10,
        SummaryCounts {
            open: 3,
            resolved: 0,
            closed: 0,
        },
    );
    let by_sev = BTreeMap::new();
    let by_pri = BTreeMap::new();
    let by_cat = BTreeMap::new();
    let by_res = BTreeMap::new();
    let by_rep = BTreeMap::new();
    let by_hnd = BTreeMap::new();

    let html = render_summary_page(
        &ctx, 3, 3, 0, 0, &by_status, &by_sev, &by_pri, &by_cat, &by_res, &by_rep, &by_hnd,
    )
    .0;
    assert!(html.contains("By Status"));
    assert!(html.contains("By Category"));
    assert!(html.contains("By Resolution"));
    assert!(html.contains("<th>Open</th><th>Resolved</th><th>Closed</th><th>Total</th>"));
}
