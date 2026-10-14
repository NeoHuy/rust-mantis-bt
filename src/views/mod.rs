pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
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
