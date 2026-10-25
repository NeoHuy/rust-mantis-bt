pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
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

#[cfg(test)]
pub mod tests {
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
}
