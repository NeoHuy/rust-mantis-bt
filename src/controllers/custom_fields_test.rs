use super::*;

#[test]
fn test_custom_field_form_deserialize() {
    let form: CustomFieldForm = serde_json::from_str(
        r#"{
            "name": "OS Version",
            "field_type": 0,
            "possible_values": "v1|v2",
            "default_value": "v1",
            "require_report": "1",
            "display_update": "1"
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "OS Version");
    assert_eq!(form.field_type, Some(0));
    assert_eq!(form.possible_values.as_deref(), Some("v1|v2"));
    assert_eq!(form.default_value.as_deref(), Some("v1"));
    assert_eq!(form.require_report.as_deref(), Some("1"));
    assert_eq!(form.display_update.as_deref(), Some("1"));
}

#[test]
fn test_link_project_form_deserialize() {
    let form: LinkProjectForm =
        serde_json::from_str(r#"{"project_id": 10, "sequence": 2}"#).unwrap();
    assert_eq!(form.project_id, 10);
    assert_eq!(form.sequence, Some(2));
}
