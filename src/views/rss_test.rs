use super::*;

#[test]
fn test_render_issues_rss_valid_xml_and_escaping() {
    let entries = vec![IssueRssEntry {
        id: 42,
        summary: "Crash with <script>alert(1)</script> & \"quotes\"".to_string(),
        description: "Repro: click <button> & send ]]> payload\x00".to_string(),
        category: "Backend & API".to_string(),
        project_name: "Core <App>".to_string(),
        reporter: "alice's".to_string(),
        status_label: "new".to_string(),
        priority_label: "high".to_string(),
        severity_label: "crash".to_string(),
        link: "http://127.0.0.1:3000/view_bug_page/42".to_string(),
        pub_date_ts: 1_700_000_000,
    }];

    let xml = render_issues_rss(
        "MantisBT - Issues (Core <App>)",
        "http://127.0.0.1:3000/view_all_bug_page",
        "http://127.0.0.1:3000/issues_rss?project_id=1",
        &entries,
    );

    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<rss version=\"2.0\""));
    assert!(xml.contains("<title>0000042: Crash with &lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;quotes&quot;</title>"));
    assert!(xml.contains("<dc:creator>alice&apos;s</dc:creator>"));
    assert!(xml.contains("<category>[Core &lt;App&gt;] Backend &amp; API</category>"));
    assert!(xml.contains("Repro: click &lt;button&gt; &amp; send ]]&gt; payload"));
    assert!(!xml.contains("<script>"));
    assert!(!xml.contains('\x00'));
    assert!(xml.contains("<pubDate>Tue, 14 Nov 2023 22:13:20 +0000</pubDate>"));
}

#[test]
fn test_render_news_rss_announcement_and_empty_channel() {
    let empty_xml = render_news_rss(
        "MantisBT - News",
        "http://127.0.0.1:3000/news_page",
        "http://127.0.0.1:3000/news_rss",
        &[],
    );
    assert!(empty_xml.contains("<channel>"));
    assert!(empty_xml.contains("<lastBuildDate>"));
    assert!(!empty_xml.contains("1970"));
    assert!(!empty_xml.contains("<item>"));

    let items = vec![NewsRssEntry {
        id: 7,
        headline: "Release <2.0> & Security".to_string(),
        body: "Notes with <tags> & details".to_string(),
        project_name: "All Projects".to_string(),
        author: "administrator".to_string(),
        link: "http://127.0.0.1:3000/news_page".to_string(),
        pub_date_ts: 1_700_000_000,
        announcement: 1,
    }];
    let xml = render_news_rss(
        "MantisBT - News",
        "http://127.0.0.1:3000/news_page",
        "http://127.0.0.1:3000/news_rss",
        &items,
    );
    assert!(xml.contains("<title>[Announcement] Release &lt;2.0&gt; &amp; Security</title>"));
    assert!(
        xml.contains("<guid isPermaLink=\"true\">http://127.0.0.1:3000/news_page#news-7</guid>")
    );
    assert!(xml.contains("<description>Notes with &lt;tags&gt; &amp; details</description>"));
}
