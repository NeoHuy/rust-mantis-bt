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

#[tokio::test]
async fn test_project_only_manager_public_filter_move_and_private_delete_rbac() {
    use crate::controllers::build_router;
    use crate::models::BugFilter;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let proj_a = db.create_project("ProjMgrA", 10, 10, "", true).unwrap();
    let proj_b = db.create_project("ProjMgrB", 10, 10, "", true).unwrap();

    // User is global Reporter (25), Manager (70) ONLY on proj_a, Reporter (25) on proj_b.
    let u_id = db
        .create_user(
            "proj_only_mgr",
            "Proj Mgr",
            "pmgr@x.com",
            "pw123",
            25,
            true,
            false,
        )
        .unwrap();
    db.set_project_user(proj_a, u_id, 70).unwrap();
    let u = db.authenticate("proj_only_mgr", "pw123").unwrap().unwrap();
    let u_cookie = format!("MANTIS_STRING_COOKIE={}", u.cookie_string);

    // 1. Project-only manager CAN save a public filter on proj_a.
    let res_save_pub = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &u_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=ProjAPublic&project_id={}&is_public=1",
                    proj_a
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_save_pub.status(), StatusCode::SEE_OTHER);
    let loc = res_save_pub
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.contains("msg=Filter+saved"));
    let fid: i64 = loc
        .split("filter_id=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    // 2. Moving that public filter to All Projects (0) or unmanaged proj_b is denied (403).
    for dest_pid in [0, proj_b] {
        let res_move = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/manage_filter_update/{}", fid))
                    .header("cookie", &u_cookie)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "name=ProjAPublic&project_id={}",
                        dest_pid
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_move.status(), StatusCode::FORBIDDEN);
        let (sf, _) = db.get_filter(fid).unwrap().unwrap();
        assert_eq!(sf.project_id, proj_a);
    }

    // 3. Another Manager (global 70) cannot delete a user's private filter (403).
    let priv_fid = db
        .save_filter(u_id, proj_a, false, "UserPrivate", &BugFilter::default())
        .unwrap();
    db.create_user(
        "global_mgr",
        "Global Mgr",
        "gmgr@x.com",
        "pw123",
        70,
        true,
        false,
    )
    .unwrap();
    let mgr = db.authenticate("global_mgr", "pw123").unwrap().unwrap();
    let mgr_cookie = format!("MANTIS_STRING_COOKIE={}", mgr.cookie_string);
    let res_mgr_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/filter_delete/{}", priv_fid))
                .header("cookie", &mgr_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_mgr_del.status(), StatusCode::FORBIDDEN);
    assert!(db.get_filter(priv_fid).unwrap().is_some());

    // 4. Oversized saved filter body exceeding 32KB JSON limit is rejected cleanly with ?err=.
    let mut big_body = format!("name=TooBig&project_id={}", proj_a);
    for i in 0..100 {
        big_body.push_str(&format!("&version={}_{}", i, "x".repeat(400)));
    }
    let res_big = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/filter_save")
                .header("cookie", &u_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(big_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_big.status(), StatusCode::SEE_OTHER);
    let big_loc = res_big
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(big_loc.contains("err=Filter+definition+exceeds+maximum+allowed+size"));
}
