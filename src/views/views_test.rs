use super::*;

#[test]
fn test_html_escape() {
    assert_eq!(html_escape(""), "");
    assert_eq!(html_escape("hello world"), "hello world");
    assert_eq!(
        html_escape(r#"<script>alert("XSS & 'bad'");</script>"#),
        "&lt;script&gt;alert(&quot;XSS &amp; &#x27;bad&#x27;&quot;);&lt;/script&gt;"
    );
    assert_eq!(
        html_escape("Tiếng Việt có dấu: 1 < 2 & 3 > 0"),
        "Tiếng Việt có dấu: 1 &lt; 2 &amp; 3 &gt; 0"
    );
}

#[test]
fn test_format_timestamp() {
    assert_eq!(format_timestamp(0), "-");
    assert_eq!(format_timestamp(-100), "-");

    // 1970-01-01 00:00:00 UTC is ts=0 -> returns "-" by design in format_timestamp
    // 1970-01-01 01:00:00 UTC is ts=3600
    assert_eq!(format_timestamp(3600), "1970-01-01 01:00");
    // 1970-01-02 00:00:00 UTC is ts=86400
    assert_eq!(format_timestamp(86400), "1970-01-02 00:00");
    // Leap day: 2024-02-29 12:30:00 UTC (1709209800)
    assert_eq!(format_timestamp(1709209800), "2024-02-29 12:30");
    // Modern date: 2026-10-25 11:00:00 UTC (1792926000)
    assert_eq!(format_timestamp(1792926000), "2026-10-25 11:00");
}

#[test]
fn test_format_date_and_parse_epoch() {
    assert_eq!(format_date(0), "");
    assert_eq!(format_date(-50), "");
    assert_eq!(format_date(86400), "1970-01-02");
    assert_eq!(format_date(1709209800), "2024-02-29");

    assert_eq!(parse_date_to_epoch(""), 0);
    assert_eq!(parse_date_to_epoch("invalid"), 0);
    assert_eq!(parse_date_to_epoch("1970-01-01"), 0);
    assert_eq!(parse_date_to_epoch("1970-01-02"), 86400);
    assert_eq!(parse_date_to_epoch("2024-02-29"), 1709164800);
    assert_eq!(format_date(parse_date_to_epoch("2026-11-25")), "2026-11-25");
}

#[test]
fn test_status_color() {
    assert_eq!(status_color(10), "#fcbdbd");
    assert_eq!(status_color(20), "#e3b7eb");
    assert_eq!(status_color(30), "#ffcd85");
    assert_eq!(status_color(40), "#fff494");
    assert_eq!(status_color(50), "#c2dfff");
    assert_eq!(status_color(80), "#d2f5b0");
    assert_eq!(status_color(90), "#c9ccc4");
    assert_eq!(status_color(999), "#e8eef3");
}

#[test]
fn test_render_status_badge() {
    let badge = render_status_badge(10, "new");
    assert!(badge.contains(r#"background-color:#fcbdbd;"#));
    assert!(badge.contains("<span>new</span>"));

    let badge_escaped = render_status_badge(20, "feed<back>");
    assert!(badge_escaped.contains("<span>feed&lt;back&gt;</span>"));
}

#[test]
fn test_render_status_legend() {
    let legend = render_status_legend();
    assert!(legend.contains("status-legend-bar"));
    assert!(legend.contains("new"));
    assert!(legend.contains("feedback"));
    assert!(legend.contains("acknowledged"));
    assert!(legend.contains("confirmed"));
    assert!(legend.contains("assigned"));
    assert!(legend.contains("resolved"));
    assert!(legend.contains("closed"));
}

#[test]
fn test_view_labels() {
    assert_eq!(bugs::status_label(10), "new");
    assert_eq!(bugs::status_label(999), "unknown");

    assert_eq!(bugs::priority_label(30), "normal");
    assert_eq!(bugs::priority_label(999), "unknown");

    assert_eq!(bugs::severity_label(50), "minor");
    assert_eq!(bugs::severity_label(999), "unknown");

    assert_eq!(bugs::reproducibility_label(10), "always");
    assert_eq!(bugs::reproducibility_label(999), "unknown");

    assert_eq!(bugs::resolution_label(20), "fixed");
    assert_eq!(bugs::resolution_label(999), "unknown");

    assert_eq!(bugs::relationship_label(0), "related to");
    assert_eq!(bugs::relationship_label(999), "related to");
}

#[test]
fn test_render_manage_tabs_rbac() {
    let make_ctx = |access_level| AuthContext {
        user: crate::models::User {
            id: 1,
            username: "u".to_string(),
            realname: "U".to_string(),
            email: "u@x.com".to_string(),
            password: "h".to_string(),
            enabled: 1,
            protected: 0,
            access_level,
            login_count: 1,
            lost_password_request_count: 0,
            failed_login_count: 0,
            cookie_string: "c".to_string(),
            last_visit: 0,
            date_created: 0,
        },
        active_project_id: 0,
        visible_projects: vec![],
    };

    let admin_tabs = render_manage_tabs(&make_ctx(90), "overview");
    assert!(admin_tabs.contains(r#"class="nav-tabs""#));
    assert!(admin_tabs.contains("/manage_overview_page"));
    assert!(admin_tabs.contains("/manage_user_page"));
    assert!(admin_tabs.contains("/manage_proj_page"));
    assert!(admin_tabs.contains("/manage_tags_page"));
    assert!(admin_tabs.contains("/manage_custom_field_page"));
    assert!(admin_tabs.contains("/manage_plugin_page"));
    assert!(admin_tabs.contains("/manage_jira_page"));
    assert!(admin_tabs.contains("/manage_email_queue_page"));

    let mgr_tabs = render_manage_tabs(&make_ctx(70), "projects");
    assert!(!mgr_tabs.contains("/manage_overview_page"));
    assert!(!mgr_tabs.contains("/manage_user_page"));
    assert!(mgr_tabs.contains("/manage_proj_page"));
    assert!(mgr_tabs.contains("/manage_tags_page"));

    let dev_tabs = render_manage_tabs(&make_ctx(55), "tags");
    assert!(!dev_tabs.contains("/manage_overview_page"));
    assert!(!dev_tabs.contains("/manage_proj_page"));
    assert!(dev_tabs.contains("/manage_tags_page"));
}
