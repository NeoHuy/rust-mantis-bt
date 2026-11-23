use super::*;

#[test]
fn test_save_filter_form_deserialize() {
    let form: SaveFilterForm = serde_json::from_str(
        r#"{
            "name": "My Open Bugs",
            "is_public": "1",
            "search": "auth",
            "status": 10,
            "severity": 50,
            "category_id": 3
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "My Open Bugs");
    assert_eq!(form.is_public.as_deref(), Some("1"));
    assert_eq!(form.search.as_deref(), Some("auth"));
    assert_eq!(form.status, Some(10));
    assert_eq!(form.severity, Some(50));
    assert_eq!(form.category_id, Some(3));
    assert_eq!(form.sort, None);
    assert_eq!(form.dir, None);
    assert_eq!(form.per_page, None);
}

#[test]
fn test_save_filter_form_with_sort_dir_per_page() {
    let form: SaveFilterForm = serde_json::from_str(
        r#"{
            "name": "High Priority Criticals",
            "search": "security",
            "sort": "priority",
            "dir": "asc",
            "per_page": 50
        }"#,
    )
    .unwrap();
    assert_eq!(form.name, "High Priority Criticals");
    assert_eq!(form.search.as_deref(), Some("security"));
    assert_eq!(form.sort.as_deref(), Some("priority"));
    assert_eq!(form.dir.as_deref(), Some("asc"));
    assert_eq!(form.per_page, Some(50));
}
