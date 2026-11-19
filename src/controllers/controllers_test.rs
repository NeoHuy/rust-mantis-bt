use super::*;

#[test]
fn test_parse_bool() {
    assert!(parse_bool(Some("1")));
    assert!(parse_bool(Some("true")));
    assert!(parse_bool(Some("TRUE")));
    assert!(parse_bool(Some("  True  ")));
    assert!(parse_bool(Some("yes")));
    assert!(parse_bool(Some("YES")));
    assert!(parse_bool(Some("on")));
    assert!(parse_bool(Some("ON")));

    assert!(!parse_bool(Some("0")));
    assert!(!parse_bool(Some("false")));
    assert!(!parse_bool(Some("FALSE")));
    assert!(!parse_bool(Some("no")));
    assert!(!parse_bool(Some("off")));
    assert!(!parse_bool(Some("")));
    assert!(!parse_bool(Some("unknown")));
    assert!(!parse_bool(None));
}
