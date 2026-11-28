use super::*;

#[test]
fn test_render_manage_overview_page_escapes_html_and_shows_error() {
    let ctx = AuthContext {
        user: crate::models::User {
            id: 1,
            username: "admin".to_string(),
            realname: "Admin".to_string(),
            email: "a@x.com".to_string(),
            password: "h".to_string(),
            enabled: 1,
            protected: 0,
            access_level: 90,
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
    let stats = SiteOverviewStats {
        projects_count: 3,
        users_count: 5,
        issues_count: 10,
        bugnotes_count: 4,
        attachments_count: 2,
        plugins_count: 1,
        pending_emails_count: 0,
    };
    let runtime = SiteRuntimeConfig {
        cookie_secure: true,
        upload_fs: true,
        sendgrid_enabled: false,
        jira_enabled: true,
        query_token_enabled: false,
    };
    let html = render_manage_overview_page(&ctx, &stats, &runtime, Some("DB <err>")).0;
    assert!(html.contains("DB &lt;err&gt;"));
    assert!(!html.contains("DB <err>"));
    assert!(html.contains("Filesystem (MANTIS_UPLOAD_DIR)"));
    assert!(html.contains("Disabled (DB Queue Only)"));
    assert!(html.contains("zebra-odd"));
    assert!(html.contains("zebra-even"));
}
