use super::*;
use crate::models::{Project, User};

fn make_test_user(access_level: i32) -> User {
    User {
        id: 1,
        username: "jdoe".to_string(),
        realname: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        password: "hash".to_string(),
        enabled: 1,
        protected: 0,
        access_level,
        login_count: 5,
        lost_password_request_count: 0,
        failed_login_count: 0,
        cookie_string: "cookiestr".to_string(),
        last_visit: 0,
        date_created: 0,
    }
}

#[test]
fn test_render_unauthenticated_layout() {
    let out = render_unauthenticated_layout("Login", Some("Bad credentials"), "<p>Form</p>");
    let html = out.0;
    assert!(html.contains("<title>Login - MantisBT</title>"));
    assert!(html.contains("Bad credentials"));
    assert!(html.contains(r#"class="flash-error""#));
    assert!(html.contains("<p>Form</p>"));
    assert!(html.contains(r#"class="login-layout light-login""#));
    assert!(html.contains(r#"src="/assets/mantis_logo.png""#));
    assert!(html.contains(r#"href="/assets/tailwind.css?v="#));
    assert!(html.contains(r#"href="/assets/font-awesome.min.css?v="#));
}

#[test]
fn test_render_layout_authenticated_admin_shell() {
    let proj = Project {
        id: 42,
        name: "Alpha Project".to_string(),
        status: 10,
        enabled: 1,
        view_state: 10,
        access_min: 10,
        description: String::new(),
        category_id: 1,
        inherit_global: 1,
    };
    let ctx = AuthContext {
        user: make_test_user(90),
        active_project_id: 42,
        visible_projects: vec![(proj, 0)],
    };

    let out = render_layout(
        "Manage Users",
        &ctx,
        Some("Action successful"),
        "<p>User List</p>",
    );
    let html = out.0;

    assert!(html.contains("<title>Manage Users - MantisBT</title>"));
    assert!(html.contains(r#"<meta name="csrf-token" content=""#));
    assert!(html.contains(r#"href="/assets/tailwind.css?v="#));
    assert!(html.contains(r#"href="/assets/font-awesome.min.css?v="#));
    assert!(html.contains(r#"src="/assets/mantis.js?v="#));
    assert!(html.contains(r#"<body class="skin-3" data-username="jdoe">"#));

    // Navbar controls
    assert!(html.contains(r#"id="menu-toggler""#));
    assert!(html.contains(r#"class="btn-report-top""#));
    assert!(html.contains(r#"href="/manage_user_create_page""#));
    assert!(html.contains("Invite Users"));
    assert!(html.contains(r#"data-project-search"#));
    assert!(html.contains(r#"data-project-id="42""#));
    assert!(html.contains("Alpha Project"));

    // User dropdown menu with POST /logout
    assert!(html.contains(r#"href="/account_page""#));
    assert!(html.contains(r#"href="/account_manage_api_tokens_page""#));
    assert!(html.contains(r#"action="/logout" method="POST""#));

    // Font Awesome sidebar icons & collapse toggle
    assert!(html.contains("fa-dashboard"));
    assert!(html.contains("fa-list-alt"));
    assert!(html.contains("fa-edit"));
    assert!(html.contains("fa-retweet"));
    assert!(html.contains("fa-road"));
    assert!(html.contains("fa-bar-chart-o"));
    assert!(html.contains("fa-clock-o"));
    assert!(html.contains("fa-newspaper-o"));
    assert!(html.contains("fa-gears"));
    assert!(html.contains(r#"id="sidebar-collapse-btn""#));

    // Breadcrumbs, recently visited placeholder & jump input
    assert!(html.contains(r#"id="nav-recent""#));
    assert!(html.contains(r#"id="jump_bug_id""#));

    // Flash & content
    assert!(html.contains("Action successful"));
    assert!(html.contains("<p>User List</p>"));
    assert!(html.contains("jdoe"));
    assert!(html.contains("administrator"));
    assert!(html.contains("Manage Users"));
}

#[test]
fn test_render_layout_reporter_hides_admin_and_billing_items() {
    let ctx = AuthContext {
        user: make_test_user(25), // Reporter
        active_project_id: 0,
        visible_projects: vec![],
    };

    let out = render_layout("My View", &ctx, None, "<p>Dashboard</p>");
    let html = out.0;

    assert!(html.contains("All Projects"));
    assert!(html.contains(r#"class="btn-report-top""#));
    assert!(!html.contains("Invite Users"));
    assert!(!html.contains(r#"href="/billing_page""#));
}
