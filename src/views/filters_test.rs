use super::*;
use crate::models::User;

fn dummy_ctx(user_id: i64, access_level: i32) -> AuthContext {
    AuthContext {
        user: User {
            id: user_id,
            username: "tester".to_string(),
            realname: "Tester".to_string(),
            email: "t@example.com".to_string(),
            password: String::new(),
            enabled: 1,
            protected: 0,
            access_level,
            login_count: 0,
            lost_password_request_count: 0,
            failed_login_count: 0,
            cookie_string: "c".to_string(),
            last_visit: 0,
            date_created: 0,
        },
        active_project_id: 0,
        visible_projects: Vec::new(),
    }
}

#[test]
fn test_render_manage_filter_page_rbac_and_permalink() {
    let sf_own = SavedFilter {
        id: 11,
        user_id: 1,
        project_id: 0,
        is_public: 0,
        name: "My Filter".to_string(),
        filter_string: "{}".to_string(),
    };
    let sf_other = SavedFilter {
        id: 12,
        user_id: 2,
        project_id: 0,
        is_public: 1,
        name: "Team Filter".to_string(),
        filter_string: "{}".to_string(),
    };
    let mut users_map = HashMap::new();
    users_map.insert(1, "rep1".to_string());
    users_map.insert(2, "mgr".to_string());

    let mut rep_manageable = HashSet::new();
    rep_manageable.insert(11);
    let ctx_rep = dummy_ctx(1, 25);
    let Html(html) = render_manage_filter_page(
        &ctx_rep,
        &[sf_own.clone(), sf_other.clone()],
        &rep_manageable,
        &users_map,
        &HashMap::new(),
        None,
        None,
    );
    assert!(html.contains("/view_all_bug_page?filter_id=11"));
    assert!(html.contains("/manage_filter_edit_page/11"));
    assert!(!html.contains("/manage_filter_edit_page/12"));
    assert!(html.contains("data-copy-target=\"filter-permalink-11\""));

    let mut mgr_manageable = HashSet::new();
    mgr_manageable.insert(12);
    let ctx_mgr = dummy_ctx(3, 70);
    let Html(html_mgr) = render_manage_filter_page(
        &ctx_mgr,
        &[sf_own, sf_other],
        &mgr_manageable,
        &users_map,
        &HashMap::new(),
        None,
        None,
    );
    assert!(!html_mgr.contains("/manage_filter_edit_page/11"));
    assert!(html_mgr.contains("/manage_filter_edit_page/12"));
}

#[test]
fn test_render_manage_filter_edit_page_visibility_control() {
    let sf = SavedFilter {
        id: 5,
        user_id: 1,
        project_id: 0,
        is_public: 0,
        name: "Critical Open".to_string(),
        filter_string: "{}".to_string(),
    };
    let bf = BugFilter {
        match_type: FILTER_MATCH_ANY,
        statuses: vec![10, 50],
        ..Default::default()
    };

    let ctx_rep = dummy_ctx(1, 25);
    let Html(html_rep) = render_manage_filter_edit_page(&ctx_rep, &sf, &bf, &[], false, None, None);
    assert!(!html_rep.contains(r#"name="is_public""#));
    assert!(html_rep.contains("Match: ANY (OR)"));
    assert!(html_rep.contains("Status: new, assigned"));

    let ctx_mgr = dummy_ctx(1, 70);
    let Html(html_mgr) = render_manage_filter_edit_page(&ctx_mgr, &sf, &bf, &[], true, None, None);
    assert!(html_mgr.contains(r#"name="is_public""#));
}
