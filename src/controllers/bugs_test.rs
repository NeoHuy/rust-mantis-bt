use super::*;

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
