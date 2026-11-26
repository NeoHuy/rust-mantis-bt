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
        project_id: Some(11),
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
        target_version: Some("v2.0 beta".to_string()),
        fixed_in_version: Some("v1.9".to_string()),
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
    assert!(url.contains("project_id=11"));
    assert!(url.contains("target_version=v2.0+beta"));
    assert!(url.contains("fixed_in_version=v1.9"));
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

#[test]
fn test_validate_attachment_filename_default_rules() {
    // Allowed file extensions
    assert!(validate_attachment_filename("document.pdf").is_ok());
    assert!(validate_attachment_filename("photo.png").is_ok());
    assert!(validate_attachment_filename("archive.tar.gz").is_ok());
    assert!(validate_attachment_filename("code.rs").is_ok());
    assert!(validate_attachment_filename("README").is_ok());
    assert!(validate_attachment_filename(".gitignore").is_ok());

    // Disallowed file extensions (default dangerous extensions)
    let dangerous = [
        "image.svg",
        "vector.svgz",
        "index.html",
        "page.htm",
        "app.xhtml",
        "script.js",
        "module.mjs",
        "malware.exe",
        "run.bat",
        "cmd.cmd",
        "script.sh",
        "payload.ps1",
        "backdoor.php",
        "shell.phtml",
        "config.xml",
        "doc.xht",
        "include.shtml",
        "app.hta",
        "script.vbs",
        "plugin.jar",
        "setup.msi",
        "driver.dll",
        "screensaver.scr",
        "command.com",
    ];
    for filename in &dangerous {
        assert!(
            validate_attachment_filename(filename).is_err(),
            "Expected {} to be blocked",
            filename
        );
    }

    // Trailing dot bypass attempt
    assert!(validate_attachment_filename("attack.html.").is_err());
    assert!(validate_attachment_filename("malware.exe...").is_err());

    // Case insensitivity
    assert!(validate_attachment_filename("EVIL.EXE").is_err());
    assert!(validate_attachment_filename("danger.SvG").is_err());
    assert!(validate_attachment_filename("test.HTML").is_err());
    assert!(validate_attachment_filename("photo.PNG").is_ok());

    // Empty filename check
    assert!(validate_attachment_filename("").is_err());
    assert!(validate_attachment_filename("   ").is_err());
}

#[test]
fn test_parse_urlencoded_form_and_decode() {
    let raw = b"summary=Issue+title&desc=Hello%20World%21&flag=&special=%2B%26%3D";
    let parsed = parse_urlencoded_form(raw);
    assert_eq!(
        parsed,
        vec![
            ("summary".to_string(), "Issue title".to_string()),
            ("desc".to_string(), "Hello World!".to_string()),
            ("flag".to_string(), "".to_string()),
            ("special".to_string(), "+&=".to_string()),
        ]
    );

    // Empty and malformed sequences
    assert_eq!(parse_urlencoded_form(b""), Vec::<(String, String)>::new());
    assert_eq!(url_decode("%2"), "%2");
    assert_eq!(url_decode("%ZZ"), "%ZZ");
    assert_eq!(url_decode("pure+text"), "pure text");
}

#[test]
fn test_parse_multipart_data_multi_file_and_fields() {
    let boundary = "---------------------------123456789012345";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={}", boundary)
            .parse()
            .unwrap(),
    );

    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"summary\"\r\n\r\n\
         Bug summary here\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"notes.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         File 1 contents\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         PNG_MOCK_BYTES\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"category_id\"\r\n\r\n\
         1\r\n\
         --{boundary}--\r\n",
        boundary = boundary
    );

    let (fields, files) = parse_multipart_data(&headers, body.as_bytes());
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields,
        vec![
            ("summary".to_string(), "Bug summary here".to_string()),
            ("category_id".to_string(), "1".to_string()),
        ]
    );

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].filename, "notes.txt");
    assert_eq!(files[0].content_type, "text/plain");
    assert_eq!(files[0].data, b"File 1 contents");

    assert_eq!(files[1].filename, "photo.png");
    assert_eq!(files[1].content_type, "image/png");
    assert_eq!(files[1].data, b"PNG_MOCK_BYTES");
}

#[test]
fn test_parse_multipart_data_ignores_empty_files_and_missing_boundary() {
    let boundary = "---------------------------9876543210";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={}", boundary)
            .parse()
            .unwrap(),
    );

    // Empty filename AND empty file data (unselected file input) should be ignored
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"empty.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         --{boundary}--\r\n",
        boundary = boundary
    );

    let (fields, files) = parse_multipart_data(&headers, body.as_bytes());
    assert!(fields.is_empty());
    // empty.txt is a real selected 0-byte file and must not be silently dropped
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "empty.txt");
    assert!(files[0].data.is_empty());

    // Missing header
    let empty_headers = HeaderMap::new();
    let (f, fl) = parse_multipart_data(&empty_headers, b"dummy");
    assert!(f.is_empty());
    assert!(fl.is_empty());
}

#[test]
fn test_parse_multipart_data_preserves_more_than_ten_files_for_controller_validation() {
    let boundary = "---------------------------1122334455";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={}", boundary)
            .parse()
            .unwrap(),
    );

    let mut body = String::new();
    for i in 1..=12 {
        body.push_str(&format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"file{i}.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             Content of file {i}\r\n",
            boundary = boundary,
            i = i
        ));
    }
    body.push_str(&format!("--{boundary}--\r\n", boundary = boundary));

    let (_, files) = parse_multipart_data(&headers, body.as_bytes());
    assert_eq!(
        files.len(),
        12,
        "parse_multipart_data must not truncate files, letting controllers validate limit"
    );
}

#[test]
fn test_build_format_context_visibility_and_toggle() {
    let db = Db::open_in_memory().unwrap();
    let pub_proj = db.create_project("PubFmt", 10, 10, "", true).unwrap();
    let priv_proj = db.create_project("PrivFmt", 50, 10, "", true).unwrap();

    let rep_id = db
        .create_user("fmt_rep", "Rep", "fmtrep@x.com", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("fmt_dev", "Dev", "fmtdev@x.com", "pw", 55, true, false)
        .unwrap();
    let rep = db.get_user_by_id(rep_id).unwrap().unwrap();
    let dev = db.get_user_by_id(dev_id).unwrap().unwrap();

    let pub_bug = db
        .create_bug(
            pub_proj,
            rep_id,
            1,
            "Public bug",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let priv_bug = db
        .create_bug(
            priv_proj,
            dev_id,
            1,
            "Secret bug",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();
    let pub_note = db.add_bugnote(pub_bug, dev_id, "Public note", 10).unwrap();
    let priv_note = db.add_bugnote(pub_bug, dev_id, "Private note", 50).unwrap();

    let corpus_str = format!(
        "See #{} #{} ~{} ~{}",
        pub_bug, priv_bug, pub_note, priv_note
    );
    let users = db.list_users(None, false).unwrap();

    // Reporter sees public bug & public note only (not private bug or private note)
    let rep_ctx = build_format_context(&db, &rep, &[&corpus_str], &users);
    assert!(rep_ctx.enabled);
    assert!(rep_ctx.issue_refs.contains_key(&pub_bug));
    assert!(!rep_ctx.issue_refs.contains_key(&priv_bug));
    assert!(rep_ctx.note_refs.contains_key(&pub_note));
    assert!(!rep_ctx.note_refs.contains_key(&priv_note));
    assert!(rep_ctx.valid_usernames.contains("fmt_dev"));

    // Developer sees all
    let dev_ctx = build_format_context(&db, &dev, &[&corpus_str], &users);
    assert!(dev_ctx.issue_refs.contains_key(&priv_bug));
    assert!(dev_ctx.note_refs.contains_key(&priv_note));

    // When MantisCoreFormatting is disabled, build_format_context skips DB lookups
    db.set_plugin_enabled_admin(crate::models::PLUGIN_CORE_FORMATTING, false)
        .unwrap();
    let off_ctx = build_format_context(&db, &dev, &[&corpus_str], &users);
    assert!(!off_ctx.enabled);
    assert!(off_ctx.issue_refs.is_empty());
    assert!(off_ctx.note_refs.is_empty());
}

async fn test_http_login(
    app: &axum::Router,
    username: &str,
    password: &str,
    proj_id: i64,
) -> String {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}",
                    username, password
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let auth_cookie = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &auth_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}", proj_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::SEE_OTHER);
    let proj_cookie = res2
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    format!("{}; {}", auth_cookie, proj_cookie)
}

#[tokio::test]
async fn test_http_bug_report_target_version_assign_to_and_report_stay() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("UIM3Report", 10, 10, "", true).unwrap();
    db.add_version(proj_id, "v1.0", "Released", true, false)
        .unwrap();
    db.add_version(proj_id, "v2.0", "Planned", false, false)
        .unwrap();

    let rep_id = db
        .create_user("rep_m3", "Rep M3", "rep@m3.com", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("dev_m3", "Dev M3", "dev@m3.com", "pw", 55, true, false)
        .unwrap();
    let cat_id = db.add_category(proj_id, "Backend", dev_id).unwrap();
    let disabled_dev_id = db
        .create_user(
            "disabled_dev_m3",
            "Disabled",
            "dis@m3.com",
            "pw",
            55,
            false,
            false,
        )
        .unwrap();
    let proj2_id = db
        .create_project("UIM3Secondary", 10, 10, "", true)
        .unwrap();
    let cat2_id = db.add_category(proj2_id, "Frontend", dev_id).unwrap();

    let app = crate::build_router(db.clone());
    let dev_cookie = test_http_login(&app, "dev_m3", "pw", proj_id).await;
    let rep_cookie = test_http_login(&app, "rep_m3", "pw", proj_id).await;

    // 1. GET /bug_report_page as Reporter: no Assign To, no Target Version, only released Product Version
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/bug_report_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rep_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!rep_html.contains(r#"name="handler_id""#));
    assert!(!rep_html.contains(r#"name="target_version""#));
    assert!(rep_html.contains(r#"<option value="v1.0">v1.0</option>"#));
    assert!(!rep_html.contains(r#"<option value="v2.0">v2.0</option>"#));
    assert!(rep_html.contains(r#"name="report_stay""#));

    // 2. Reporter attempting to assign handler_id > 0 -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id={}&summary=Unauthorized+Assign&handler_id={}",
                    proj_id, cat_id, dev_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2b. Reporter attempting to set target_version -> 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id={}&summary=Unauthorized+TargetVer&target_version=v2.0",
                    proj_id, cat_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2c. Developer submitting already-released or unknown target_version -> redirected with err
    for bad_ver in ["v1.0", "nonexistent-v99"] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bug_report")
                    .header("cookie", &dev_cookie)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "project_id={}&category_id={}&summary=Bad+TargetVer&target_version={}",
                        proj_id, cat_id, bad_ver
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let err_loc = res.headers().get("location").unwrap().to_str().unwrap();
        assert!(
            err_loc.contains("err="),
            "expected err redirect for {}",
            bad_ver
        );
    }

    // 2d. Developer assigning disabled, non-developer, or nonexistent handler_id -> redirected with err
    for bad_hid in [disabled_dev_id, rep_id, 999999] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bug_report")
                    .header("cookie", &dev_cookie)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "project_id={}&category_id={}&summary=Bad+Handler&handler_id={}",
                        proj_id, cat_id, bad_hid
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let err_loc = res.headers().get("location").unwrap().to_str().unwrap();
        assert!(
            err_loc.contains("err="),
            "expected err redirect for handler_id {}",
            bad_hid
        );
    }

    // 3. Developer submits into non-default secondary project with report_stay=1 -> preserves project_id & category_id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id={}&summary=Secondary+Stay&report_stay=1",
                    proj2_id, cat2_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let sec_loc = res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(sec_loc.contains(&format!("project_id={}", proj2_id)));
    assert!(sec_loc.contains(&format!("category_id={}", cat2_id)));
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&sec_loc)
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let sec_stay_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(sec_stay_html.contains(&format!(
        r#"<option value="{}" selected>UIM3Secondary</option>"#,
        proj2_id
    )));
    assert!(sec_stay_html.contains(&format!(
        r#"<option value="{}" selected>Frontend</option>"#,
        cat2_id
    )));
    assert!(sec_stay_html.contains(r#"id="report_stay" name="report_stay" value="1" checked"#));

    // 3b. Developer submits with target_version=v2.0, handler_id=1 (admin), and report_stay=1
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id={}&summary=Stay+Test+Bug&target_version=v2.0&handler_id=1&report_stay=1",
                    proj_id, cat_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.starts_with("/bug_report_page?"));
    assert!(loc.contains(&format!("project_id={}", proj_id)));
    assert!(loc.contains("created_bug_id="));

    // Follow redirect to /bug_report_page?project_id=...&created_bug_id=...
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&loc)
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stay_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(stay_html.contains(r#"href="/view_bug_page/"#));
    assert_eq!(stay_html.matches(r#"class="flash-message""#).count(), 1);

    // 3b. Forged created_bug_id pointing to private issue not visible to reporter does NOT leak banner
    let priv_bug_id = db
        .create_bug(
            proj_id,
            1,
            cat_id,
            "Secret Issue",
            "Secret",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/bug_report_page?created_bug_id={}", priv_bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let forged_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!forged_html.contains(&format!("/view_bug_page/{}", priv_bug_id)));

    // Verify bug in DB got target_version="v2.0" and handler_id=1
    let bugs = db.list_bugs(proj_id, 0, 50).unwrap();
    let stay_bug = bugs.iter().find(|b| b.summary == "Stay Test Bug").unwrap();
    assert_eq!(stay_bug.target_version, "v2.0");
    assert_eq!(stay_bug.handler_id, 1);
    assert_eq!(stay_bug.status, 50);

    // 4. Default submit without report_stay and handler_id=0 keeps category default assignee & redirects to view_bug_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id={}&summary=Default+Assignee+Bug&handler_id=0",
                    proj_id, cat_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc2 = res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc2.starts_with("/view_bug_page/"));

    let bugs2 = db.list_bugs(proj_id, 0, 50).unwrap();
    let def_bug = bugs2
        .iter()
        .find(|b| b.summary == "Default Assignee Bug")
        .unwrap();
    assert_eq!(def_bug.handler_id, dev_id);
}

#[tokio::test]
async fn test_http_view_and_update_bug_ui_m3_parity() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("UIM3View", 10, 10, "", true).unwrap();
    db.add_version(proj_id, "v1.0", "Released", true, false)
        .unwrap();
    db.add_version(proj_id, "v2.0", "Planned", false, false)
        .unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Detail View Test",
            "See #1 and `code`",
            "",
            "",
            40,
            50,
            10,
            "legacy-0.5",
            "Linux",
            "6.6",
            "x86_64",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        bug_id,
        1,
        BugUpdate {
            target_version: Some("legacy-target".to_string()),
            fixed_in_version: Some("legacy-fixed".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let note_id = db
        .add_bugnote(bug_id, 1, "First timeline note", 10)
        .unwrap();

    let app = crate::build_router(db.clone());
    let admin_cookie = test_http_login(&app, "administrator", "root", proj_id).await;

    // 1. GET /view_bug_page/:id?msg=Issue+updated has exactly ONE flash banner
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}?msg=Issue+updated", bug_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let view_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!view_html.contains("flash-success"));
    assert_eq!(view_html.matches("Issue updated").count(), 1);
    assert!(view_html.contains(&format!(r#"id="c{}""#, note_id)));
    assert!(view_html.contains("First timeline note"));
    assert!(view_html.contains("formatted-text"));
    assert!(view_html.contains("btn-white btn-round"));
    assert!(view_html.contains("Close</button>"));
    assert!(view_html.contains(r#"aria-controls="widget-body-bugnotes""#));
    assert!(view_html.contains(r#"onsubmit="return confirm('Close this issue?');""#));

    // 1b. Reporter / Developer (< 70) must NOT see Close button or Delete Issue button
    let dev2_id = db
        .create_user(
            "dev_view_m3",
            "Dev View",
            "dev_view@example.com",
            "pw",
            55,
            true,
            false,
        )
        .unwrap();
    let _ = dev2_id;
    let dev2_cookie = test_http_login(&app, "dev_view_m3", "pw", proj_id).await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &dev2_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dev_view_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!dev_view_html.contains("Close</button>"));
    assert!(!dev_view_html.contains("Delete Issue</button>"));

    // 2. GET /update_bug_page/:id has <select> version dropdowns with legacy fallbacks selected
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/update_bug_page/{}", bug_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let upd_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(upd_html.contains(r#"<select id="version" name="version">"#));
    assert!(upd_html.contains(r#"<option value="legacy-0.5" selected>legacy-0.5</option>"#));
    assert!(upd_html.contains(r#"<option value="legacy-fixed" selected>legacy-fixed</option>"#));
    assert!(upd_html.contains(r#"<option value="legacy-target" selected>legacy-target</option>"#));
    assert!(upd_html.contains("Back To Issue"));

    // 3. Already-closed bug (status=90) must hide Close button even for Administrator
    db.change_bug_status(bug_id, 1, 90, 20, None).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let closed_view_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!closed_view_html.contains("Close</button>"));
    assert!(closed_view_html.contains("Delete Issue</button>"));

    // 4. Assign To dropdown on view_bug_page excludes reporters but preserves disabled/former current assignee
    let rep_view_id = db
        .create_user(
            "rep_no_assign",
            "Rep NoAssign",
            "rep_no@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let former_dev_id = db
        .create_user(
            "former_dev",
            "Former Dev",
            "former@example.com",
            "pw",
            55,
            false,
            false,
        )
        .unwrap();
    let _ = rep_view_id;
    db.assign_bug(bug_id, 1, former_dev_id).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let assign_view_html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!assign_view_html.contains("rep_no_assign"));
    assert!(assign_view_html.contains(&format!(
        r#"<option value="{}" selected>former_dev</option>"#,
        former_dev_id
    )));
}
