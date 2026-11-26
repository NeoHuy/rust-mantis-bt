use crate::auth::AuthContext;
use crate::models::AccessLevel;
use crate::views::html_escape;
use axum::response::Html;

pub fn render_layout(
    title: &str,
    ctx: &AuthContext,
    flash: Option<&str>,
    content: &str,
) -> Html<String> {
    let access_level_name = AccessLevel::from_i32(ctx.user.access_level)
        .map(|a| a.as_str())
        .unwrap_or("user");

    let active_project_name = if ctx.active_project_id == 0 {
        "All Projects".to_string()
    } else {
        ctx.visible_projects
            .iter()
            .find(|(p, _)| p.id == ctx.active_project_id)
            .map(|(p, _)| p.name.clone())
            .unwrap_or_else(|| "All Projects".to_string())
    };

    let mut project_dropdown_items = String::new();
    let all_active_cls = if ctx.active_project_id == 0 {
        " active"
    } else {
        ""
    };
    project_dropdown_items.push_str(&format!(
        r#"<button type="button" class="project-option{}" data-project-id="0" data-project-name="All Projects">All Projects</button>"#,
        all_active_cls
    ));

    let mut project_options = String::new();
    let all_selected = if ctx.active_project_id == 0 {
        "selected"
    } else {
        ""
    };
    project_options.push_str(&format!(
        r#"<option value="0" {}>All Projects</option>"#,
        all_selected
    ));

    for (proj, depth) in &ctx.visible_projects {
        let is_active = proj.id == ctx.active_project_id;
        let selected = if is_active { "selected" } else { "" };
        let active_cls = if is_active { " active" } else { "" };
        let mut indent_str = String::new();
        for _ in 0..*depth {
            indent_str.push_str("&nbsp;&nbsp;&nbsp;&nbsp;");
        }
        let prefix = if *depth > 0 { "» " } else { "" };
        let escaped_name = html_escape(&proj.name);
        project_dropdown_items.push_str(&format!(
            r#"<button type="button" class="project-option{}" data-project-id="{}" data-project-name="{}">{}{}{}</button>"#,
            active_cls, proj.id, escaped_name, indent_str, prefix, escaped_name
        ));
        project_options.push_str(&format!(
            r#"<option value="{}" {}>{}{}{}</option>"#,
            proj.id, selected, indent_str, prefix, escaped_name
        ));
    }

    let is_manage_page = title.contains("Manage")
        || title.contains("Overview")
        || title.contains("Project")
        || title.contains("User")
        || title.contains("Custom Field")
        || title.contains("Plugin")
        || title.contains("Email Queue")
        || title.contains("Tag");

    let is_account_page = title.contains("My Account") || title.contains("API Tokens");

    // Left Sidebar items (MantisBT 2.28.4 .skin-3 compact)
    let mut sidebar_items: Vec<(&str, &str, &str, bool)> = vec![
        (
            "My View",
            "/my_view_page",
            "fa-dashboard",
            title == "My View",
        ),
        (
            "View Issues",
            "/view_all_bug_page",
            "fa-list-alt",
            title == "View Issues"
                || title.starts_with("Issue #")
                || title.starts_with("Update Issue"),
        ),
    ];
    if ctx.user.access_level >= 25 {
        sidebar_items.push((
            "Report Issue",
            "/bug_report_page",
            "fa-edit",
            title == "Report Issue",
        ));
    }
    sidebar_items.push((
        "Change Log",
        "/changelog_page",
        "fa-retweet",
        title == "Changelog",
    ));
    sidebar_items.push(("Roadmap", "/roadmap_page", "fa-road", title == "Roadmap"));
    sidebar_items.push((
        "Summary",
        "/summary_page",
        "fa-bar-chart-o",
        title == "Summary",
    ));
    if ctx.user.access_level >= 55 {
        sidebar_items.push((
            "Billing",
            "/billing_page",
            "fa-clock-o",
            title == "Time Tracking" || title == "Billing",
        ));
    }
    sidebar_items.push(("News", "/news_page", "fa-newspaper-o", title == "News"));
    let manage_default_href = if ctx.user.access_level >= 90 {
        "/manage_overview_page"
    } else if ctx.user.access_level >= 70 {
        "/manage_proj_page"
    } else {
        "/manage_tags_page"
    };
    sidebar_items.push(("Manage", manage_default_href, "fa-gears", is_manage_page));

    let mut sidebar_html = String::new();
    for (label, href, icon, active) in sidebar_items {
        let li_class = if active { r#" class="active""# } else { "" };
        sidebar_html.push_str(&format!(
            r#"<li{}><a href="{}" title="{}"><i class="menu-icon fa {}" aria-hidden="true"></i><span class="menu-text">{}</span></a></li>"#,
            li_class, href, label, icon, label
        ));
    }

    // Contextual Sub-Navigation Tabs for Manage & Account pages (MantisBT 2.x style)
    let mut sub_tabs_html = String::new();
    if is_manage_page {
        let active_tab = if title.contains("Overview") {
            "overview"
        } else if title.contains("User") {
            "users"
        } else if title.contains("Project") {
            "projects"
        } else if title.contains("Tag") {
            "tags"
        } else if title.contains("Custom Field") {
            "custom_fields"
        } else if title.contains("Plugin") {
            "plugins"
        } else if title.contains("Jira") {
            "jira"
        } else if title.contains("Email Queue") {
            "email_queue"
        } else {
            ""
        };
        sub_tabs_html = crate::views::render_manage_tabs(ctx, active_tab);
    } else if is_account_page {
        let acc_act = if title == "My Account" {
            r#" class="active""#
        } else {
            ""
        };
        let tok_act = if title == "API Tokens" {
            r#" class="active""#
        } else {
            ""
        };
        sub_tabs_html.push_str(&format!(
            r#"<div class="sub-nav-tabs">
                <a href="/account_page"{}>My Account</a>
                <a href="/api_tokens_page"{}>API Tokens</a>
            </div>"#,
            acc_act, tok_act
        ));
    }

    let report_top_btn = if ctx.user.access_level >= 25 {
        r#"<a href="/bug_report_page" class="btn-report-top"><i class="fa fa-edit" aria-hidden="true"></i> <span class="btn-report-text">Report Issue</span></a>"#
    } else {
        ""
    };

    let invite_top_btn = if ctx.user.access_level >= 90 {
        r#"<a href="/manage_user_create_page" class="btn-invite-top"><i class="fa fa-user-plus" aria-hidden="true"></i> Invite Users</a>"#
    } else {
        ""
    };

    let flash_html = match flash {
        Some(msg) => format!(r#"<div class="flash-message">{}</div>"#, html_escape(msg)),
        None => String::new(),
    };

    let csrf_token = crate::auth::csrf_token_for_session(&ctx.user.cookie_string);
    let asset_ver = env!("CARGO_PKG_VERSION");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{csrf_token}">
    <title>{title} - MantisBT</title>
    <link rel="icon" type="image/png" href="/images/mantis_logo.png">
    <link rel="stylesheet" href="/assets/tailwind.css?v={asset_ver}">
    <link rel="stylesheet" href="/assets/font-awesome.min.css?v={asset_ver}">
    <script>try{{if(localStorage.getItem('mantis_sidebar_collapsed')==='1')document.documentElement.classList.add('sidebar-collapsed-init');}}catch(_){{}}</script>
    <script src="/assets/mantis.js?v={asset_ver}" defer></script>
</head>
<body class="skin-3" data-username="{username}">
    <header class="navbar">
        <div class="navbar-left">
            <button id="menu-toggler" type="button" class="navbar-toggle" aria-label="Toggle sidebar" aria-expanded="false">
                <i class="fa fa-bars" aria-hidden="true"></i>
            </button>
            <a href="/my_view_page" class="navbar-brand">
                <img src="/images/mantis_logo.png" alt="MantisBT Logo" class="brand-logo">
                <span>MantisBT</span>
            </a>
        </div>
        <div class="navbar-actions">
            {report_top_btn}
            {invite_top_btn}
            <div class="nav-dropdown" id="dropdown_projects_menu">
                <button type="button" class="nav-dropdown-toggle" data-dropdown-toggle="projects" aria-haspopup="true" aria-controls="projects-dropdown-menu" aria-expanded="false">
                    <span class="active-project-label">{active_project_name}</span>
                    <i class="fa fa-angle-down" aria-hidden="true"></i>
                </button>
                <div id="projects-dropdown-menu" class="nav-dropdown-menu projects-dropdown-menu">
                    <div class="projects-searchbox">
                        <input type="text" data-project-search placeholder="Search projects..." aria-label="Search projects" autocomplete="off">
                    </div>
                    <div class="projects-scroll">
                        {project_dropdown_items}
                    </div>
                </div>
                <noscript>
                    <form action="/set_project" method="POST" style="display:inline-flex; align-items:center; gap:4px; margin:0;">
                        <select name="project_id" id="top_project_id">{project_options}</select>
                        <button type="submit">Switch</button>
                    </form>
                </noscript>
            </div>
            <div class="nav-dropdown" id="dropdown_user_menu">
                <button type="button" class="nav-dropdown-toggle" data-dropdown-toggle="user" aria-haspopup="true" aria-controls="user-dropdown-menu" aria-expanded="false">
                    <i class="fa fa-user" aria-hidden="true"></i>
                    <span class="user-info">{username}</span>
                    <i class="fa fa-angle-down" aria-hidden="true"></i>
                </button>
                <div id="user-dropdown-menu" class="nav-dropdown-menu">
                    <div class="dropdown-user-meta">{username} ({role})</div>
                    <a href="/account_page" class="dropdown-item-link"><i class="fa fa-user" aria-hidden="true"></i> My Account</a>
                    <a href="/account_manage_api_tokens_page" class="dropdown-item-link"><i class="fa fa-key" aria-hidden="true"></i> API Tokens</a>
                    <div class="dropdown-divider"></div>
                    <form action="/logout" method="POST" class="m-0">
                        <button type="submit" class="dropdown-logout-btn"><i class="fa fa-sign-out" aria-hidden="true"></i> Logout</button>
                    </form>
                </div>
                <noscript>
                    <form action="/logout" method="POST" style="display:inline; margin:0;">
                        <button type="submit" class="nav-dropdown-toggle">Logout</button>
                    </form>
                </noscript>
            </div>
        </div>
    </header>
    <div id="sidebar-backdrop" class="sidebar-backdrop"></div>
    <div class="main-container">
        <aside id="sidebar" class="sidebar">
            <div class="sidebar-inner">
                <ul class="nav-list">
                    {sidebar_html}
                </ul>
                <button type="button" id="sidebar-collapse-btn" class="sidebar-collapse" aria-label="Toggle sidebar collapse">
                    <span class="sidebar-collapse-icon">
                        <i class="fa fa-angle-double-left" aria-hidden="true"></i>
                    </span>
                </button>
            </div>
        </aside>
        <div class="main-content">
            <div class="breadcrumbs" id="breadcrumbs">
                <div class="breadcrumb-links">
                    <i class="fa fa-home" aria-hidden="true"></i>
                    <a href="/my_view_page">MantisBT</a>
                    <span>/</span>
                    <span>{title}</span>
                </div>
                <div class="breadcrumbs-right">
                    <div id="nav-recent" class="nav-recent" hidden></div>
                    <form id="nav-search-form" class="nav-search" action="/view_all_bug_page" method="GET">
                        <i class="fa fa-search nav-search-icon" aria-hidden="true"></i>
                        <input type="text" id="jump_bug_id" name="q" placeholder="Issue #" aria-label="Jump to Issue ID" autocomplete="off">
                    </form>
                </div>
            </div>
            <div class="page-container">
                {sub_tabs_html}
                {flash_html}
                {content}
            </div>
            <footer class="mantis-footer">
                <span>Powered by <strong>MantisBT 2.x</strong> (Rust + SQLite Single-Binary Edition)</span>
                <span>User: {username} ({role})</span>
            </footer>
        </div>
    </div>
</body>
</html>"#,
        title = html_escape(title),
        csrf_token = html_escape(&csrf_token),
        report_top_btn = report_top_btn,
        invite_top_btn = invite_top_btn,
        active_project_name = html_escape(&active_project_name),
        project_dropdown_items = project_dropdown_items,
        project_options = project_options,
        username = html_escape(&ctx.user.username),
        role = html_escape(access_level_name),
        sidebar_html = sidebar_html,
        sub_tabs_html = sub_tabs_html,
        flash_html = flash_html,
        content = content
    );

    Html(html)
}

pub fn render_unauthenticated_layout(
    title: &str,
    flash_error: Option<&str>,
    content: &str,
) -> Html<String> {
    let error_html = match flash_error {
        Some(msg) => format!(r#"<div class="flash-error">{}</div>"#, html_escape(msg)),
        None => String::new(),
    };

    let asset_ver = env!("CARGO_PKG_VERSION");
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title} - MantisBT</title>
    <link rel="icon" type="image/png" href="/images/mantis_logo.png">
    <link rel="stylesheet" href="/assets/tailwind.css?v={asset_ver}">
    <link rel="stylesheet" href="/assets/font-awesome.min.css?v={asset_ver}">
</head>
<body class="login-layout light-login">
    <div class="login-container">
        <div class="login-logo-title">
            <img src="/assets/mantis_logo.png" alt="MantisBT Logo" class="login-logo">
        </div>
        {error_html}
        {content}
    </div>
</body>
</html>"#,
        title = html_escape(title),
        error_html = error_html,
        content = content
    );

    Html(html)
}

#[cfg(test)]
#[path = "layout_test.rs"]
mod tests;
