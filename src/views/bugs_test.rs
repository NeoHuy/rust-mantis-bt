use super::*;
use crate::models::{Bug, Project, RevisionType};

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
    assert!(out.contains(">Unassigned</a>"));
    assert!(!out.contains("Unassigned (0)"));
    assert!(out.contains(">0</span>"));
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
    assert!(out.contains(">Recent</a>"));
    assert!(!out.contains("Recent (1)"));
    assert!(out.contains(">1 - 1 / 1</span>"));
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
        &ViewIssuesMeta::default(),
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
        &ViewIssuesMeta::default(),
    );

    // Sort column header for summary should be active with asc indicator (fa-caret-up)
    assert!(html.contains("Summary <i class=\"fa fa-caret-up"));
    // Clicking it toggles to desc
    assert!(html.contains("sort=summary&amp;dir=desc") || html.contains("sort=summary&dir=desc"));

    // ID sort link switches to sort=id
    assert!(html.contains("sort=id"));
    assert!(html.contains("sort=last_updated"));

    // Per-page control has options 10, 25, 50, 100
    assert!(html.contains("Per page:"));
    assert!(html.contains("per_page=10"));
    assert!(html.contains("per_page=25"));
    assert!(html.contains("per_page=100"));
    // 50 is active with high-contrast primary style
    assert!(html.contains(
        r#"<span class="btn btn-primary btn-sm font-bold pointer-events-none">50</span>"#
    ));

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
        &ViewIssuesMeta::default(),
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
        &ViewIssuesMeta::default(),
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
        &ViewIssuesMeta::default(),
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
        &ViewIssuesMeta::default(),
    );
    assert!(html_admin.contains(r#"action="/filter_delete/99""#));
}

#[test]
fn test_human_readable_history() {
    let mut users_map = HashMap::new();
    users_map.insert(2, "john_dev".to_string());

    let mut categories_map = HashMap::new();
    categories_map.insert(5, "Database".to_string());

    // Status
    assert_eq!(
        format_history_value("status", "10", &users_map, &categories_map),
        "new"
    );
    assert_eq!(
        format_history_value("status", "50", &users_map, &categories_map),
        "assigned"
    );

    // Resolution
    assert_eq!(
        format_history_value("resolution", "10", &users_map, &categories_map),
        "open"
    );
    assert_eq!(
        format_history_value("resolution", "20", &users_map, &categories_map),
        "fixed"
    );

    // Priority
    assert_eq!(
        format_history_value("priority", "30", &users_map, &categories_map),
        "normal"
    );
    assert_eq!(
        format_history_value("priority", "50", &users_map, &categories_map),
        "urgent"
    );

    // Severity
    assert_eq!(
        format_history_value("severity", "50", &users_map, &categories_map),
        "minor"
    );
    assert_eq!(
        format_history_value("severity", "70", &users_map, &categories_map),
        "crash"
    );

    // Reproducibility
    assert_eq!(
        format_history_value("reproducibility", "70", &users_map, &categories_map),
        "have not tried"
    );
    assert_eq!(
        format_history_value("reproducibility", "10", &users_map, &categories_map),
        "always"
    );

    // View state
    assert_eq!(
        format_history_value("view_state", "10", &users_map, &categories_map),
        "public"
    );
    assert_eq!(
        format_history_value("view_state", "50", &users_map, &categories_map),
        "private"
    );

    // Sticky
    assert_eq!(
        format_history_value("sticky", "1", &users_map, &categories_map),
        "yes"
    );
    assert_eq!(
        format_history_value("sticky", "0", &users_map, &categories_map),
        "no"
    );

    // Due date
    assert_eq!(
        format_history_value("due_date", "0", &users_map, &categories_map),
        "none"
    );
    assert_eq!(
        format_history_value("due_date", "1709164800", &users_map, &categories_map),
        "2024-02-29"
    );

    // Handler ID
    assert_eq!(
        format_history_value("handler_id", "0", &users_map, &categories_map),
        "none"
    );
    assert_eq!(
        format_history_value("handler_id", "2", &users_map, &categories_map),
        "john_dev"
    );
    assert_eq!(
        format_history_value("handler_id", "99", &users_map, &categories_map),
        "user#99"
    );

    // Category ID
    assert_eq!(
        format_history_value("category_id", "0", &users_map, &categories_map),
        "none"
    );
    assert_eq!(
        format_history_value("category_id", "5", &users_map, &categories_map),
        "Database"
    );
    assert_eq!(
        format_history_value("category_id", "99", &users_map, &categories_map),
        "category#99"
    );

    // Test render_bug_history_html empty
    let empty_html = render_bug_history_html(&[], &users_map, &categories_map);
    assert!(empty_html.contains("No history entries."));

    // Test render_bug_history_html with changes
    let history = vec![
        BugHistory {
            id: 1,
            user_id: 2,
            bug_id: 10,
            date_modified: 1700000000,
            field_name: "".to_string(),
            old_value: "".to_string(),
            new_value: "".to_string(),
            type_: 1,
        },
        BugHistory {
            id: 2,
            user_id: 2,
            bug_id: 10,
            date_modified: 1700000100,
            field_name: "status".to_string(),
            old_value: "10".to_string(),
            new_value: "50".to_string(),
            type_: 0,
        },
        BugHistory {
            id: 3,
            user_id: 2,
            bug_id: 10,
            date_modified: 1700000200,
            field_name: "sticky".to_string(),
            old_value: "0".to_string(),
            new_value: "1".to_string(),
            type_: 0,
        },
    ];

    let rendered = render_bug_history_html(&history, &users_map, &categories_map);
    assert!(rendered.contains("New Issue"));
    assert!(rendered.contains("new =&gt; assigned"));
    assert!(rendered.contains("no =&gt; yes"));
    assert!(rendered.contains("john_dev"));
}

#[test]
fn test_view_bug_page_build_due_date_overdue_and_sticky() {
    let dummy_user = User {
        id: 1,
        username: "admin".to_string(),
        realname: "Admin".to_string(),
        email: "admin@example.com".to_string(),
        password: "".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 90,
        login_count: 0,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "".to_string(),
        last_visit: 0,
        date_created: 0,
    };

    let ctx = AuthContext {
        user: dummy_user,
        active_project_id: 1,
        visible_projects: Vec::new(),
    };

    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 70,
        status: 10, // New (< 80)
        resolution: 10,
        os: "macOS".to_string(),
        os_build: "14.5".to_string(),
        platform: "arm64".to_string(),
        version: "1.0".to_string(),
        fixed_in_version: "".to_string(),
        build: "build-beta-9".to_string(),
        view_state: 10,
        summary: "Important issue".to_string(),
        sticky: 1,
        target_version: "".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 86400, // Past epoch (1970-01-02), overdue!
        last_updated: 1700000000,
    };

    let full = BugFull {
        bug: bug.clone(),
        description: "Desc".to_string(),
        steps_to_reproduce: "".to_string(),
        additional_information: "".to_string(),
    };

    let html = render_view_bug_page(
        &ctx,
        &full,
        "SampleProj",
        "General",
        "admin",
        "none",
        90,
        &[],
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        None,
        "",
        None,
        None,
        &crate::views::formatting::FormatContext::default(),
    );

    let html_str = html.0;
    assert!(html_str.contains("build-beta-9"));
    assert!(html_str.contains("1970-01-02"));
    assert!(html_str.contains("Overdue"));
    assert!(html_str.contains("[Sticky]"));

    // Check sticky in render_view_all_bug_page
    let view_all_html = render_view_all_bug_page(
        &ctx,
        &[bug],
        1,
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
        &ListBugsQuery::default(),
        &ViewIssuesMeta::default(),
    );
    assert!(view_all_html.0.contains("[Sticky]"));
    assert!(view_all_html.0.contains("fa-thumb-tack"));
}

#[test]
fn test_render_status_change_form_with_resolve_close_custom_fields() {
    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 50, // assigned (< 80)
        resolution: 10,
        os: String::new(),
        os_build: String::new(),
        platform: String::new(),
        version: String::new(),
        fixed_in_version: String::new(),
        build: String::new(),
        view_state: 10,
        summary: "Status test".to_string(),
        sticky: 0,
        target_version: String::new(),
        category_id: 1,
        date_submitted: 1000,
        due_date: 0,
        last_updated: 1000,
    };

    let cf_res = CustomField {
        id: 101,
        name: "Root Cause".to_string(),
        field_type: 0,
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 55,
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
        id: 102,
        name: "Close Signoff".to_string(),
        field_type: 0,
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: "".to_string(),
        access_level_r: 10,
        access_level_rw: 70,
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

    let custom_fields = vec![cf_res, cf_close];
    let cf_values = vec![];
    let versions = vec![];
    let categories = vec![];

    // Developer (eff 55) viewing unresolved bug:
    // Can resolve (eff 55), so display_resolved field "Root Cause *" is rendered
    // Cannot close (eff 55 < 70), so display_closed field is omitted
    let form_dev =
        render_status_change_form(&bug, 55, &custom_fields, &cf_values, &versions, &categories);
    assert!(form_dev.contains(r#"action="/bug_change_status/42""#));
    assert!(form_dev.contains("Root Cause *"));
    assert!(form_dev.contains(r#"name="custom_field_101""#));
    assert!(!form_dev.contains("Close Signoff"));
    // Inputs must NOT have native 'required' attribute to avoid blocking intermediate status changes
    assert!(!form_dev.contains(" required"));

    // Manager (eff 70) viewing unresolved bug:
    // Can resolve and close, so both fields are rendered
    let form_mgr =
        render_status_change_form(&bug, 70, &custom_fields, &cf_values, &versions, &categories);
    assert!(form_mgr.contains("Root Cause *"));
    assert!(form_mgr.contains("Close Signoff *"));
    assert!(form_mgr.contains(r#"name="custom_field_102""#));
    assert!(!form_mgr.contains(" required"));
}

fn assert_no_nested_forms(html: &str) {
    let mut depth = 0;
    let mut rest = html;
    while !rest.is_empty() {
        let open_pos = rest.find("<form");
        let close_pos = rest.find("</form");
        match (open_pos, close_pos) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (Some(_), Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (Some(o), None) => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (None, Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (None, None) => break,
        }
    }
}

#[test]
fn test_render_bugnotes_cards_and_actions() {
    let mut users_map = HashMap::new();
    users_map.insert(1, "alice".to_string());
    users_map.insert(2, "bob".to_string());

    let note_pub = Bugnote {
        id: 101,
        bug_id: 10,
        reporter_id: 1,
        bugnote_text_id: 1,
        view_state: 10,
        date_submitted: 1600000000,
        last_modified: 1600000000,
        note_type: 0,
        note_attr: String::new(),
        time_tracking: 90,
        note: "Hello public note".to_string(),
    };

    let note_priv = Bugnote {
        id: 102,
        bug_id: 10,
        reporter_id: 2,
        bugnote_text_id: 2,
        view_state: 50,
        date_submitted: 1600001000,
        last_modified: 1600001000,
        note_type: 0,
        note_attr: String::new(),
        time_tracking: 0,
        note: "Secret dev note".to_string(),
    };

    let notes = vec![note_pub, note_priv];

    // Reporter (eff 25, user 1):
    // Can edit note 101 (own note), cannot edit note 102
    // Cannot toggle visibility (requires eff >= 55)
    // Can see revisions link for both
    let mut access_map = HashMap::new();
    access_map.insert(1, 25);
    access_map.insert(2, 60); // custom numeric threshold falls back to "developer"
    let empty_fmt = crate::views::formatting::FormatContext::default();
    let table_rep = render_bugnotes_cards(&notes, &users_map, &access_map, 25, 1, &empty_fmt);
    assert!(table_rep.contains("alice"));
    assert!(table_rep.contains("reporter"));
    assert!(table_rep.contains("developer"));
    assert!(table_rep.contains("Hello public note"));
    assert!(table_rep.contains(r#"href="/bugnote_edit_page/101""#));
    assert!(!table_rep.contains(r#"href="/bugnote_edit_page/102""#));
    assert!(!table_rep.contains(r#"action="/bugnote_set_view_state/101""#));
    assert!(table_rep.contains(r#"href="/bug_revision_view_page/10?bugnote_id=101""#));
    assert!(table_rep.contains(r#"href="/bug_revision_view_page/10?bugnote_id=102""#));

    // Developer (eff 55, user 3):
    // Can toggle visibility for note 101 (Make Private) and note 102 (Make Public)
    let table_dev = render_bugnotes_cards(&notes, &users_map, &access_map, 55, 3, &empty_fmt);
    assert!(table_dev.contains(r#"action="/bugnote_set_view_state/101""#));
    assert!(table_dev.contains("Make Private"));
    assert!(table_dev.contains(r#"action="/bugnote_set_view_state/102""#));
    assert!(table_dev.contains("Make Public"));

    // Manager (eff 70, user 3):
    // Can edit and delete any note
    let table_mgr = render_bugnotes_cards(&notes, &users_map, &access_map, 70, 3, &empty_fmt);
    assert!(table_mgr.contains(r#"href="/bugnote_edit_page/101""#));
    assert!(table_mgr.contains(r#"href="/bugnote_edit_page/102""#));
    assert!(table_mgr.contains(r#"action="/bugnote_delete/101""#));
    assert!(table_mgr.contains(r#"action="/bugnote_delete/102""#));
    assert!(table_mgr.contains(r#"data-confirm="Delete this note?""#));
}

#[test]
fn test_render_bugnote_edit_page_layout() {
    let ctx = dummy_ctx(1, 25);

    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        category_id: 1,
        date_submitted: 1600000000,
        last_updated: 1600000000,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        target_version: "".to_string(),
        build: "".to_string(),
        summary: "Bug summary".to_string(),
        view_state: 10,
        sticky: 0,
        due_date: 0,
    };

    let note = Bugnote {
        id: 88,
        bug_id: 42,
        reporter_id: 1,
        bugnote_text_id: 88,
        view_state: 10,
        date_submitted: 1600000000,
        last_modified: 1600000000,
        note_type: 0,
        note_attr: String::new(),
        time_tracking: 0,
        note: "Original Note <content>".to_string(),
    };

    let page = render_bugnote_edit_page(&ctx, &bug, &note, Some("Sample error"));
    let html = page.0;
    assert!(html.contains("Sample error"));
    assert!(html.contains(r#"action="/bugnote_update/88""#));
    assert!(html.contains("Original Note &lt;content&gt;"));
    assert!(html.contains(r#"href="/view_bug_page/42""#));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_bug_revision_view_page_layout() {
    let ctx = dummy_ctx(1, 70);

    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        category_id: 1,
        date_submitted: 1600000000,
        last_updated: 1600000000,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        target_version: "".to_string(),
        build: "".to_string(),
        summary: "Bug summary".to_string(),
        view_state: 10,
        sticky: 0,
        due_date: 0,
    };

    let rev1 = BugRevision {
        id: 1,
        bug_id: 42,
        bugnote_id: 0,
        user_id: 1,
        timestamp: 1600000050,
        rev_type: RevisionType::Description as i32,
        value: "Old <description>".to_string(),
    };

    let rev2 = BugRevision {
        id: 2,
        bug_id: 42,
        bugnote_id: 88,
        user_id: 1,
        timestamp: 1600000060,
        rev_type: RevisionType::Bugnote as i32,
        value: "Old <note> text".to_string(),
    };

    let mut users_map = HashMap::new();
    users_map.insert(1, "tester".to_string());

    // Manager can_drop = true:
    let page_mgr = render_bug_revision_view_page(
        &ctx,
        &bug,
        &[rev1, rev2.clone()],
        &users_map,
        true,
        None,
        None,
    );
    let html_mgr = page_mgr.0;
    assert!(html_mgr.contains("Description"));
    assert!(html_mgr.contains("Old &lt;description&gt;"));
    assert!(html_mgr.contains("Bugnote"));
    assert!(html_mgr.contains("Old &lt;note&gt; text"));
    assert!(html_mgr.contains(r#"action="/bug_revision_drop/1""#));
    assert!(html_mgr.contains(r#"action="/bug_revision_drop/2""#));
    assert_no_nested_forms(&html_mgr);

    // Non-manager can_drop = false:
    let page_non_mgr =
        render_bug_revision_view_page(&ctx, &bug, &[rev2], &users_map, false, Some(88), None);
    let html_non_mgr = page_non_mgr.0;
    assert!(!html_non_mgr.contains("Drop Revision"));
    assert_no_nested_forms(&html_non_mgr);
}

#[test]
fn test_render_update_bug_page_optimistic_locking() {
    let ctx = dummy_ctx(1, 70);

    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        category_id: 1,
        date_submitted: 1600000000,
        last_updated: 1623715200,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        target_version: "".to_string(),
        build: "".to_string(),
        summary: "Bug summary".to_string(),
        view_state: 10,
        sticky: 0,
        due_date: 0,
    };

    let full = BugFull {
        bug,
        description: "Desc".to_string(),
        steps_to_reproduce: "Steps".to_string(),
        additional_information: "AddInfo".to_string(),
    };

    let page = render_update_bug_page(&ctx, &full, "", &[], &[], 70, &[], &[]);
    let html = page.0;
    assert!(html.contains(r#"<input type="hidden" name="last_updated" value="1623715200">"#));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_conflict_page() {
    let ctx = dummy_ctx(1, 70);
    let html = render_conflict_page(&ctx, 42, "Modified by another user").0;
    assert!(html.contains("Conflict"));
    assert!(html.contains("Modified by another user"));
    assert!(html.contains(r#"href="/view_bug_page/42""#));
    assert!(html.contains(r#"href="/update_bug_page/42""#));
    assert!(!html.contains("javascript:"));
}

#[test]
fn test_render_bug_clone_and_move_pages_and_no_nested_forms() {
    let mut ctx = dummy_ctx(1, 55);
    ctx.active_project_id = 1;
    ctx.visible_projects = vec![
        (
            Project {
                id: 1,
                name: "Project A".to_string(),
                status: 10,
                enabled: 1,
                view_state: 10,
                access_min: 10,
                description: "".to_string(),
                category_id: 1,
                inherit_global: 1,
            },
            55,
        ),
        (
            Project {
                id: 2,
                name: "Project B".to_string(),
                status: 10,
                enabled: 1,
                view_state: 10,
                access_min: 10,
                description: "".to_string(),
                category_id: 1,
                inherit_global: 1,
            },
            55,
        ),
    ];

    let full = BugFull {
        bug: Bug {
            id: 42,
            project_id: 1,
            reporter_id: 1,
            handler_id: 1,
            duplicate_id: 0,
            priority: 30,
            severity: 50,
            reproducibility: 10,
            status: 10,
            resolution: 10,
            category_id: 1,
            date_submitted: 1700000000,
            last_updated: 1700000000,
            os: "Linux".to_string(),
            os_build: "".to_string(),
            platform: "x86_64".to_string(),
            version: "1.0".to_string(),
            fixed_in_version: "".to_string(),
            target_version: "".to_string(),
            build: "".to_string(),
            summary: "Clone and Move Test Bug".to_string(),
            view_state: 10,
            sticky: 0,
            due_date: 0,
        },
        description: "Test description".to_string(),
        steps_to_reproduce: "".to_string(),
        additional_information: "".to_string(),
    };

    let target_projects = vec![
        (1, "Project A".to_string(), true),
        (2, "Project B".to_string(), false),
    ];

    // 1. Clone page
    let Html(clone_html) = render_bug_clone_page(
        &ctx,
        &full,
        "Project A",
        &target_projects,
        None,
        Some("Ready to clone"),
    );
    assert!(clone_html.contains("Clone Issue #0000042"));
    assert!(clone_html.contains(r#"action="/bug_clone/42""#));
    assert!(clone_html.contains(r#"name="target_project_id""#));
    assert!(clone_html.contains(r#"name="copy_relationships""#));
    assert!(clone_html.contains(r#"name="copy_files""#));
    assert_no_nested_forms(&clone_html);

    // 2. Move page
    let Html(move_html) =
        render_bug_move_page(&ctx, &full, "Project A", &target_projects[1..], None, None);
    assert!(move_html.contains("Move Issue #0000042"));
    assert!(move_html.contains(r#"action="/bug_move/42""#));
    assert!(move_html.contains(r#"name="target_project_id""#));
    assert_no_nested_forms(&move_html);
}

#[test]
fn test_render_view_all_bug_page_bulk_action_bar_and_no_nested_forms() {
    let mut ctx = dummy_ctx(1, 90);
    ctx.active_project_id = 1;
    ctx.visible_projects = vec![(
        Project {
            id: 1,
            name: "Project A".to_string(),
            status: 10,
            enabled: 1,
            view_state: 10,
            access_min: 10,
            description: "".to_string(),
            category_id: 1,
            inherit_global: 1,
        },
        90,
    )];

    let sample_bug = Bug {
        id: 100,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        category_id: 1,
        date_submitted: 1700000000,
        last_updated: 1700000000,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        target_version: "".to_string(),
        build: "".to_string(),
        summary: "Sample Issue for Bulk Action".to_string(),
        view_state: 10,
        sticky: 0,
        due_date: 0,
    };

    let q = ListBugsQuery::default();
    let filter = BugFilter::default();

    let saved_filters = vec![SavedFilter {
        id: 5,
        user_id: 1,
        project_id: 1,
        name: "My Filter".to_string(),
        filter_string: "".to_string(),
        is_public: 0,
    }];

    let Html(html) = render_view_all_bug_page(
        &ctx,
        &[sample_bug],
        1,
        1,
        10,
        &filter,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &[],
        &[],
        &saved_filters,
        None,
        None,
        &q,
        &ViewIssuesMeta::default(),
    );

    // Verify checkbox column and select all checkbox
    assert!(html.contains(r#"id="select_all_bugs""#));
    assert!(html.contains(r#"name="bug_arr" value="100""#));

    // Verify bulk action bar (wired via mantis.js, no inline JS handlers)
    assert!(html.contains(r#"id="bulk_action_form""#));
    assert!(html.contains(r#"action="/bug_actiongroup""#));
    assert!(html.contains(r#"name="return_query""#));
    assert!(html.contains(r#"name="action" id="bulk_action_select""#));
    assert!(html.contains(r#">-- Select Action --</option>"#));
    assert!(!html.contains("onsubmit="));
    assert!(!html.contains("onchange="));
    assert!(!html.contains("onclick="));
    assert!(html.contains(r#"value="MOVE">Move</option>"#));
    assert!(html.contains(r#"value="COPY">Copy</option>"#));
    assert!(html.contains(r#"value="ASSIGN">Assign</option>"#));
    assert!(html.contains(r#"value="CLOSE">Close</option>"#));
    assert!(html.contains(r#"value="DELETE">Delete</option>"#));
    assert!(html.contains(r#"value="RESOLVE">Resolve</option>"#));
    assert!(html.contains(r#"value="UP_STATUS">Update Status</option>"#));
    assert!(html.contains(r#"value="UP_PRIOR">Update Priority</option>"#));
    assert!(html.contains(r#"value="UP_SEV">Update Severity</option>"#));
    assert!(html.contains(r#"value="UP_CATEGORY">Update Category</option>"#));
    assert!(html.contains(r#"value="UP_TARGET_VERSION">Update Target Version</option>"#));
    assert!(html.contains(r#"value="UP_DUE_DATE">Update Due Date</option>"#));
    assert!(html.contains(r#"value="VIEW_STATUS">Update View Status</option>"#));
    assert!(html.contains(r#"value="SET_STICKY">Set Sticky</option>"#));
    assert!(html.contains(r#"value="ADD_NOTE">Add Note</option>"#));
    assert!(html.contains(r#"value="ATTACH_TAGS">Attach Tags</option>"#));

    // Critical check: Ensure NO nested forms exist!
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_dropzone_and_upload_form_html() {
    let dz = render_dropzone_html("test_suffix", "Drop your files here");
    assert!(dz.contains("dropzone-test_suffix"));
    assert!(dz.contains("file-input-test_suffix"));
    assert!(dz.contains("file-list-test_suffix"));
    assert!(dz.contains("Drop your files here"));

    let upload_form = render_upload_form_html(123);
    assert!(upload_form.contains(r#"action="/bug_file_add/123""#));
    assert!(upload_form.contains(r#"enctype="multipart/form-data""#));
    assert!(upload_form.contains("dropzone-view"));
}

#[test]
fn test_render_monitors_and_monitor_form_html() {
    let empty_monitors = render_monitors_html(&[], 42);
    assert!(empty_monitors.contains("No users monitoring this issue."));

    let user = User {
        id: 7,
        username: "mon_user".to_string(),
        realname: "Monitor User".to_string(),
        email: "mon@example.com".to_string(),
        password: "hash".to_string(),
        access_level: 55,
        enabled: 1,
        protected: 0,
        login_count: 0,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "".to_string(),
        last_visit: 0,
        date_created: 0,
    };
    let monitors_html = render_monitors_html(&[user], 42);
    assert!(monitors_html.contains("mon_user"));
    assert!(monitors_html.contains("Monitor User"));
    assert!(monitors_html.contains(r#"action="/bug_monitor_delete/42""#));
    assert!(monitors_html.contains(r#"value="7""#));

    let form_html = render_monitor_form_html(42);
    assert!(form_html.contains(r#"action="/bug_monitor_add/42""#));
    assert!(form_html.contains(r#"name="username""#));
}

#[test]
fn test_render_file_preview_html_and_reminder_badge() {
    let img_preview = render_file_preview_html(99, "screenshot.png", true, None);
    assert!(img_preview.contains(r#"src="/file/bug/99/screenshot.png?inline=1""#));

    let text_preview =
        render_file_preview_html(100, "build.log", false, Some("Line 1: Build started"));
    assert!(text_preview.contains("<details"));
    assert!(text_preview.contains("Line 1: Build started"));

    let empty_preview = render_file_preview_html(101, "archive.zip", false, None);
    assert!(empty_preview.is_empty());

    assert_eq!(render_reminder_badge(0), "");
    assert!(render_reminder_badge(1).contains("Reminder"));
}

#[test]
fn test_render_my_view_page_two_column_timeline_and_rich_rows() {
    let ctx = dummy_ctx(1, 55);
    let open_bug = Bug {
        id: 1,
        project_id: 1,
        reporter_id: 1,
        handler_id: 1,
        duplicate_id: 0,
        priority: 50, // urgent -> fa-arrow-up
        severity: 70,
        reproducibility: 10,
        status: 50, // assigned
        resolution: 10,
        os: String::new(),
        os_build: String::new(),
        platform: String::new(),
        version: String::new(),
        fixed_in_version: String::new(),
        target_version: String::new(),
        build: String::new(),
        summary: "Urgent login error".to_string(),
        view_state: 10,
        sticky: 1,
        category_id: 2,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000100,
    };
    let resolved_bug = Bug {
        id: 2,
        project_id: 1,
        reporter_id: 1,
        handler_id: 1,
        duplicate_id: 0,
        priority: 20, // low -> fa-chevron-down
        severity: 50,
        reproducibility: 10,
        status: 80, // resolved -> strikethrough
        resolution: 20,
        os: String::new(),
        os_build: String::new(),
        platform: String::new(),
        version: String::new(),
        fixed_in_version: String::new(),
        target_version: String::new(),
        build: String::new(),
        summary: "Fixed CSS layout".to_string(),
        view_state: 50, // private -> fa-lock
        sticky: 0,
        category_id: 2,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000200,
    };

    let mut meta = MyViewMeta::default();
    meta.categories_map.insert(2, "Frontend".to_string());
    meta.projects_map.insert(1, "Demo Project".to_string());
    meta.editable_bug_ids.insert(1);
    meta.users_map.insert(1, "alice".to_string());
    meta.timeline_events = vec![
        TimelineEvent {
            id: 10,
            bug_id: 1,
            bug_summary: "Urgent login error".to_string(),
            bug_status: 50,
            project_id: 1,
            user_id: 1,
            username: "alice".to_string(),
            date_modified: 1700000150,
            field_name: "bugnote_added".to_string(),
            old_value: String::new(),
            new_value: "11".to_string(),
            type_: 1,
        },
        TimelineEvent {
            id: 11,
            bug_id: 2,
            bug_summary: "Fixed CSS layout".to_string(),
            bug_status: 80,
            project_id: 1,
            user_id: 1,
            username: "alice".to_string(),
            date_modified: 1700000200,
            field_name: "status".to_string(),
            old_value: "50".to_string(),
            new_value: "80".to_string(),
            type_: 0,
        },
    ];

    let Html(html) = render_my_view_page(
        &ctx,
        &[open_bug],
        &[],
        std::slice::from_ref(&resolved_bug),
        std::slice::from_ref(&resolved_bug),
        &[],
        &[],
        &meta,
    );

    // 2-column responsive layout + collapsible widget boxes
    assert!(html.contains("lg:grid-cols-12"));
    assert!(html.contains("lg:col-span-7"));
    assert!(html.contains("lg:col-span-5"));
    assert!(html.contains(r#"data-widget-id="my_view_assigned""#));
    assert!(html.contains(r#"data-widget-id="my_view_timeline""#));
    assert!(html.contains(r#"data-action="collapse""#));

    // Rich row details: status square swatch, edit pencil, priority icons, strikethrough ID, category
    assert!(html.contains("fa-square fa-status-box"));
    assert!(html.contains(r#"href="/update_bug_page/1""#));
    assert!(!html.contains(r#"href="/update_bug_page/2""#));
    assert!(html.contains("fa-pencil"));
    assert!(html.contains("fa-arrow-up"));
    assert!(html.contains("fa-chevron-down"));
    assert!(html.contains("fa-lock"));
    assert!(html.contains("fa-thumb-tack"));
    assert!(html.contains("Frontend"));
    assert!(html.contains("[Demo Project]"));
    assert!(html.contains("issue-link-resolved"));

    // Activity Timeline details
    assert!(html.contains("fa-clock-o"));
    assert!(html.contains("Timeline"));
    assert!(html.contains("alice"));
    assert!(html.contains("commented on issue"));
    assert!(html.contains("resolved issue"));
    assert!(html.contains("0000001"));
    assert!(html.contains("0000002"));

    // Status legend bar
    assert!(html.contains("status-legend-bar"));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_view_all_bug_page_ui_m2_collapsible_filter_and_responsive_table() {
    let mut ctx = dummy_ctx(1, 55);
    ctx.active_project_id = 0; // All Projects -> shows Project column
    let mut projects_map = HashMap::new();
    projects_map.insert(1, "Alpha".to_string());
    let mut categories_map = HashMap::new();
    categories_map.insert(3, "Backend".to_string());
    let mut users_map = HashMap::new();
    users_map.insert(2, "bob".to_string());

    let bugs = vec![
        Bug {
            id: 11,
            project_id: 1,
            reporter_id: 1,
            handler_id: 2,
            duplicate_id: 0,
            priority: 60, // immediate
            severity: 70, // crash -> bold red
            reproducibility: 10,
            status: 50,
            resolution: 10,
            os: String::new(),
            os_build: String::new(),
            platform: String::new(),
            version: String::new(),
            fixed_in_version: String::new(),
            target_version: String::new(),
            build: String::new(),
            summary: "Server crash on boot".to_string(),
            view_state: 10,
            sticky: 1,
            category_id: 3,
            date_submitted: 1700000000,
            due_date: 0,
            last_updated: 1700000300,
        },
        Bug {
            id: 12,
            project_id: 1,
            reporter_id: 2,
            handler_id: 0,
            duplicate_id: 0,
            priority: 30, // normal
            severity: 50, // minor
            reproducibility: 10,
            status: 90, // closed -> strikethrough
            resolution: 20,
            os: String::new(),
            os_build: String::new(),
            platform: String::new(),
            version: String::new(),
            fixed_in_version: String::new(),
            target_version: String::new(),
            build: String::new(),
            summary: "Minor typo".to_string(),
            view_state: 10,
            sticky: 0,
            category_id: 3,
            date_submitted: 1700000000,
            due_date: 0,
            last_updated: 1700000400,
        },
    ];

    let mut meta = ViewIssuesMeta::default();
    meta.editable_bug_ids.insert(11);
    meta.note_counts.insert(11, 4);
    meta.file_counts.insert(11, 2);

    let filter = BugFilter {
        search: Some("crash".to_string()),
        severity: Some(70),
        ..Default::default()
    };
    let q = ListBugsQuery {
        q: Some("crash".to_string()),
        severity: Some(70),
        sort: Some("last_updated".to_string()),
        dir: Some("desc".to_string()),
        ..Default::default()
    };

    let Html(html) = render_view_all_bug_page(
        &ctx,
        &bugs,
        2,
        1,
        25,
        &filter,
        &projects_map,
        &categories_map,
        &users_map,
        &HashMap::new(),
        &[],
        &[],
        &[],
        &[],
        None,
        None,
        &q,
        &meta,
    );

    // Collapsible filter box collapsed by default + active indicator + non-JS fallback
    assert!(html.contains(r#"id="filter" class="widget-box widget-color-blue2 collapsed""#));
    assert!(html.contains(r#"data-widget-id="view_issues_filter""#));
    assert!(html.contains(r#"<noscript><style>#filter.collapsed > .widget-body { display: block; }</style></noscript>"#));
    assert!(html.contains("Active (2)"));

    // Responsive table wrapper + rich MantisBT columns
    assert!(html.contains(r#"class="table-responsive""#));
    assert!(html.contains(r#"href="/update_bug_page/11""#));
    assert!(!html.contains(r#"href="/update_bug_page/12""#));
    assert!(html.contains("fa-exclamation-triangle"));
    assert!(html.contains("fa-minus"));
    assert!(html.contains("fa-thumb-tack"));
    assert!(html.contains(r#"href="/view_bug_page/11#bugnotes""#));
    assert!(html.contains(r#"href="/view_bug_page/11#attachments""#));
    assert!(html.contains("font-bold text-red-700"));
    assert!(html.contains("assigned (bob)"));
    assert!(html.contains("Updated <i class=\"fa fa-caret-down"));
    assert!(html.contains("issue-link-resolved"));

    // Bulk actiongroup & zero nested forms
    assert!(html.contains(r#"action="/bug_actiongroup""#));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_bug_report_page_ui_m3_fields_and_permissions() {
    let project_opts = vec![(1, "Demo Project".to_string(), true)];
    let categories = vec![(1, "Backend".to_string())];
    let versions = vec![
        ProjectVersion {
            id: 1,
            project_id: 1,
            version: "v1.0".to_string(),
            description: String::new(),
            released: 1,
            obsolete: 0,
            date_order: 100,
        },
        ProjectVersion {
            id: 2,
            project_id: 1,
            version: "v2.0".to_string(),
            description: String::new(),
            released: 0,
            obsolete: 0,
            date_order: 200,
        },
        ProjectVersion {
            id: 3,
            project_id: 1,
            version: "v0.9-old".to_string(),
            description: String::new(),
            released: 0,
            obsolete: 1,
            date_order: 50,
        },
    ];
    let dev_user = User {
        id: 2,
        username: "dev_alice".to_string(),
        realname: "Alice Dev".to_string(),
        email: "alice@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level: 55,
        login_count: 0,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: String::new(),
        last_visit: 0,
        date_created: 0,
    };

    // Developer sees Target Version (unreleased only), Assign To, Report Stay, OS Version, Product Build
    let ctx_dev = dummy_ctx(2, 55);
    let Html(dev_html) = render_bug_report_page(
        &ctx_dev,
        &project_opts,
        &categories,
        None,
        &versions,
        std::slice::from_ref(&dev_user),
        &[],
        55,
        None,
        None,
        Some(42),
        true,
    );
    assert!(dev_html.contains("widget-box"));
    assert!(dev_html.contains("fa-edit"));
    assert!(dev_html.contains("Enter Issue Details"));
    assert!(dev_html.contains("table-responsive"));
    assert!(dev_html.contains("form-table-left"));
    assert!(dev_html.contains(r#"class="required">*"#));
    assert!(dev_html.contains(r#"name="target_version""#));
    assert!(dev_html.contains(r#"<option value="v2.0">v2.0</option>"#));
    assert!(!dev_html.contains("v0.9-old"));
    // Target Version select must NOT list already-released v1.0
    let tv_start = dev_html.find(r#"name="target_version""#).unwrap();
    let tv_end = dev_html[tv_start..].find("</select>").unwrap() + tv_start;
    assert!(!dev_html[tv_start..tv_end].contains("v1.0"));
    assert!(dev_html.contains(r#"name="handler_id""#));
    assert!(dev_html.contains("dev_alice (Alice Dev)"));
    assert!(dev_html.contains(r#"name="report_stay""#));
    assert!(dev_html.contains(r#"id="report_stay" name="report_stay" value="1" checked"#));
    assert!(dev_html.contains("OS Version"));
    assert!(dev_html.contains("Product Build"));
    assert!(!dev_html.contains("OS Build"));
    assert!(dev_html.contains(r#"type="radio" name="view_state" value="10""#));
    assert!(dev_html.contains(r#"href="/view_bug_page/42""#));
    assert_eq!(dev_html.matches(r#"class="flash-message""#).count(), 1);
    assert_no_nested_forms(&dev_html);

    // Reporter (access 25) does NOT see Assign To, Target Version, or unreleased Product Versions
    let ctx_rep = dummy_ctx(3, 25);
    let Html(rep_html) = render_bug_report_page(
        &ctx_rep,
        &project_opts,
        &categories,
        Some(1),
        &versions,
        &[dev_user],
        &[],
        25,
        None,
        None,
        None,
        false,
    );
    assert!(!rep_html.contains(r#"name="handler_id""#));
    assert!(!rep_html.contains(r#"name="target_version""#));
    assert!(rep_html.contains(r#"<option value="v1.0">v1.0</option>"#));
    assert!(!rep_html.contains(r#"<option value="v2.0">v2.0</option>"#));
    assert!(rep_html.contains(r#"<option value="1" selected>Backend</option>"#));
    assert!(rep_html.contains(r#"data-reload-url="/bug_report_page""#));
    assert!(rep_html.contains(r#"name="report_stay""#));
    assert_no_nested_forms(&rep_html);
}

#[test]
fn test_render_view_bug_page_single_flash_collapsible_and_no_nested_forms() {
    let ctx = dummy_ctx(1, 70);
    let bug = Bug {
        id: 77,
        project_id: 1,
        reporter_id: 1,
        handler_id: 1,
        duplicate_id: 0,
        priority: 50, // urgent
        severity: 60,
        reproducibility: 10,
        status: 50,
        resolution: 10,
        os: "Linux".to_string(),
        os_build: "6.8".to_string(),
        platform: "x86_64".to_string(),
        version: "1.0".to_string(),
        fixed_in_version: "".to_string(),
        build: "b-123".to_string(),
        view_state: 10,
        summary: "UI M3 Detail Test".to_string(),
        sticky: 0,
        target_version: "2.0".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000000,
    };
    let full = BugFull {
        bug,
        description: "**Bold** description referencing #77".to_string(),
        steps_to_reproduce: "1. Step one".to_string(),
        additional_information: "Extra info".to_string(),
    };
    let fmt_ctx = crate::views::formatting::FormatContext {
        enabled: true,
        ..Default::default()
    };

    let Html(html) = render_view_bug_page(
        &ctx,
        &full,
        "Demo",
        "Core",
        "tester",
        "tester",
        70,
        &[],
        r#"<a class="btn btn-primary btn-white btn-round btn-sm" href="/update_bug_page/77">Edit</a>"#,
        "<span>No tags attached.</span>",
        "<p>No relationships.</p>",
        "",
        "<p>No attachments.</p>",
        &render_upload_form_html(77),
        "<p>No notes</p>",
        r#"<form method="POST" action="/bugnote_add/77"><textarea name="note"></textarea></form>"#,
        &render_monitors_html(&[], 77),
        &render_monitor_form_html(77),
        "<p>No history entries.</p>",
        None,
        "",
        Some("Status updated successfully"),
        None,
        &fmt_ctx,
    );

    // Single flash banner only (via render_layout, NO duplicate .flash-success)
    assert!(!html.contains("flash-success"));
    assert_eq!(html.matches("Status updated successfully").count(), 1);

    // Collapsible widget boxes & quick-jump links
    assert!(html.contains(r#"data-widget-id="view_bug_details""#));
    assert!(html.contains(r#"data-widget-id="attachments""#));
    assert!(html.contains(r#"data-widget-id="relationships""#));
    assert!(html.contains(r#"data-widget-id="monitors""#));
    assert!(html.contains(r#"id="bugnotes""#));
    assert!(html.contains(r#"data-widget-id="bugnote_add""#));
    assert!(html.contains(r#"id="history""#));
    assert!(html.contains(r#"data-action="collapse""#));
    assert!(html.contains(r#"href="/bug_reminder_page/77""#));
    assert!(html.contains("Send a Reminder"));
    assert!(html.contains(r##"href="#bugnotes""##));
    assert!(html.contains(r##"href="#history""##));
    assert!(html.contains("formatted-text"));
    assert!(html.contains("fa-arrow-up")); // priority urgent icon
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_update_bug_page_version_dropdowns_and_legacy_fallback() {
    let ctx = dummy_ctx(1, 70);
    let bug = Bug {
        id: 42,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        category_id: 1,
        date_submitted: 1600000000,
        last_updated: 1623715200,
        os: "macOS".to_string(),
        os_build: "14.5".to_string(),
        platform: "arm64".to_string(),
        version: "legacy-0.8".to_string(),
        fixed_in_version: "legacy-fix-0.8".to_string(),
        target_version: "legacy-target-3.0".to_string(),
        build: "build-99".to_string(),
        summary: "Legacy version test".to_string(),
        view_state: 10,
        sticky: 0,
        due_date: 0,
    };
    let full = BugFull {
        bug,
        description: "Desc".to_string(),
        steps_to_reproduce: "Steps".to_string(),
        additional_information: "AddInfo".to_string(),
    };
    let versions = vec![
        ProjectVersion {
            id: 1,
            project_id: 1,
            version: "v1.0".to_string(),
            description: String::new(),
            released: 1,
            obsolete: 0,
            date_order: 100,
        },
        ProjectVersion {
            id: 2,
            project_id: 1,
            version: "v2.0".to_string(),
            description: String::new(),
            released: 0,
            obsolete: 0,
            date_order: 200,
        },
    ];

    let Html(html) = render_update_bug_page(&ctx, &full, "", &[], &[], 70, &versions, &[]);
    assert!(html.contains(r#"<select id="version" name="version">"#));
    assert!(html.contains(r#"<select id="fixed_in_version" name="fixed_in_version">"#));
    assert!(html.contains(r#"<select id="target_version" name="target_version">"#));
    assert!(!html.contains(r#"<input type="text" name="version""#));
    assert!(!html.contains(r#"<input type="text" name="fixed_in_version""#));
    assert!(!html.contains(r#"<input type="text" name="target_version""#));

    // Legacy fallback options preserved & selected
    assert!(html.contains(r#"<option value="legacy-0.8" selected>legacy-0.8</option>"#));
    assert!(html.contains(r#"<option value="legacy-fix-0.8" selected>legacy-fix-0.8</option>"#));
    assert!(
        html.contains(r#"<option value="legacy-target-3.0" selected>legacy-target-3.0</option>"#)
    );

    // Layout & labels
    assert!(html.contains("widget-box"));
    assert!(html.contains("Back To Issue"));
    assert!(html.contains("OS Version"));
    assert!(html.contains("Product Build"));
    assert!(html.contains(r#"<input type="hidden" name="last_updated" value="1623715200">"#));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_bugnotes_cards_anchors_badges_and_no_nested_forms() {
    let mut users_map = HashMap::new();
    users_map.insert(1, "alice".to_string());
    let mut access_map = HashMap::new();
    access_map.insert(1, 55);

    let note = Bugnote {
        id: 105,
        bug_id: 10,
        reporter_id: 1,
        bugnote_text_id: 1,
        view_state: 50,
        date_submitted: 1600000000,
        last_modified: 1600000000,
        note_type: 1,
        note_attr: String::new(),
        time_tracking: 90,
        note: "Note body with `inline`".to_string(),
    };
    let fmt_ctx = crate::views::formatting::FormatContext {
        enabled: true,
        ..Default::default()
    };

    let out = render_bugnotes_cards(&[note], &users_map, &access_map, 55, 1, &fmt_ctx);
    assert!(out.contains(r#"id="c105""#));
    assert!(out.contains("bugnote-card"));
    assert!(out.contains("bugnote-private"));
    assert!(out.contains("fa-user"));
    assert!(out.contains("developer"));
    assert!(out.contains("~0000105"));
    assert!(out.contains(r#"href="/view_bug_page/10#c105""#));
    assert!(out.contains("Reminder"));
    assert!(out.contains("private"));
    assert!(out.contains("01:30"));
    assert!(out.contains("formatted-text"));
    assert!(out.contains("inline-code"));
    assert!(out.contains("btn-white btn-round"));
    assert!(out.contains("Make Public"));
    assert_no_nested_forms(&out);
}

#[test]
fn test_view_all_bug_page_counts_and_renders_version_scope_chips() {
    let ctx = dummy_ctx(1, 55);
    let q_tv = ListBugsQuery {
        target_version: Some("v2.0".to_string()),
        ..Default::default()
    };
    let f_tv = BugFilter {
        target_version: Some("v2.0".to_string()),
        ..Default::default()
    };
    let Html(html_tv) = render_view_all_bug_page(
        &ctx,
        &[],
        0,
        1,
        25,
        &f_tv,
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
        &q_tv,
        &ViewIssuesMeta::default(),
    );
    assert!(html_tv.contains("Active (1)"));
    assert!(html_tv.contains("Version Scope:"));
    assert!(html_tv.contains("Target Version: v2.0"));
    assert!(html_tv.contains("Clear Target Version filter"));
    assert!(
        html_tv.contains(r#"href="/view_all_bug_page?page=1" title="Clear Target Version filter""#)
    );
    assert_no_nested_forms(&html_tv);

    let q_both = ListBugsQuery {
        target_version: Some("v2.0".to_string()),
        fixed_in_version: Some("v1.9".to_string()),
        ..Default::default()
    };
    let f_both = BugFilter {
        target_version: Some("v2.0".to_string()),
        fixed_in_version: Some("v1.9".to_string()),
        ..Default::default()
    };
    let Html(html_both) = render_view_all_bug_page(
        &ctx,
        &[],
        0,
        1,
        25,
        &f_both,
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
        &q_both,
        &ViewIssuesMeta::default(),
    );
    assert!(html_both.contains("Active (2)"));
    assert!(html_both.contains("Target Version: v2.0"));
    assert!(html_both.contains("Fixed in Version: v1.9"));
    assert_no_nested_forms(&html_both);

    // M-1 + M-2: clearing a version chip while a saved filter is active must drop filter_id,
    // use resolved filter state instead of stale URL params, and preserve remaining version scope.
    let q_saved = ListBugsQuery {
        filter_id: Some(5),
        status: Some(10),
        target_version: Some("v3.0".to_string()),
        ..Default::default()
    };
    let f_saved = BugFilter {
        status: Some(50),
        target_version: Some("v3.0".to_string()),
        fixed_in_version: Some("v1.9".to_string()),
        ..Default::default()
    };
    let Html(html_saved) = render_view_all_bug_page(
        &ctx,
        &[],
        0,
        1,
        25,
        &f_saved,
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
        &q_saved,
        &ViewIssuesMeta::default(),
    );
    assert!(html_saved.contains(
        r#"href="/view_all_bug_page?page=1&amp;status=50&amp;fixed_in_version=v1.9" title="Clear Target Version filter""#
    ));
    assert_no_nested_forms(&html_saved);
}

#[test]
fn test_render_view_all_bug_page_advanced_filters_and_custom_columns() {
    let mut ctx = dummy_ctx(1, 70);
    ctx.active_project_id = 1;
    let bug = Bug {
        id: 21,
        project_id: 1,
        reporter_id: 1,
        handler_id: 2,
        duplicate_id: 0,
        priority: 50,
        severity: 60,
        reproducibility: 10,
        status: 50,
        resolution: 20,
        os: "macOS".to_string(),
        os_build: "15.1".to_string(),
        platform: "arm64".to_string(),
        version: "v1.0".to_string(),
        fixed_in_version: "v1.1".to_string(),
        target_version: "v2.0".to_string(),
        build: "b42".to_string(),
        summary: "Custom col test".to_string(),
        view_state: 10,
        sticky: 0,
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000300,
    };
    let cf = CustomField {
        id: 9,
        name: "Browser".to_string(),
        field_type: 3,
        possible_values: "Chrome|Firefox".to_string(),
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
    let mut custom_field_labels = HashMap::new();
    custom_field_labels.insert("custom_field_9".to_string(), "Browser".to_string());
    let mut custom_field_values = HashMap::new();
    custom_field_values.insert((21, 9), "Firefox".to_string());
    let meta = ViewIssuesMeta {
        columns: vec![
            "id".to_string(),
            "resolution".to_string(),
            "target_version".to_string(),
            "custom_field_9".to_string(),
            "summary".to_string(),
        ],
        versions: vec!["v1.0".to_string(), "v2.0".to_string()],
        filter_custom_fields: vec![(cf, vec!["Chrome".to_string(), "Firefox".to_string()])],
        custom_field_labels,
        custom_field_values,
        ..Default::default()
    };

    let mut filter = BugFilter {
        match_type: FILTER_MATCH_ANY,
        resolutions: vec![20],
        date_submitted_from: Some("2026-01-01".to_string()),
        ..Default::default()
    };
    filter.custom_fields.insert(9, vec!["Firefox".to_string()]);
    let q = filter.to_list_query();

    let Html(html) = render_view_all_bug_page(
        &ctx,
        &[bug],
        1,
        1,
        25,
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
        &meta,
    );

    assert!(html.contains(r#"href="/account_manage_columns_page""#));
    assert!(html.contains(r#"href="/manage_filter_page""#));
    assert!(html.contains("Print Reports"));
    assert!(html.contains("Excel Export"));
    assert!(html.contains("Word Export"));
    assert!(html.contains(r#"/issues_rss"#));
    assert!(html.contains("Firefox"));
    assert!(html.contains("Browser"));
    assert!(html.contains("v2.0"));
    assert!(html.contains("name=\"match_type\""));
    assert!(html.contains("name=\"date_submitted_from\""));
    assert!(html.contains("name=\"relationship_type\""));
    assert!(html.contains("name=\"custom_field_9\""));
    assert_no_nested_forms(&html);
}

#[test]
fn test_render_excel_xml_escapes_and_sanitizes_formulas() {
    let headers = vec![
        "Id".to_string(),
        "Notes".to_string(),
        "Summary".to_string(),
        "Custom <Field>".to_string(),
    ];
    let col_keys = vec![
        "id".to_string(),
        "bugnotes_count".to_string(),
        "summary".to_string(),
        "custom_field_1".to_string(),
    ];
    let rows = vec![vec![
        "0000012".to_string(),
        "3".to_string(),
        "=CMD|'calc'!A0 & <script>alert(1)</script>".to_string(),
        "\t@SUM(1,2)".to_string(),
    ]];

    let xml = render_excel_xml("Proj: Alpha/1?", &headers, &col_keys, &rows, None);
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<Worksheet ss:Name=\"Proj  Alpha 1\">"));
    assert!(!xml.contains("<Worksheet ss:Name=\"Notice\">"));
    assert!(xml.contains("<Cell><Data ss:Type=\"Number\">3</Data></Cell>"));
    assert!(xml.contains(
        "<Cell><Data ss:Type=\"String\">&apos;=CMD|&apos;calc&apos;!A0 &amp; &lt;script&gt;alert(1)&lt;/script&gt;</Data></Cell>"
    ));
    assert!(xml.contains("<Cell><Data ss:Type=\"String\">&apos;\t@SUM(1,2)</Data></Cell>"));
    assert!(!xml.contains("<script>"));

    let trunc_xml = render_excel_xml("Proj", &headers, &col_keys, &rows, Some(5));
    assert!(trunc_xml.contains("<Worksheet ss:Name=\"Notice\">"));
    assert!(trunc_xml.contains("Export capped at 1 of 5 matching issues (MANTIS_EXPORT_MAX_ROWS)."));
}

#[test]
fn test_render_print_and_word_exports_escape_html_and_no_nested_forms() {
    let ctx = dummy_ctx(1, 90);
    let bug = Bug {
        id: 15,
        project_id: 1,
        reporter_id: 1,
        handler_id: 1,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 50,
        resolution: 10,
        os: "Linux".to_string(),
        os_build: "6.8".to_string(),
        platform: "x86_64".to_string(),
        version: "1.0".to_string(),
        fixed_in_version: "".to_string(),
        target_version: "1.1".to_string(),
        build: "b1".to_string(),
        summary: "=evil <img src=x onerror=alert(1)>".to_string(),
        view_state: 10,
        sticky: 0,
        category_id: 1,
        date_submitted: 1_700_000_000,
        due_date: 0,
        last_updated: 1_700_000_000,
    };
    let headers = vec![
        "Id".to_string(),
        "Status".to_string(),
        "Summary".to_string(),
    ];
    let col_keys = vec![
        "id".to_string(),
        "status".to_string(),
        "summary".to_string(),
    ];
    let rows = vec![vec![
        "0000015".to_string(),
        "assigned".to_string(),
        bug.summary.clone(),
    ]];

    let print_html = render_print_all_bug_page(
        &ctx,
        "Print<Proj>",
        &ListBugsQuery::default(),
        &headers,
        &col_keys,
        std::slice::from_ref(&bug),
        &rows,
        None,
    )
    .0;
    assert!(print_html.contains("data-action=\"print\""));
    assert!(print_html.contains("/excel_all_bug_page"));
    assert!(print_html.contains("/word_all_bug_page"));
    assert!(print_html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!print_html.contains("<img src=x"));
    assert_no_nested_forms(&print_html);

    let word_doc = render_word_export_doc(
        "Print<Proj>",
        &[WordIssueEntry {
            bug,
            project_name: "Print<Proj>".to_string(),
            category_name: "Core".to_string(),
            reporter_name: "rep".to_string(),
            handler_name: "dev".to_string(),
            description: "Desc <b>bold</b>\nLine2".to_string(),
            steps_to_reproduce: "Step 1".to_string(),
            additional_information: "".to_string(),
            tags: vec!["rss<tag>".to_string()],
            custom_fields: vec![("Env<Prod>".to_string(), "Staging & QA".to_string())],
            notes: vec![WordBugnoteEntry {
                id: 9,
                reporter_name: "dev".to_string(),
                date_submitted: 1_700_000_000,
                view_state: 50,
                note: "Internal note <script>".to_string(),
            }],
        }],
        Some(5),
    );
    assert!(word_doc.contains("xmlns:w=\"urn:schemas-microsoft-com:office:word\""));
    assert!(word_doc.contains("Word export truncated to 1 of 5"));
    assert!(word_doc.contains("Env&lt;Prod&gt;"));
    assert!(word_doc.contains("Staging &amp; QA"));
    assert!(word_doc.contains("Internal note &lt;script&gt;"));
    assert!(word_doc.contains("[private]"));
    assert!(!word_doc.contains("<script>"));
}

#[test]
fn test_render_my_view_config_and_custom_9box_order() {
    let ctx = dummy_ctx(1, 55);
    let cfg = MyViewConfig {
        show_timeline: false,
        boxes: vec![
            crate::models::MyViewBoxConfigEntry {
                key: "my_comments".to_string(),
                enabled: true,
                order: 1,
            },
            crate::models::MyViewBoxConfigEntry {
                key: "verify".to_string(),
                enabled: true,
                order: 2,
            },
        ],
    }
    .normalize();

    let Html(cfg_html) = render_my_view_config_page(&ctx, &cfg, None, None);
    assert!(cfg_html.contains("Configure My View Boxes"));
    assert!(cfg_html.contains("Awaiting Feedback"));
    assert!(cfg_html.contains("Waiting for Verification"));
    assert!(cfg_html.contains("Issues I Have Commented On"));
    assert_no_nested_forms(&cfg_html);

    let boxes = vec![
        (MyViewBox::MyComments, Vec::new()),
        (MyViewBox::Verify, Vec::new()),
    ];
    let Html(mv_html) =
        render_my_view_page_configured(&ctx, &boxes, &MyViewMeta::default(), false, None, None);
    let idx_comm = mv_html.find("Issues I Have Commented On").unwrap();
    let idx_ver = mv_html.find("Waiting for Verification").unwrap();
    assert!(idx_comm < idx_ver);
    assert!(!mv_html.contains("Timeline"));
    assert!(!mv_html.contains(r#"id="timeline""#));
    assert_no_nested_forms(&mv_html);
}

#[test]
fn test_saved_filter_active_count_matches_actual_criteria() {
    let ctx = dummy_ctx(1, 55);
    let filter = BugFilter {
        priority: Some(40),
        ..Default::default()
    };
    let q = ListBugsQuery {
        filter_id: Some(1),
        ..Default::default()
    };
    let Html(html) = render_view_all_bug_page(
        &ctx,
        &[],
        0,
        1,
        25,
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
        &ViewIssuesMeta::default(),
    );
    assert!(html.contains("Active (1)"));
    assert!(!html.contains("Active (2)"));
}
