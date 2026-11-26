pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
pub mod formatting;
pub mod jira;
pub mod layout;
pub mod planning;
pub mod projects;
pub mod tags;
pub mod users;

use crate::auth::AuthContext;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

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

/// Convert Unix timestamp (seconds) to `YYYY-MM-DD` in UTC. Returns empty string if `ts <= 0`.
pub fn format_date(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let formatted = format_timestamp(ts);
    if let Some(date_part) = formatted.split_whitespace().next() {
        date_part.to_string()
    } else {
        String::new()
    }
}

/// Parse `YYYY-MM-DD` to UTC epoch seconds using Howard Hinnant's days_from_civil algorithm.
/// Returns `0` if empty or invalid.
pub fn parse_date_to_epoch(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return 0;
    }
    let year: i64 = match parts[0].parse() {
        Ok(y) => y,
        Err(_) => return 0,
    };
    let month: u32 = match parts[1].parse() {
        Ok(m) if (1..=12).contains(&m) => m,
        _ => return 0,
    };
    let day: u32 = match parts[2].parse() {
        Ok(d) if (1..=31).contains(&d) => d,
        _ => return 0,
    };

    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = month as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (day as u64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = (era * 146_097 + (doe as i64)) - 719_468;
    days * 86_400
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

pub fn render_error_page(
    ctx: &AuthContext,
    title: &str,
    flash: Option<&str>,
    content: &str,
) -> Html<String> {
    layout::render_layout(title, ctx, flash, content)
}

pub fn render_access_denied_page(ctx: &AuthContext) -> Response {
    (
        StatusCode::FORBIDDEN,
        layout::render_layout(
            "Access Denied",
            ctx,
            Some("Access denied. Administrator privileges required."),
            "<p>Access denied. Administrator privileges required.</p>",
        ),
    )
        .into_response()
}

pub fn render_not_found_page(ctx: &AuthContext, message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        layout::render_layout(
            "Not Found",
            ctx,
            Some(message),
            &format!("<p>{}</p>", html_escape(message)),
        ),
    )
        .into_response()
}

/// Renders the unified MantisBT 2.28.4 `.nav-tabs` navigation bar across all Manage pages.
pub fn render_manage_tabs(ctx: &AuthContext, active_tab: &str) -> String {
    let mut tabs: Vec<(&str, &str, &str, bool)> = Vec::new();
    if ctx.user.access_level >= 90 {
        tabs.push((
            "overview",
            "/manage_overview_page",
            r#"<i class="fa fa-info-circle" aria-hidden="true"></i> Overview"#,
            active_tab == "overview",
        ));
        tabs.push((
            "users",
            "/manage_user_page",
            r#"<i class="fa fa-users" aria-hidden="true"></i> Manage Users"#,
            active_tab == "users",
        ));
    }
    if ctx.user.access_level >= 70 {
        tabs.push((
            "projects",
            "/manage_proj_page",
            r#"<i class="fa fa-folder-open" aria-hidden="true"></i> Manage Projects"#,
            active_tab == "projects",
        ));
        tabs.push((
            "tags",
            "/manage_tags_page",
            r#"<i class="fa fa-tags" aria-hidden="true"></i> Manage Tags"#,
            active_tab == "tags",
        ));
    }
    if ctx.user.access_level >= 90 {
        tabs.push((
            "custom_fields",
            "/manage_custom_field_page",
            r#"<i class="fa fa-sliders" aria-hidden="true"></i> Manage Custom Fields"#,
            active_tab == "custom_fields",
        ));
        tabs.push((
            "plugins",
            "/manage_plugin_page",
            r#"<i class="fa fa-plug" aria-hidden="true"></i> Manage Plugins"#,
            active_tab == "plugins",
        ));
        tabs.push((
            "jira",
            "/manage_jira_page",
            r#"<i class="fa fa-exchange" aria-hidden="true"></i> Manage Jira"#,
            active_tab == "jira",
        ));
        tabs.push((
            "email_queue",
            "/manage_email_queue_page",
            r#"<i class="fa fa-envelope-o" aria-hidden="true"></i> Email Queue"#,
            active_tab == "email_queue",
        ));
    }

    if tabs.is_empty() {
        return String::new();
    }

    let mut html = String::from(r#"<ul class="nav-tabs" role="tablist">"#);
    for (_key, href, label, is_active) in tabs {
        let li_cls = if is_active { r#" class="active""# } else { "" };
        html.push_str(&format!(
            r#"<li{}><a href="{}">{}</a></li>"#,
            li_cls, href, label
        ));
    }
    html.push_str("</ul>");
    html
}

/// Renders the `/manage_overview_page` Site Information dashboard (Admin >= 90 only).
pub fn render_manage_overview_page(
    ctx: &AuthContext,
    stats: &crate::models::SiteOverviewStats,
    runtime_info: &[(&str, &str)],
) -> Html<String> {
    let mut config_rows = String::new();
    for (idx, (k, v)) in runtime_info.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        config_rows.push_str(&format!(
            r#"<tr class="{}"><td class="label-col">{}</td><td class="input-col">{}</td></tr>"#,
            row_cls,
            html_escape(k),
            html_escape(v)
        ));
    }

    let content = format!(
        r#"<div class="section-title"><i class="fa fa-info-circle" aria-hidden="true"></i> Site Information</div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-server" aria-hidden="true"></i> Application &amp; Database Summary</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="form-table" style="margin-bottom:0;">
                            <tr><td class="label-col">rust-mantis-bt Version</td><td class="input-col"><strong>{}</strong></td></tr>
                            <tr><td class="label-col">Database Engine</td><td class="input-col">SQLite (rusqlite bundled)</td></tr>
                            <tr><td class="label-col">Projects</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Users</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Issues</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Bugnotes</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Attachments</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Registered Plugins</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Pending Email Queue</td><td class="input-col">{}</td></tr>
                        </table>
                    </div>
                </div>
            </div>
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-sliders" aria-hidden="true"></i> Runtime Configuration Status</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="form-table" style="margin-bottom:0;">
                            {}
                        </table>
                    </div>
                </div>
            </div>
        </div>"#,
        html_escape(env!("CARGO_PKG_VERSION")),
        stats.projects_count,
        stats.users_count,
        stats.issues_count,
        stats.bugnotes_count,
        stats.attachments_count,
        stats.plugins_count,
        stats.pending_emails_count,
        config_rows
    );

    layout::render_layout("Manage Overview", ctx, None, &content)
}

#[cfg(test)]
#[path = "views_test.rs"]
mod tests;
