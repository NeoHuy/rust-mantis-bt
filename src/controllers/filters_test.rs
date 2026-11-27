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
    let form: UpdateFilterForm = serde_json::from_str(
        r#"{"name":"Renamed Filter","project_id":2,"is_public":"1","match_type":1}"#,
    )
    .unwrap();
    assert_eq!(form.name, "Renamed Filter");
    assert_eq!(form.project_id, Some(2));
    assert_eq!(form.is_public.as_deref(), Some("1"));
    assert_eq!(form.match_type, Some(1));
}

#[tokio::test]
async fn test_manage_filter_update_inaccessible_project_denied() {
    use crate::controllers::build_router;
    use crate::models::BugFilter;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let rep_id = db
        .create_user(
            "flt_rep",
            "Flt Rep",
            "fltrep@x.com",
            "pw123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep = db.authenticate("flt_rep", "pw123").unwrap().unwrap();
    let cookie = format!("MANTIS_STRING_COOKIE={}", rep.cookie_string);
    let pub_proj = db.create_project("PubFltProj", 10, 10, "", true).unwrap();
    let priv_proj = db.create_project("PrivFltProj", 10, 50, "", true).unwrap();
    let fid = db
        .save_filter(rep_id, pub_proj, false, "My Filter", &BugFilter::default())
        .unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_filter_update/{}", fid))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("name=Moved&project_id={}", priv_proj)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
