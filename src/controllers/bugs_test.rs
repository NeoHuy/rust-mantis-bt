use super::*;
use crate::models::url_encode;
use crate::models::*;

#[test]
fn test_parse_multipart_file_valid() {
    let boundary = "---------------------------974767299852498929531610575";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={}", boundary)
            .parse()
            .unwrap(),
    );

    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"test_log.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         Hello, this is a test attachment payload.\r\n\
         --{boundary}--\r\n",
        boundary = boundary
    );

    let parsed = parse_multipart_file(&headers, body.as_bytes());
    assert!(parsed.is_some());
    let (filename, content_type, data) = parsed.unwrap();
    assert_eq!(filename, "test_log.txt");
    assert_eq!(content_type, "text/plain");
    assert_eq!(data, b"Hello, this is a test attachment payload.");
}

#[test]
fn test_parse_multipart_file_missing_header_or_boundary() {
    let headers = HeaderMap::new();
    assert!(parse_multipart_file(&headers, b"dummy data").is_none());

    let mut headers_no_boundary = HeaderMap::new();
    headers_no_boundary.insert(header::CONTENT_TYPE, "multipart/form-data".parse().unwrap());
    assert!(parse_multipart_file(&headers_no_boundary, b"dummy data").is_none());
}

#[test]
fn test_sanitize_download_filename_and_content_type() {
    // CRLF and control characters
    assert_eq!(
        sanitize_download_filename("evil\r\nX-Injected: 1.txt"),
        "evil__X-Injected: 1.txt"
    );
    assert_eq!(sanitize_download_filename("a\0b.log"), "a_b.log");

    // Path traversal (Unix & Windows separators)
    assert_eq!(sanitize_download_filename("../../etc/passwd"), "passwd");
    assert_eq!(
        sanitize_download_filename(r"..\..\secret\report.pdf"),
        "report.pdf"
    );

    // Quotes and semicolons
    assert_eq!(
        sanitize_download_filename("my\"file;name.txt"),
        "my_file_name.txt"
    );

    // Empty or dot-only names fall back safely
    assert_eq!(sanitize_download_filename(""), "attachment.bin");
    assert_eq!(sanitize_download_filename("..."), "attachment.bin");
    assert_eq!(sanitize_download_filename("../../"), "attachment.bin");

    // Content-Type sanitization
    assert_eq!(sanitize_content_type(""), "application/octet-stream");
    assert_eq!(
        sanitize_content_type("text/plain\r\nSet-Cookie: x=1"),
        "application/octet-stream"
    );
    assert_eq!(sanitize_content_type("image/png"), "image/png");
}

#[test]
fn test_clamp_per_page() {
    assert_eq!(clamp_per_page(Some(10)), 10);
    assert_eq!(clamp_per_page(Some(25)), 25);
    assert_eq!(clamp_per_page(Some(50)), 50);
    assert_eq!(clamp_per_page(Some(100)), 100);
    assert_eq!(clamp_per_page(None), 25);
    assert_eq!(clamp_per_page(Some(0)), 25);
    assert_eq!(clamp_per_page(Some(15)), 25);
    assert_eq!(clamp_per_page(Some(999)), 25);
}

#[test]
fn test_url_encode_cases() {
    assert_eq!(url_encode("simple"), "simple");
    assert_eq!(url_encode("hello world"), "hello+world");
    assert_eq!(url_encode("a+b=c&d"), "a%2Bb%3Dc%26d");
    assert_eq!(url_encode("100% done"), "100%25+done");
    assert_eq!(url_encode("path/to?query"), "path%2Fto%3Fquery");
}

#[test]
fn test_build_view_all_bug_page_url_preserves_all_parameters_and_replaces_page() {
    let q = ListBugsQuery {
        page: Some(1),
        filter_id: Some(42),
        q: Some("deadlock test".to_string()),
        search: None,
        status: Some(50),
        hide_status: Some(80),
        priority: Some(40),
        severity: Some(60),
        category_id: Some(7),
        reporter_id: Some(3),
        handler_id: Some(-1),
        tag_id: Some(9),
        monitored_by: Some(2),
        msg: Some("Ignored flash".to_string()),
        err: Some("Ignored error".to_string()),
        sort: Some("priority".to_string()),
        dir: Some("asc".to_string()),
        per_page: Some(50),
    };

    let url = build_view_all_bug_page_url(&q, 3);
    assert!(url.starts_with("/view_all_bug_page?page=3"));
    assert!(url.contains("filter_id=42"));
    assert!(url.contains("q=deadlock+test"));
    assert!(url.contains("status=50"));
    assert!(url.contains("hide_status=80"));
    assert!(url.contains("priority=40"));
    assert!(url.contains("severity=60"));
    assert!(url.contains("category_id=7"));
    assert!(url.contains("reporter_id=3"));
    assert!(url.contains("handler_id=-1"));
    assert!(url.contains("tag_id=9"));
    assert!(url.contains("monitored_by=2"));
    assert!(url.contains("sort=priority"));
    assert!(url.contains("dir=asc"));
    assert!(url.contains("per_page=50"));

    // Flash messages should not be leaked into pagination links
    assert!(!url.contains("Ignored"));
}

#[test]
fn test_build_view_all_bug_page_url_fallback_search_and_clamping() {
    let q = ListBugsQuery {
        page: Some(5),
        search: Some("memory & leak".to_string()),
        ..Default::default()
    };
    let url = build_view_all_bug_page_url(&q, 1);
    assert_eq!(url, "/view_all_bug_page?page=1&search=memory+%26+leak");

    // Negative page clamps to 1
    let url_neg = build_view_all_bug_page_url(&q, -5);
    assert_eq!(url_neg, "/view_all_bug_page?page=1&search=memory+%26+leak");
}

fn dummy_bug(
    id: i64,
    priority: i32,
    severity: i32,
    status: i32,
    last_updated: i64,
    summary: &str,
    sticky: i32,
) -> Bug {
    Bug {
        id,
        project_id: 1,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority,
        severity,
        reproducibility: 10,
        status,
        resolution: 10,
        category_id: 1,
        date_submitted: 1000,
        last_updated,
        os: String::new(),
        os_build: String::new(),
        platform: String::new(),
        version: String::new(),
        fixed_in_version: String::new(),
        target_version: String::new(),
        build: String::new(),
        summary: summary.to_string(),
        view_state: 10,
        sticky,
        due_date: 0,
    }
}

#[test]
fn test_sort_bugs_whitelisted_keys() {
    let mut bugs = vec![
        dummy_bug(1, 20, 60, 10, 1000, "Zebra issue", 0),
        dummy_bug(2, 50, 20, 80, 3000, "Alpha issue", 0),
        dummy_bug(3, 30, 40, 50, 2000, "Beta issue", 0),
    ];

    // 1. Sort by id asc
    sort_bugs(&mut bugs, Some("id"), Some("asc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1, 2, 3]);

    // 2. Sort by id desc
    sort_bugs(&mut bugs, Some("id"), Some("desc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![3, 2, 1]);

    // 3. Sort by priority desc
    sort_bugs(&mut bugs, Some("priority"), Some("desc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![2, 3, 1]);

    // 4. Sort by priority asc
    sort_bugs(&mut bugs, Some("priority"), Some("asc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1, 3, 2]);

    // 5. Sort by severity desc
    sort_bugs(&mut bugs, Some("severity"), Some("desc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1, 3, 2]);

    // 6. Sort by status asc
    sort_bugs(&mut bugs, Some("status"), Some("asc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1, 3, 2]);

    // 7. Sort by summary asc
    sort_bugs(&mut bugs, Some("summary"), Some("asc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![2, 3, 1]);

    // 8. Sort by summary desc
    sort_bugs(&mut bugs, Some("summary"), Some("desc"));
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1, 3, 2]);

    // 9. Default sort (last_updated desc) when sort is None or invalid
    sort_bugs(&mut bugs, None, None);
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![2, 3, 1]);

    sort_bugs(&mut bugs, Some("nonexistent_col"), None);
    assert_eq!(bugs.iter().map(|b| b.id).collect::<Vec<_>>(), vec![2, 3, 1]);
}

#[test]
fn test_sort_bugs_sticky_pinned_first() {
    let mut bugs = vec![
        dummy_bug(1, 60, 80, 10, 1000, "Normal High Priority", 0),
        dummy_bug(2, 20, 20, 10, 5000, "Sticky Low Priority", 1),
        dummy_bug(3, 50, 40, 10, 2000, "Sticky Medium Priority", 1),
        dummy_bug(4, 10, 10, 10, 3000, "Normal Low Priority", 0),
    ];

    // Priority DESC: sticky bugs (2, 3) must be first, ordered by priority (3 has 50, 2 has 20).
    // Then normal bugs (1, 4), ordered by priority (1 has 60, 4 has 10).
    sort_bugs(&mut bugs, Some("priority"), Some("desc"));
    assert_eq!(
        bugs.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![3, 2, 1, 4]
    );

    // Priority ASC: sticky bugs (2, 3) must still be first, ordered by priority asc (2 has 20, 3 has 50).
    // Then normal bugs (4 has 10, 1 has 60).
    sort_bugs(&mut bugs, Some("priority"), Some("asc"));
    assert_eq!(
        bugs.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![2, 3, 4, 1]
    );

    // ID ASC: sticky bugs first (2, 3), then normal (1, 4)
    sort_bugs(&mut bugs, Some("id"), Some("asc"));
    assert_eq!(
        bugs.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![2, 3, 1, 4]
    );

    // ID DESC: sticky bugs first (3, 2), then normal (4, 1)
    sort_bugs(&mut bugs, Some("id"), Some("desc"));
    assert_eq!(
        bugs.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![3, 2, 4, 1]
    );

    // Default last_updated DESC: sticky first (2: 5000, 3: 2000), then normal (4: 3000, 1: 1000)
    sort_bugs(&mut bugs, None, None);
    assert_eq!(
        bugs.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![2, 3, 4, 1]
    );
}

#[test]
fn test_bugs_router_assembly() {
    let db = Db::open_in_memory().unwrap();
    let r = router();
    let _app: axum::Router = r.with_state(db);
}

#[test]
fn test_bug_clone_form_deserialize() {
    let form: BugCloneForm = serde_json::from_str(
        r#"{"target_project_id": 42, "copy_relationships": "1", "copy_files": "on"}"#,
    )
    .unwrap();
    assert_eq!(form.target_project_id, 42);
    assert_eq!(form.copy_relationships.as_deref(), Some("1"));
    assert_eq!(form.copy_files.as_deref(), Some("on"));

    let form_minimal: BugCloneForm = serde_json::from_str(r#"{"target_project_id": 10}"#).unwrap();
    assert_eq!(form_minimal.target_project_id, 10);
    assert!(form_minimal.copy_relationships.is_none());
    assert!(form_minimal.copy_files.is_none());
}

#[test]
fn test_bug_move_form_deserialize() {
    let form: BugMoveForm = serde_json::from_str(r#"{"target_project_id": 99}"#).unwrap();
    assert_eq!(form.target_project_id, 99);
}
