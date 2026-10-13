use crate::auth::AuthContext;
use crate::models::AccessLevel;
use axum::response::Html;

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Convert Unix timestamp (seconds) to `YYYY-MM-DD HH:MM` in UTC using Howard Hinnant's civil calendar algorithm.
pub fn format_timestamp(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let days = ts.div_euclid(86_400);
    let secs_of_day = ts.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, m, d, hour, minute)
}

/// Official MantisBT status color palette
pub fn status_color(status: i32) -> &'static str {
    match status {
        10 => "#fcbdbd", // new
        20 => "#e3b7eb", // feedback
        30 => "#ffcd85", // acknowledged
        40 => "#fff494", // confirmed
        50 => "#c2dfff", // assigned
        80 => "#d2f5b0", // resolved
        90 => "#c9ccc4", // closed
        _ => "#e8eef3",
    }
}

pub fn render_status_badge(status: i32, label: &str) -> String {
    let color = status_color(status);
    format!(
        r#"<span class="status-swatch" style="background-color:{};"></span><span>{}</span>"#,
        color,
        html_escape(label)
    )
}

pub fn render_status_legend() -> &'static str {
    r#"<div class="status-legend-bar">
        <div class="status-legend-item" style="background-color:#fcbdbd;">new</div>
        <div class="status-legend-item" style="background-color:#e3b7eb;">feedback</div>
        <div class="status-legend-item" style="background-color:#ffcd85;">acknowledged</div>
        <div class="status-legend-item" style="background-color:#fff494;">confirmed</div>
        <div class="status-legend-item" style="background-color:#c2dfff;">assigned</div>
        <div class="status-legend-item" style="background-color:#d2f5b0;">resolved</div>
        <div class="status-legend-item" style="background-color:#c9ccc4;">closed</div>
    </div>"#
}

const CSS: &str = r#"
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
    font-family: "Open Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background-color: #e4e6e9;
    color: #393939;
    font-size: 13px;
    line-height: 1.5;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
}
a { color: #337ab7; text-decoration: none; }
a:hover { color: #23527c; text-decoration: underline; }

/* MantisBT 2.x Ace Admin Top Navbar */
header.top-bar {
    background-color: #438eb9;
    color: #fff;
    min-height: 45px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid #347499;
    box-shadow: 0 1px 2px rgba(0,0,0,0.12);
}
.top-bar .brand {
    font-size: 18px;
    font-weight: 600;
    color: #fff;
    letter-spacing: 0.3px;
    display: flex;
    align-items: center;
    gap: 8px;
    text-decoration: none;
}
.top-bar .brand:hover { text-decoration: none; color: #fff; }
.top-bar .user-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
}
.top-bar .nav-pill {
    background-color: #2e6589;
    padding: 5px 10px;
    border-radius: 3px;
    color: #fff;
    display: inline-flex;
    align-items: center;
    gap: 6px;
}
.top-bar .btn-report-top {
    background-color: #629b58;
    border: 1px solid #4f8546;
    color: #fff;
    padding: 4px 10px;
    border-radius: 3px;
    font-weight: 600;
    font-size: 12px;
    text-decoration: none;
}
.top-bar .btn-report-top:hover { background-color: #4f8546; text-decoration: none; color: #fff; }
.top-bar select {
    padding: 3px 8px;
    font-size: 12px;
    border-radius: 2px;
    border: 1px solid #2e6589;
    background: #fff;
    color: #333;
    width: auto;
}
.top-bar button {
    padding: 4px 10px;
    font-size: 12px;
    border-radius: 2px;
    border: 1px solid #b74635;
    background: #d15b47;
    color: #fff;
    cursor: pointer;
    font-weight: 600;
}
.top-bar button:hover { background: #b74635; }

/* MantisBT 2.x Main Container: Left Sidebar + Right Content */
.main-container {
    display: flex;
    flex: 1;
}
aside.sidebar {
    width: 190px;
    min-width: 190px;
    background-color: #f2f2f2;
    border-right: 1px solid #ccc;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
}
ul.nav-list {
    list-style: none;
    margin: 0;
    padding: 0;
}
ul.nav-list li {
    border-bottom: 1px solid #e5e5e5;
    position: relative;
}
ul.nav-list li a {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    color: #585858;
    font-size: 13px;
    font-weight: 400;
    background-color: #f8f8f8;
    text-decoration: none;
    transition: background-color 0.1s ease;
}
ul.nav-list li a:hover {
    background-color: #fff;
    color: #2b7dbc;
    text-decoration: none;
}
ul.nav-list li.active a {
    background-color: #fff;
    color: #2b7dbc;
    font-weight: 700;
}
ul.nav-list li.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background-color: #2b7dbc;
}
ul.nav-list li a .nav-icon {
    width: 18px;
    text-align: center;
    font-size: 14px;
    color: #666;
}
ul.nav-list li.active a .nav-icon,
ul.nav-list li a:hover .nav-icon {
    color: #2b7dbc;
}
.sidebar-collapse {
    background-color: #f3f3f3;
    border-top: 1px solid #e0e0e0;
    text-align: center;
    padding: 6px 0;
    color: #aaa;
    font-size: 12px;
}

.main-content {
    flex: 1;
    background-color: #fff;
    display: flex;
    flex-direction: column;
    min-width: 0;
}

/* Breadcrumbs bar */
.breadcrumbs {
    background-color: #f5f5f5;
    border-bottom: 1px solid #e5e5e5;
    padding: 7px 18px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    color: #555;
}
.breadcrumb-links {
    display: flex;
    align-items: center;
    gap: 6px;
}
.breadcrumb-links a { color: #4c8fbd; font-weight: 600; }
.nav-search {
    display: flex;
    align-items: center;
    gap: 4px;
}
.nav-search input[type="text"] {
    width: 130px;
    padding: 3px 8px;
    font-size: 12px;
    border: 1px solid #6fb3e0;
    border-radius: 3px !important;
}

/* Sub-navigation tabs (Manage / Account tabs) */
.sub-nav-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    border-bottom: 2px solid #4c8fbd;
    margin-bottom: 16px;
    padding-top: 4px;
}
.sub-nav-tabs a {
    padding: 7px 14px;
    background-color: #f0f4f8;
    border: 1px solid #c5d0dc;
    border-bottom: none;
    color: #555;
    font-size: 12px;
    font-weight: 600;
    border-radius: 3px 3px 0 0;
    text-decoration: none;
}
.sub-nav-tabs a:hover {
    background-color: #e2eaf2;
    color: #2b7dbc;
}
.sub-nav-tabs a.active {
    background-color: #4c8fbd;
    border-color: #4c8fbd;
    color: #fff;
}

.page-container {
    padding: 16px 20px 28px 20px;
    flex: 1;
}

.flash-message {
    background-color: #dff0d8;
    border: 1px solid #d6e9c6;
    border-left: 4px solid #3c763d;
    color: #3c763d;
    padding: 10px 14px;
    border-radius: 2px;
    margin-bottom: 16px;
    font-size: 13px;
}
.flash-error {
    background-color: #f2dede;
    border: 1px solid #ebccd1;
    border-left: 4px solid #a94442;
    color: #a94442;
    padding: 10px 14px;
    border-radius: 2px;
    margin-bottom: 16px;
    font-size: 13px;
}

.section-title {
    font-size: 16px;
    font-weight: 600;
    color: #2679b5;
    margin-bottom: 12px;
    border-bottom: 1px dotted #e2e2e2;
    padding-bottom: 6px;
}

/* MantisBT 2.x Widget Box (.panel) */
.panel {
    background: #fff;
    border: 1px solid #307ecc;
    border-radius: 0;
    margin-bottom: 18px;
    box-shadow: none;
}
.panel-header {
    background: #307ecc;
    padding: 7px 12px;
    font-weight: 600;
    color: #fff;
    border-bottom: 1px solid #307ecc;
    font-size: 13px;
    letter-spacing: 0.2px;
}
.panel-header a {
    color: #fff !important;
    background: rgba(255,255,255,0.18);
    padding: 2px 8px;
    border-radius: 2px;
    border: 1px solid rgba(255,255,255,0.35);
    text-decoration: none;
}
.panel-header a:hover {
    background: rgba(255,255,255,0.32);
    text-decoration: none;
}
.panel-body { padding: 12px; }

/* MantisBT 2.x Tables */
table.data-table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 12px;
    font-size: 12px;
    background: #fff;
    border: 1px solid #ddd;
}
table.data-table th {
    background: linear-gradient(to bottom, #f8f8f8 0%, #ececec 100%);
    color: #555;
    text-align: left;
    padding: 7px 10px;
    border: 1px solid #ddd;
    font-weight: 700;
}
table.data-table td {
    padding: 6px 10px;
    border: 1px solid #e4e4e4;
    vertical-align: middle;
}
table.data-table tr.row-1 { background-color: #ffffff; }
table.data-table tr.row-2 { background-color: #f9f9f9; }
table.data-table tr:hover { background-color: #f1f5f9; }

.form-table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 12px;
    border: 1px solid #ddd;
}
.form-table th.category-header {
    background: #edf3f4;
    color: #336199;
    font-weight: 700;
    text-align: left;
    padding: 6px 10px;
    border: 1px solid #dcebf7;
    font-size: 12px;
}
.form-table td {
    padding: 6px 10px;
    vertical-align: middle;
    font-size: 12px;
}
.form-table td.label-col {
    width: 18%;
    font-weight: 700;
    color: #336199;
    background-color: #edf3f4;
    border: 1px solid #dcebf7;
    text-align: right;
}
.form-table td.input-col {
    border: 1px solid #e4e4e4;
    background-color: #fff;
}
.form-table tr.spacer-row td {
    padding: 2px;
    background-color: #f5f5f5;
    border-left: none;
    border-right: none;
}

input[type="text"], input[type="password"], input[type="email"], select, textarea {
    padding: 4px 8px;
    font-size: 12px;
    border: 1px solid #d5d5d5;
    border-radius: 0;
    width: 100%;
    max-width: 420px;
    color: #333;
    background-color: #fff;
}
input[type="text"]:focus, input[type="password"]:focus, input[type="email"]:focus, select:focus, textarea:focus {
    outline: none;
    border-color: #f59942;
    box-shadow: none;
}
input[type="checkbox"] { margin-right: 5px; vertical-align: middle; }
textarea { min-height: 80px; font-family: inherit; max-width: 100%; }

/* MantisBT 2.x Ace Buttons */
.btn {
    display: inline-block;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 2px;
    border: 1px solid #428bca;
    background: #428bca;
    color: #fff !important;
    cursor: pointer;
    text-decoration: none;
}
.btn:hover { background: #1b6aaa; border-color: #1b6aaa; text-decoration: none; color: #fff !important; }
.btn-secondary {
    background: #abbac3;
    color: #fff !important;
    border-color: #abbac3;
}
.btn-secondary:hover { background: #8b9aa3; border-color: #8b9aa3; color: #fff !important; }
.btn-danger {
    background: #d15b47;
    border-color: #d15b47;
    color: #fff !important;
}
.btn-danger:hover { background: #b74635; border-color: #b74635; color: #fff !important; }
.btn-sm { padding: 3px 8px; font-size: 11px; }

.badge {
    display: inline-block;
    padding: 2px 6px;
    font-size: 11px;
    font-weight: 700;
    border-radius: 2px;
}
.badge-success { background: #82af6f; color: #fff; }
.badge-warning { background: #f89406; color: #fff; }
.badge-danger { background: #d15b47; color: #fff; }

/* MantisBT Status Swatches & Legend Bar */
.status-swatch {
    display: inline-block;
    width: 13px;
    height: 13px;
    border: 1px solid rgba(0,0,0,0.2);
    vertical-align: middle;
    margin-right: 6px;
}
.status-legend-bar {
    display: flex;
    flex-wrap: wrap;
    border: 1px solid #ccc;
    background: #fff;
    padding: 4px;
    gap: 4px;
    margin-top: 12px;
}
.status-legend-item {
    flex: 1;
    min-width: 95px;
    text-align: center;
    padding: 4px 8px;
    font-size: 11px;
    font-weight: 600;
    color: #222;
    border: 1px solid rgba(0,0,0,0.12);
}

/* MantisBT 2.x Multi-Column Filter Grid */
.filter-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(175px, 1fr));
    gap: 8px 12px;
    margin-bottom: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid #e5e5e5;
}
.filter-cell {
    display: flex;
    flex-direction: column;
    gap: 3px;
}
.filter-label {
    font-size: 11px;
    font-weight: 700;
    color: #336199;
    background: #edf3f4;
    padding: 3px 6px;
    border: 1px solid #dcebf7;
}

/* Footer */
footer.mantis-footer {
    border-top: 3px double #e5e5e5;
    padding: 10px 20px;
    margin-top: auto;
    display: flex;
    justify-content: space-between;
    color: #777;
    font-size: 11px;
    background: #fff;
}

/* MantisBT 2.x Login Page */
body.login-layout {
    background-color: #1d2024;
    justify-content: center;
    align-items: center;
}
.login-container {
    width: 100%;
    max-width: 380px;
    margin: 50px auto;
}
.login-logo-title {
    text-align: center;
    color: #fff;
    font-size: 26px;
    font-weight: 400;
    margin-bottom: 18px;
    letter-spacing: 0.5px;
}
.login-logo-title span {
    color: #438eb9;
    font-weight: 700;
}
.login-box {
    background: #f7f7f7;
    border: 4px solid #394557;
    padding: 22px 26px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.45);
}
.login-header {
    color: #478fca;
    font-size: 17px;
    font-weight: 600;
    border-bottom: 1px solid #d5e3ef;
    padding-bottom: 10px;
    margin-bottom: 16px;
}
.login-body input[type="text"],
.login-body input[type="password"] {
    max-width: 100%;
    padding: 7px 10px;
    font-size: 13px;
}
"#;

pub fn render_layout(
    title: &str,
    ctx: &AuthContext,
    flash: Option<&str>,
    content: &str,
) -> Html<String> {
    let access_level_name = AccessLevel::from_i32(ctx.user.access_level)
        .map(|a| a.as_str())
        .unwrap_or("user");

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
        let selected = if proj.id == ctx.active_project_id {
            "selected"
        } else {
            ""
        };
        let mut indent_str = String::new();
        for _ in 0..*depth {
            indent_str.push_str("&nbsp;&nbsp;&nbsp;&nbsp;");
        }
        let prefix = if *depth > 0 { "» " } else { "" };
        project_options.push_str(&format!(
            r#"<option value="{}" {}>{}{}{}</option>"#,
            proj.id,
            selected,
            indent_str,
            prefix,
            html_escape(&proj.name)
        ));
    }

    let is_manage_page = title.contains("Manage")
        || title.contains("Project")
        || title.contains("User")
        || title.contains("Custom Field")
        || title.contains("Plugin")
        || title.contains("Email Queue")
        || title.contains("Tag");

    let is_account_page = title.contains("My Account") || title.contains("API Tokens");

    // Left Sidebar items (MantisBT 2.x Ace Admin)
    let mut sidebar_items: Vec<(&str, &str, &str, bool)> = vec![
        ("My View", "/my_view_page", "&#9638;", title == "My View"),
        (
            "View Issues",
            "/view_all_bug_page",
            "&#9776;",
            title == "View Issues" || title.starts_with("Issue #") || title.starts_with("Update Issue"),
        ),
    ];
    if ctx.user.access_level >= 25 {
        sidebar_items.push((
            "Report Issue",
            "/bug_report_page",
            "&#9998;",
            title == "Report Issue",
        ));
    }
    sidebar_items.push((
        "Change Log",
        "/changelog_page",
        "&#8635;",
        title == "Changelog",
    ));
    sidebar_items.push(("Roadmap", "/roadmap_page", "&#9873;", title == "Roadmap"));
    sidebar_items.push(("Summary", "/summary_page", "&#9636;", title == "Summary"));
    sidebar_items.push(("News", "/news_page", "&#9993;", title == "News"));
    if ctx.user.access_level >= 55 {
        sidebar_items.push((
            "Time Tracking",
            "/billing_page",
            "&#9201;",
            title == "Time Tracking",
        ));
    }
    let manage_default_href = if ctx.user.access_level >= 70 {
        "/manage_proj_page"
    } else {
        "/manage_tags_page"
    };
    sidebar_items.push(("Manage", manage_default_href, "&#9881;", is_manage_page));
    sidebar_items.push(("My Account", "/account_page", "&#9786;", is_account_page));

    let mut sidebar_html = String::new();
    for (label, href, icon, active) in sidebar_items {
        let li_class = if active { r#" class="active""# } else { "" };
        sidebar_html.push_str(&format!(
            r#"<li{}><a href="{}"><span class="nav-icon">{}</span><span class="menu-text">{}</span></a></li>"#,
            li_class, href, icon, label
        ));
    }

    // Contextual Sub-Navigation Tabs for Manage & Account pages (MantisBT 2.x style)
    let mut sub_tabs_html = String::new();
    if is_manage_page {
        let mut tabs: Vec<(&str, &str, bool)> = Vec::new();
        if ctx.user.access_level >= 90 {
            tabs.push((
                "Manage Users",
                "/manage_user_page",
                title.contains("User"),
            ));
        }
        if ctx.user.access_level >= 70 {
            tabs.push((
                "Manage Projects",
                "/manage_proj_page",
                title.contains("Project"),
            ));
        }
        tabs.push(("Manage Tags", "/manage_tags_page", title.contains("Tag")));
        if ctx.user.access_level >= 90 {
            tabs.push((
                "Manage Custom Fields",
                "/manage_custom_field_page",
                title.contains("Custom Field"),
            ));
            tabs.push((
                "Manage Plugins",
                "/manage_plugin_page",
                title.contains("Plugin"),
            ));
            tabs.push((
                "Manage Jira",
                "/manage_jira_page",
                title.contains("Jira"),
            ));
            tabs.push((
                "Email Queue",
                "/manage_email_queue_page",
                title.contains("Email Queue"),
            ));
        }
        sub_tabs_html.push_str(r#"<div class="sub-nav-tabs">"#);
        for (lbl, href, act) in tabs {
            let cls = if act { r#" class="active""# } else { "" };
            sub_tabs_html.push_str(&format!(r#"<a href="{}"{}>{}</a>"#, href, cls, lbl));
        }
        sub_tabs_html.push_str("</div>");
    } else if is_account_page {
        let acc_act = if title == "My Account" { r#" class="active""# } else { "" };
        let tok_act = if title == "API Tokens" { r#" class="active""# } else { "" };
        sub_tabs_html.push_str(&format!(
            r#"<div class="sub-nav-tabs">
                <a href="/account_page"{}>My Account</a>
                <a href="/api_tokens_page"{}>API Tokens</a>
            </div>"#,
            acc_act, tok_act
        ));
    }

    let report_top_btn = if ctx.user.access_level >= 25 {
        r#"<a href="/bug_report_page" class="btn-report-top">+ Report Issue</a>"#
    } else {
        ""
    };

    let flash_html = match flash {
        Some(msg) => format!(r#"<div class="flash-message">{}</div>"#, html_escape(msg)),
        None => String::new(),
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title} - MantisBT</title>
    <style>{css}</style>
</head>
<body>
    <header class="top-bar">
        <a href="/my_view_page" class="brand">&#128027; MantisBT</a>
        <div class="user-controls">
            {report_top_btn}
            <form action="/set_project" method="POST" style="display:inline-flex; align-items:center; gap:6px; margin:0;">
                <label for="top_project_id" style="color:#dce7f5; font-size:12px;">Project:</label>
                <select name="project_id" id="top_project_id" onchange="this.form.submit()">
                    {project_options}
                </select>
                <noscript><button type="submit">Switch</button></noscript>
            </form>
            <span class="nav-pill">&#128100; <a href="/account_page" style="color:#fff; text-decoration:none;"><strong>{username}</strong></a> <span style="opacity:0.85;">({role})</span></span>
            <form action="/logout" method="POST" style="display:inline; margin:0;">
                <button type="submit">Logout</button>
            </form>
        </div>
    </header>
    <div class="main-container">
        <aside class="sidebar">
            <ul class="nav-list">
                {sidebar_html}
            </ul>
            <div class="sidebar-collapse">&laquo;</div>
        </aside>
        <div class="main-content">
            <div class="breadcrumbs">
                <div class="breadcrumb-links">
                    <a href="/my_view_page">MantisBT</a>
                    <span>/</span>
                    <span>{title}</span>
                </div>
                <div class="nav-search">
                    <input type="text" id="jump_bug_id" placeholder="Issue #" onkeydown="if(event.key==='Enter'){{var v=parseInt(this.value,10);if(v>0)window.location.href='/view_bug_page/'+v;}}">
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
        css = CSS,
        report_top_btn = report_top_btn,
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

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{} - MantisBT</title>
    <style>{}</style>
</head>
<body class="login-layout">
    <div class="login-container">
        <div class="login-logo-title">&#128027; <span>Mantis</span>BT</div>
        {}
        {}
    </div>
</body>
</html>"#,
        html_escape(title),
        CSS,
        error_html,
        content
    );

    Html(html)
}
