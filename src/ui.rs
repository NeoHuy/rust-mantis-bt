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

const CSS: &str = r#"
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; background-color: #f7f9fa; color: #222; font-size: 13px; line-height: 1.5; }
a { color: #1e5799; text-decoration: none; }
a:hover { text-decoration: underline; }

header.top-bar { background-color: #2c5987; color: #fff; padding: 6px 16px; display: flex; align-items: center; justify-content: space-between; border-bottom: 2px solid #1a395b; }
.top-bar .brand { font-size: 16px; font-weight: bold; color: #fff; letter-spacing: 0.5px; }
.top-bar .user-controls { display: flex; align-items: center; gap: 14px; font-size: 12px; }
.top-bar .user-info { color: #dce7f5; }
.top-bar select { padding: 3px 6px; font-size: 12px; border-radius: 3px; border: 1px solid #1a395b; background: #fff; }
.top-bar button { padding: 3px 8px; font-size: 12px; border-radius: 3px; border: 1px solid #1a395b; background: #e8eef3; color: #2c5987; cursor: pointer; font-weight: 600; }
.top-bar button:hover { background: #fff; }

nav.main-nav { background-color: #e8eef3; border-bottom: 1px solid #c8d7e6; padding: 6px 16px; display: flex; gap: 12px; font-weight: 500; font-size: 13px; }
nav.main-nav a { padding: 4px 8px; border-radius: 3px; color: #2c5987; }
nav.main-nav a:hover { background-color: #d0e0ed; text-decoration: none; }

.page-container { max-width: 1200px; margin: 16px auto; padding: 0 16px; }
.flash-message { background-color: #dff0d8; border: 1px solid #d6e9c6; color: #3c763d; padding: 10px 14px; border-radius: 4px; margin-bottom: 16px; font-size: 13px; }
.flash-error { background-color: #f2dede; border: 1px solid #ebccd1; color: #a94442; padding: 10px 14px; border-radius: 4px; margin-bottom: 16px; font-size: 13px; }

.section-title { font-size: 16px; font-weight: bold; color: #2c5987; margin-bottom: 12px; border-bottom: 2px solid #c8d7e6; padding-bottom: 4px; }
.panel { background: #fff; border: 1px solid #c8d7e6; border-radius: 4px; margin-bottom: 20px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }
.panel-header { background: #e8eef3; padding: 8px 12px; font-weight: bold; color: #2c5987; border-bottom: 1px solid #c8d7e6; font-size: 14px; }
.panel-body { padding: 14px; }

table.data-table { width: 100%; border-collapse: collapse; margin-bottom: 16px; font-size: 12px; background: #fff; }
table.data-table th { background: #c8d7e6; color: #2c5987; text-align: left; padding: 6px 10px; border: 1px solid #b0c4de; font-weight: 600; }
table.data-table td { padding: 6px 10px; border: 1px solid #e0e6ed; vertical-align: middle; }
table.data-table tr.row-1 { background-color: #ffffff; }
table.data-table tr.row-2 { background-color: #f7fafc; }
table.data-table tr:hover { background-color: #edf4fa; }

.form-table { width: 100%; border-collapse: collapse; margin-bottom: 14px; }
.form-table td { padding: 6px 10px; vertical-align: middle; }
.form-table td.label-col { width: 220px; font-weight: 600; color: #444; background-color: #f4f7f9; border: 1px solid #e0e6ed; text-align: right; }
.form-table td.input-col { border: 1px solid #e0e6ed; background-color: #fff; }

input[type="text"], input[type="password"], input[type="email"], select, textarea {
    padding: 5px 8px; font-size: 13px; border: 1px solid #ccd5de; border-radius: 3px; width: 100%; max-width: 400px;
}
input[type="text"]:focus, input[type="password"]:focus, input[type="email"]:focus, select:focus, textarea:focus {
    outline: none; border-color: #2c5987; box-shadow: 0 0 3px rgba(44,89,135,0.3);
}
input[type="checkbox"] { margin-right: 6px; vertical-align: middle; }
textarea { min-height: 80px; font-family: inherit; }

.btn { display: inline-block; padding: 5px 12px; font-size: 12px; font-weight: 600; border-radius: 3px; border: 1px solid #2c5987; background: #2c5987; color: #fff; cursor: pointer; text-decoration: none; }
.btn:hover { background: #1a395b; text-decoration: none; color: #fff; }
.btn-secondary { background: #e8eef3; color: #2c5987; border-color: #c8d7e6; }
.btn-secondary:hover { background: #d0e0ed; color: #2c5987; }
.btn-danger { background: #c9302c; border-color: #ac2925; color: #fff; }
.btn-danger:hover { background: #a52824; }
.btn-sm { padding: 2px 6px; font-size: 11px; }

.badge { display: inline-block; padding: 2px 6px; font-size: 11px; font-weight: bold; border-radius: 3px; }
.badge-success { background: #dff0d8; color: #3c763d; border: 1px solid #d6e9c6; }
.badge-warning { background: #fcf8e3; color: #8a6d3b; border: 1px solid #faebcc; }
.badge-danger { background: #f2dede; color: #a94442; border: 1px solid #ebccd1; }

.login-box { max-width: 420px; margin: 60px auto; background: #fff; border: 1px solid #c8d7e6; border-radius: 4px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); overflow: hidden; }
.login-header { background: #2c5987; color: #fff; padding: 14px 20px; font-size: 18px; font-weight: bold; text-align: center; }
.login-body { padding: 20px; }
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
    let all_selected = if ctx.active_project_id == 0 { "selected" } else { "" };
    project_options.push_str(&format!(
        r#"<option value="0" {}>All Projects</option>"#,
        all_selected
    ));

    for (proj, depth) in &ctx.visible_projects {
        let selected = if proj.id == ctx.active_project_id { "selected" } else { "" };
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

    let mut nav_links = vec![
        r#"<a href="/account_page">My Account</a>"#.to_string(),
    ];
    if ctx.user.access_level >= 70 {
        nav_links.push(r#"<a href="/manage_proj_page">Manage Projects</a>"#.to_string());
    }
    if ctx.user.access_level >= 90 {
        nav_links.push(r#"<a href="/manage_user_page">Manage Users</a>"#.to_string());
    }

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
    <title>{} - MantisBT</title>
    <style>{}</style>
</head>
<body>
    <header class="top-bar">
        <div class="brand">MantisBT</div>
        <div class="user-controls">
            <span class="user-info">Logged in as: <strong>{}</strong> [{}]</span>
            <form action="/set_project" method="POST" style="display:inline-flex; align-items:center; gap:4px; margin:0;">
                <label for="top_project_id" style="color:#dce7f5; font-size:12px;">Project:</label>
                <select name="project_id" id="top_project_id" onchange="this.form.submit()">
                    {}
                </select>
                <noscript><button type="submit">Switch</button></noscript>
            </form>
            <form action="/logout" method="POST" style="display:inline; margin:0;">
                <button type="submit">Logout</button>
            </form>
        </div>
    </header>
    <nav class="main-nav">
        {}
    </nav>
    <div class="page-container">
        {}
        {}
    </div>
</body>
</html>"#,
        html_escape(title),
        CSS,
        html_escape(&ctx.user.username),
        html_escape(access_level_name),
        project_options,
        nav_links.join("\n        "),
        flash_html,
        content
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
<body>
    <div class="page-container">
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
