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
