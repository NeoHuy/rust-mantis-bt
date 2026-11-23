use super::*;
use crate::models::Bug;

#[test]
fn test_labels() {
    assert_eq!(status_label(10), "new");
    assert_eq!(status_label(90), "closed");
    assert_eq!(status_label(999), "unknown");

    assert_eq!(priority_label(30), "normal");
    assert_eq!(priority_label(50), "urgent");
    assert_eq!(priority_label(60), "immediate");
    assert_eq!(priority_label(999), "unknown");

    assert_eq!(severity_label(50), "minor");
    assert_eq!(severity_label(80), "block");
    assert_eq!(severity_label(999), "unknown");

    assert_eq!(reproducibility_label(10), "always");
    assert_eq!(reproducibility_label(70), "have not tried");
    assert_eq!(reproducibility_label(999), "unknown");

    assert_eq!(resolution_label(10), "open");
    assert_eq!(resolution_label(20), "fixed");
    assert_eq!(resolution_label(999), "unknown");

    assert_eq!(relationship_label(0), "related to");
    assert_eq!(relationship_label(1), "duplicate of");
    assert_eq!(relationship_label(999), "related to");
}

#[test]
fn test_render_my_view_box_empty() {
    let out = render_my_view_box("Unassigned", "/view_all_bug_page", &[]);
    assert!(out.contains("Unassigned"));
    assert!(out.contains("No issues found."));
    assert!(out.contains(r#"href="/view_all_bug_page""#));
}

#[test]
fn test_render_my_view_box_with_issues() {
    let sample_bug = Bug {
        id: 7,
        project_id: 1,
        reporter_id: 1,
        handler_id: 2,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "1.0".to_string(),
        fixed_in_version: "".to_string(),
        build: "".to_string(),
        view_state: 10,
        summary: "Crash on startup".to_string(),
        sticky: 0,
        target_version: "".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000000,
    };

    let out = render_my_view_box("Recent", "/view_all_bug_page", &[sample_bug]);
    assert!(out.contains("Recent"));
    assert!(out.contains("0000007"));
    assert!(out.contains("Crash on startup"));
}

fn dummy_ctx(user_id: i64, access_level: i32) -> AuthContext {
    let user = User {
        id: user_id,
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
fn test_render_view_all_bug_page_pagination_preserves_query() {
    let ctx = dummy_ctx(1, 25);
    let sample_bug = Bug {
        id: 15,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 40,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        build: "".to_string(),
        view_state: 10,
        summary: "Pagination test bug".to_string(),
        sticky: 0,
        target_version: "".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000000,
    };

    let q = ListBugsQuery {
        page: Some(2),
        q: Some("deadlock".to_string()),
        priority: Some(40),
        sort: Some("priority".to_string()),
        dir: Some("desc".to_string()),
        per_page: Some(10),
        ..Default::default()
    };

    let filter = BugFilter {
        search: Some("deadlock".to_string()),
        priority: Some(40),
        sort: Some("priority".to_string()),
        dir: Some("desc".to_string()),
        per_page: Some(10),
        ..Default::default()
    };

    let Html(html) = render_view_all_bug_page(
        &ctx,
        &[sample_bug],
        35, // total 35 bugs, page 2 of 4 (per_page 10)
        2,
        10,
        &filter,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        &[],
        None,
        None,
        &q,
    );

    // Prev link must point to page=1 and preserve q, priority, sort, dir, per_page
    assert!(html.contains("« Prev"));
    assert!(html.contains(r#"href="/view_all_bug_page?page=1"#));
    assert!(html.contains("q=deadlock"));
    assert!(html.contains("priority=40"));
    assert!(html.contains("sort=priority"));
    assert!(html.contains("dir=desc"));
    assert!(html.contains("per_page=10"));

    // Next link must point to page=3 and preserve same parameters
    assert!(html.contains("Next »"));
    assert!(html.contains(r#"href="/view_all_bug_page?page=3"#));
}

#[test]
fn test_render_view_all_bug_page_sort_headers_and_per_page_controls() {
    let ctx = dummy_ctx(1, 25);
    let q = ListBugsQuery {
        sort: Some("summary".to_string()),
        dir: Some("asc".to_string()),
        per_page: Some(50),
        ..Default::default()
    };

    let Html(html) = render_view_all_bug_page(
        &ctx,
        &[],
        0,
        1,
        50,
        &BugFilter::default(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        &[],
        None,
        None,
        &q,
    );

    // Sort column header for summary should be active with asc indicator (&#9650;)
    assert!(html.contains("Summary &#9650;"));
    // Clicking it toggles to desc
    assert!(html.contains("sort=summary&amp;dir=desc") || html.contains("sort=summary&dir=desc"));

    // ID sort link switches to sort=id
    assert!(html.contains("sort=id"));

    // Per-page control has options 10, 25, 50, 100
    assert!(html.contains("Per page:"));
    assert!(html.contains("per_page=10"));
    assert!(html.contains("per_page=25"));
    assert!(html.contains("per_page=100"));
    // 50 is active
    assert!(html.contains(r#"<span class="btn btn-secondary btn-sm" style="font-weight:bold; background:#e0e0e0; pointer-events:none;">50</span>"#));

    // CSV export link preserves sort parameters
    assert!(
        html.contains(r#"href="/csv_export?sort=summary&amp;dir=asc""#)
            || html.contains(r#"href="/csv_export?sort=summary&dir=asc""#)
    );
}

#[test]
fn test_render_view_all_bug_page_saved_filter_delete_button_permissions() {
    let filter_owner_id = 10;
    let saved_filter = SavedFilter {
        id: 99,
        user_id: filter_owner_id,
        project_id: 1,
        is_public: 1,
        name: "Team Filter".to_string(),
        filter_string: "{}".to_string(),
    };

    // 1. Filter owner (access 25) sees Delete button
    let ctx_owner = dummy_ctx(filter_owner_id, 25);
    let Html(html_owner) = render_view_all_bug_page(
        &ctx_owner,
        &[],
        0,
        1,
        25,
        &BugFilter::default(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        std::slice::from_ref(&saved_filter),
        None,
        None,
        &ListBugsQuery::default(),
    );
    assert!(html_owner.contains(r#"action="/filter_delete/99""#));
    assert!(html_owner.contains("Delete</button>"));

    // 2. Other reporter (access 25) does NOT see Delete button
    let ctx_other = dummy_ctx(20, 25);
    let Html(html_other) = render_view_all_bug_page(
        &ctx_other,
        &[],
        0,
        1,
        25,
        &BugFilter::default(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        std::slice::from_ref(&saved_filter),
        None,
        None,
        &ListBugsQuery::default(),
    );
    assert!(html_other.contains("Team Filter"));
    assert!(!html_other.contains(r#"action="/filter_delete/99""#));

    // 3. Manager (access 70) sees Delete button even though not owner
    let ctx_manager = dummy_ctx(20, 70);
    let Html(html_manager) = render_view_all_bug_page(
        &ctx_manager,
        &[],
        0,
        1,
        25,
        &BugFilter::default(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        std::slice::from_ref(&saved_filter),
        None,
        None,
        &ListBugsQuery::default(),
    );
    assert!(html_manager.contains(r#"action="/filter_delete/99""#));

    // 4. Admin (access 90) sees Delete button
    let ctx_admin = dummy_ctx(20, 90);
    let Html(html_admin) = render_view_all_bug_page(
        &ctx_admin,
        &[],
        0,
        1,
        25,
        &BugFilter::default(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        std::slice::from_ref(&saved_filter),
        None,
        None,
        &ListBugsQuery::default(),
    );
    assert!(html_admin.contains(r#"action="/filter_delete/99""#));
}
