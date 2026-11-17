use crate::auth::AuthContext;
use crate::models::AccessLevel;
use crate::views::html_escape;
use axum::response::Html;

pub const MANTIS_LOGO_DATA_URI: &str = "data:image/png;base64,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";

pub const CSS: &str = r#"
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
.top-bar .brand .brand-logo {
    width: 24px;
    height: 24px;
    object-fit: contain;
    vertical-align: middle;
}
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
}
.login-logo-title .login-logo {
    width: 44px;
    height: 44px;
    object-fit: contain;
    vertical-align: middle;
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
            title == "View Issues"
                || title.starts_with("Issue #")
                || title.starts_with("Update Issue"),
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
            tabs.push(("Manage Users", "/manage_user_page", title.contains("User")));
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
            tabs.push(("Manage Jira", "/manage_jira_page", title.contains("Jira")));
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
    <link rel="icon" type="image/png" href="{logo}">
    <style>{css}</style>
</head>
<body>
    <header class="top-bar">
        <a href="/my_view_page" class="brand"><img src="{logo}" alt="MantisBT Logo" class="brand-logo"> MantisBT</a>
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
        logo = MANTIS_LOGO_DATA_URI,
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
    <title>{title} - MantisBT</title>
    <link rel="icon" type="image/png" href="{logo}">
    <style>{css}</style>
</head>
<body class="login-layout">
    <div class="login-container">
        <div class="login-logo-title"><img src="{logo}" alt="MantisBT Logo" class="login-logo"> <span>Mantis</span>BT</div>
        {error_html}
        {content}
    </div>
</body>
</html>"#,
        title = html_escape(title),
        logo = MANTIS_LOGO_DATA_URI,
        css = CSS,
        error_html = error_html,
        content = content
    );

    Html(html)
}

#[cfg(test)]
#[path = "layout_test.rs"]
mod tests;
