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
