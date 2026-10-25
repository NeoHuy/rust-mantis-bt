use super::*;

#[test]
fn test_tag_form_deserialize() {
    let form: TagForm = serde_json::from_str(
        r#"{
            "name": "performance",
            "description": "Resource usage"
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "performance");
    assert_eq!(form.description.as_deref(), Some("Resource usage"));
}

#[test]
fn test_flash_query_deserialize() {
    let query: FlashQuery = serde_json::from_str(
        r#"{
            "msg": "Success",
            "err": "Error"
        }"#,
    )
    .unwrap();
    assert_eq!(query.msg.as_deref(), Some("Success"));
    assert_eq!(query.err.as_deref(), Some("Error"));
}
