use super::*;

#[test]
fn test_save_filter_urlencoded_parse_and_list_query() {
    let body = "name=My+Open+Bugs&is_public=1&search=auth&status=10&severity=50&category_id=3&project_id=7&target_version=v2.0&fixed_in_version=v1.9";
    let pairs = parse_query_pairs(body);
    let mut name = String::new();
    let mut is_public_raw: Option<String> = None;
    for (k, v) in &pairs {
        match k.as_str() {
            "name" => name = v.clone(),
            "is_public" => is_public_raw = Some(v.clone()),
            _ => {}
        }
    }
    let q = ListBugsQuery::from_urlencoded_pairs(&pairs);
    assert_eq!(name, "My Open Bugs");
    assert_eq!(is_public_raw.as_deref(), Some("1"));
    assert_eq!(q.search.as_deref(), Some("auth"));
    assert_eq!(q.status, Some(10));
    assert_eq!(q.severity, Some(50));
    assert_eq!(q.category_id, Some(3));
    assert_eq!(q.project_id, Some(7));
    assert_eq!(q.effective_target_versions(), vec!["v2.0".to_string()]);
    assert_eq!(q.effective_fixed_in_versions(), vec!["v1.9".to_string()]);
    assert_eq!(q.sort, None);
    assert_eq!(q.dir, None);
    assert_eq!(q.per_page, None);
}

#[test]
fn test_save_filter_query_with_sort_dir_per_page() {
    let pairs = parse_query_pairs(
        "name=High+Priority+Criticals&search=security&sort=priority&dir=asc&per_page=50",
    );
    let q = ListBugsQuery::from_urlencoded_pairs(&pairs);
    assert_eq!(q.search.as_deref(), Some("security"));
    assert_eq!(q.sort.as_deref(), Some("priority"));
    assert_eq!(q.dir.as_deref(), Some("asc"));
    assert_eq!(q.per_page, Some(50));
}

#[test]
fn test_update_filter_form_fields() {
    let form = UpdateFilterForm {
        name: "Renamed Filter".to_string(),
        project_id: Some(2),
        is_public: Some("1".to_string()),
        match_type: Some(1),
    };
    assert_eq!(form.name, "Renamed Filter");
    assert_eq!(form.project_id, Some(2));
    assert_eq!(form.is_public.as_deref(), Some("1"));
    assert_eq!(form.match_type, Some(1));
}
