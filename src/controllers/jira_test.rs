use super::*;

#[test]
fn test_sanitize_error_message() {
    assert_eq!(sanitize_error_message("Normal error"), "Normal error");
    assert_eq!(
        sanitize_error_message("Line 1\r\nLine 2\tTabbed"),
        "Line 1  Line 2 Tabbed"
    );
    assert_eq!(
        sanitize_error_message("   padded message   "),
        "padded message"
    );

    // String > 120 chars should be truncated to 117 chars + "..."
    let long_str = "a".repeat(150);
    let sanitized = sanitize_error_message(&long_str);
    assert_eq!(sanitized.chars().count(), 120);
    assert!(sanitized.ends_with("..."));
}

#[test]
fn test_url_encode() {
    assert_eq!(url_encode("hello"), "hello");
    assert_eq!(url_encode("hello world"), "hello+world");
    assert_eq!(url_encode("abc-123_test.~"), "abc-123_test.~");
    assert_eq!(
        url_encode("foo/bar?key=val&x=1"),
        "foo%2Fbar%3Fkey%3Dval%26x%3D1"
    );
}

#[test]
fn test_jira_sync_form_deserialization() {
    let json = r#"{"jira_key":"JBC-101","epic_key":"JBC-10","issue_type":"Story"}"#;
    let form: JiraSyncForm = serde_json::from_str(json).unwrap();
    assert_eq!(form.jira_key.as_deref(), Some("JBC-101"));
    assert_eq!(form.epic_key.as_deref(), Some("JBC-10"));
    assert_eq!(form.issue_type.as_deref(), Some("Story"));
}

#[test]
fn test_jira_sync_form_empty_defaults() {
    let json = r#"{}"#;
    let form: JiraSyncForm = serde_json::from_str(json).unwrap();
    assert_eq!(form.jira_key, None);
    assert_eq!(form.epic_key, None);
    assert_eq!(form.issue_type, None);
}
