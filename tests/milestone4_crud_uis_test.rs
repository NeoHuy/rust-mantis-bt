use axum::body::Body;
use axum::http::header::SET_COOKIE;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db, models::BugUpdate};
use tower::ServiceExt;

fn assert_no_nested_forms(html: &str) {
    let mut depth = 0;
    let mut rest = html;
    loop {
        let open = rest.find("<form");
        let close = rest.find("</form>");
        match (open, close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (Some(_), Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (Some(o), None) => {
                depth += 1;
                assert!(depth <= 1, "Detected nested <form> tag in HTML");
                rest = &rest[o + 5..];
            }
            (None, Some(c)) => {
                depth -= 1;
                assert!(depth >= 0, "Mismatched </form> tag in HTML");
                rest = &rest[c + 6..];
            }
            (None, None) => break,
        }
    }
    assert_eq!(depth, 0, "Unclosed <form> tag in HTML");
}

async fn get_cookie(
    app: &axum::Router,
    username: &str,
    password: &str,
    perm_login: bool,
) -> (String, String) {
    let body = if perm_login {
        format!("username={}&password={}&perm_login=1", username, password)
    } else {
        format!("username={}&password={}", username, password)
    };
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let raw_cookie = res
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cookie_val = raw_cookie.split(';').next().unwrap().to_string();
    (cookie_val, raw_cookie)
}

async fn set_project_cookie(app: &axum::Router, auth_cookie: &str, proj_id: i64) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", auth_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}", proj_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let proj_cookie = res
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    format!("{}; {}", auth_cookie, proj_cookie)
}

#[tokio::test]
async fn test_perm_login_remember_me_cookie() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Checked perm_login -> sets Max-Age=
    let (_, raw_cookie_perm) = get_cookie(&app, "administrator", "root", true).await;
    assert!(raw_cookie_perm.contains("Max-Age="));

    // 2. Unchecked perm_login -> omits Max-Age=
    let (_, raw_cookie_session) = get_cookie(&app, "administrator", "root", false).await;
    assert!(!raw_cookie_session.contains("Max-Age="));

    // 3. Explicitly empty perm_login -> omits Max-Age=
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=administrator&password=root&perm_login=",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let raw_cookie_empty = res.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    assert!(!raw_cookie_empty.contains("Max-Age="));
}

#[tokio::test]
async fn test_version_date_order_and_category_edit_flows() {
    let db = Db::open_in_memory().unwrap();
    let pid = db
        .create_project("Project Alpha", 10, 10, "Desc", true)
        .unwrap();
    let cat_id = db.add_category(pid, "General", 0).unwrap();
    let app = build_router(db.clone());
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;

    // 1. Add version with YYYY-MM-DD date_order
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_ver_add/{}", pid))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "version=v1.0.0&description=First+Release&released=1&date_order=2026-11-24",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let versions = db.list_versions(pid).unwrap();
    let v1 = versions.iter().find(|v| v.version == "v1.0.0").unwrap();
    let expected_epoch = rust_mantis_bt::views::parse_date_to_epoch("2026-11-24");
    assert_eq!(v1.date_order, expected_epoch);
    assert_eq!(v1.released, 1);
    assert_eq!(v1.obsolete, 0);

    // 2. Update version with integer timestamp date_order and obsolete=1
    let custom_timestamp = 1800000000;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_ver_update/{}", v1.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&version=v1.0.0-rc1&description=Updated+RC&released=0&obsolete=1&date_order={}",
                    pid, custom_timestamp
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let v1_updated = db.get_version_by_id(v1.id).unwrap().unwrap();
    assert_eq!(v1_updated.version, "v1.0.0-rc1");
    assert_eq!(v1_updated.date_order, custom_timestamp);
    assert_eq!(v1_updated.released, 0);
    assert_eq!(v1_updated.obsolete, 1);

    // 3. Update category name and default assignee
    let uid = db
        .create_user(
            "lead_dev",
            "Lead",
            "lead@example.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_cat_update/{}", cat_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&name=Core+Backend&user_id={}",
                    pid, uid
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let cats = db.list_direct_categories(pid).unwrap();
    let updated_cat = cats.iter().find(|c| c.id == cat_id).unwrap();
    assert_eq!(updated_cat.name, "Core Backend");
    assert_eq!(updated_cat.user_id, uid);

    // 4. Check /manage_proj_edit_page/:id has no nested forms and renders inputs
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/manage_proj_edit_page/{}", pid))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&html);
    assert!(html.contains("Core Backend"));
    assert!(html.contains("v1.0.0-rc1"));
    assert!(html.contains("/manage_proj_cat_update/"));
    assert!(html.contains("/manage_proj_ver_update/"));
}

#[tokio::test]
async fn test_permission_guards_and_not_found_paths() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("GuardProj", 10, 10, "", true).unwrap();
    let cat_id = db.add_category(pid, "Cat1", 0).unwrap();
    let ver_id = db.add_version(pid, "1.0", "", false, false).unwrap();
    let tag_id = db.create_tag(1, "test-tag", "desc").unwrap();
    let news_id = db.create_news(pid, 1, "News1", "Body", false, 10).unwrap();

    let _rep_id = db
        .create_user("rep_guard", "Rep", "rep@g.com", "pass", 25, true, false)
        .unwrap();
    let _dev_id = db
        .create_user("dev_guard", "Dev", "dev@g.com", "pass", 55, true, false)
        .unwrap();

    let app = build_router(db.clone());
    let (rep_cookie, _) = get_cookie(&app, "rep_guard", "pass", false).await;
    let (dev_cookie, _) = get_cookie(&app, "dev_guard", "pass", false).await;
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;

    // 1. Category update: Developer gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_cat_update/{}", cat_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}&name=Hacked", pid)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Version update: Developer gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_ver_update/{}", ver_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}&version=Hacked", pid)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Tag update: Developer gets 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tag_update/{}", tag_id))
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=hacked"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 4. News edit page & update: Reporter & Developer get 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/news_edit_page/{}", news_id))
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/news_update/{}", news_id))
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("headline=Hacked&body=Text"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 5. 404 Not Found for missing entities
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/news_edit_page/999999")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/news_update/999999")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("headline=X&body=Y"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/tag_view_page/999999")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_roadmap_ordering_badges_and_fallback() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("RoadmapProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;
    let cookie = set_project_cookie(&app, &admin_cookie, pid).await;

    // 1. Create versions in DB with distinct date_order
    // v1.0 -> date_order = 1000, released = 1
    let _v1 = db
        .add_version_with_order(pid, "v1.0", "Version 1.0", true, false, Some(1000))
        .unwrap();
    // v2.0 -> date_order = 2000, released = 0 (Planned)
    let _v2 = db
        .add_version_with_order(pid, "v2.0", "Version 2.0", false, false, Some(2000))
        .unwrap();
    // v0.9 -> date_order = 3000, obsolete = 1 (Must be excluded!)
    let _v_obs = db
        .add_version_with_order(pid, "v0.9", "Obsolete Version", true, true, Some(3000))
        .unwrap();

    // 2. Create bugs targeting these versions + an unknown legacy target_version
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let b1 = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Issue for v1.0",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b1,
        admin.id,
        BugUpdate {
            target_version: Some("v1.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    db.change_bug_status(b1, admin.id, 80, 20, None).unwrap();

    let b2 = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Issue for v2.0",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b2,
        admin.id,
        BugUpdate {
            target_version: Some("v2.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let b_obs = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Issue for obsolete",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_obs,
        admin.id,
        BugUpdate {
            target_version: Some("v0.9".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let b_legacy = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Issue for legacy target",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_legacy,
        admin.id,
        BugUpdate {
            target_version: Some("v-legacy-target".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    // 3. GET /roadmap_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/roadmap_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    // Verification:
    // a) v2.0 (date_order 2000) appears BEFORE v1.0 (date_order 1000)
    let pos_v2 = html
        .find("Target Version: v2.0")
        .expect("v2.0 should be on roadmap");
    let pos_v1 = html
        .find("Target Version: v1.0")
        .expect("v1.0 should be on roadmap");
    assert!(
        pos_v2 < pos_v1,
        "v2.0 (date_order 2000) must appear before v1.0 (date_order 1000)"
    );

    // b) Obsolete version v0.9 is completely EXCLUDED
    assert!(
        !html.contains("Target Version: v0.9"),
        "Obsolete version must be excluded"
    );
    assert!(
        !html.contains("Issue for obsolete"),
        "Issue for obsolete version must be excluded"
    );

    // c) Legacy target_version not in mantis_project_version_table is appended at the end
    let pos_legacy = html
        .find("Target Version: v-legacy-target")
        .expect("legacy target must appear");
    assert!(
        pos_v1 < pos_legacy,
        "Legacy target must appear after DB versions"
    );

    // d) Badges: Released badge for v1.0, Planned badge for v2.0 and fallback
    assert!(html.contains("Released"));
    assert!(html.contains("Planned"));
}

#[tokio::test]
async fn test_tags_detail_usage_counts_and_edit_ui() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("TagProject", 10, 10, "", true).unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    let tag_id = db
        .create_tag(admin.id, "regression", "Regression test tag")
        .unwrap();
    let b1 = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Reg issue 1",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let b2 = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Reg issue 2",
            "Desc",
            "",
            "",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.attach_tag_to_bug(b1, tag_id, admin.id).unwrap();
    db.attach_tag_to_bug(b2, tag_id, admin.id).unwrap();

    let app = build_router(db.clone());
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;

    // 1. GET /manage_tags_page shows tag name linking to /tag_view_page/:id and usage count 2
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/manage_tags_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains(&format!(r#"href="/tag_view_page/{}""#, tag_id)));
    assert!(html.contains("regression"));
    assert!(html.contains(">2<"));

    // 2. GET /tag_view_page/:id shows details, creator, count, link, and Edit form for Manager
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tag_view_page/{}", tag_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&html);
    assert!(html.contains("regression"));
    assert!(html.contains("Regression test tag"));
    assert!(html.contains("administrator"));
    assert!(html.contains("2 issue(s)"));
    assert!(html.contains(&format!(r#"href="/view_all_bug_page?tag_id={}""#, tag_id)));
    assert!(html.contains(&format!(r#"action="/tag_update/{}""#, tag_id)));

    // 3. POST /tag_update/:id edits tag
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tag_update/{}", tag_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=critical-regression&description=Urgent+regressions",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let updated_tag = db.get_tag_by_id(tag_id).unwrap().unwrap();
    assert_eq!(updated_tag.name, "critical-regression");
    assert_eq!(updated_tag.description, "Urgent regressions");
}

#[tokio::test]
async fn test_news_crud_edit_page_and_no_nested_forms() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("NewsProject", 10, 10, "", true).unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let news_id = db
        .create_news(
            pid,
            admin.id,
            "Initial Announcement",
            "Welcome all!",
            true,
            10,
        )
        .unwrap();

    let app = build_router(db.clone());
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;
    let cookie = set_project_cookie(&app, &admin_cookie, pid).await;

    // 1. GET /news_page shows Edit button
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/news_page")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains(&format!(r#"href="/news_edit_page/{}""#, news_id)));

    // 2. GET /news_edit_page/:id renders edit form with no nested forms
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/news_edit_page/{}", news_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_no_nested_forms(&html);
    assert!(html.contains("Initial Announcement"));
    assert!(html.contains("Welcome all!"));
    assert!(html.contains(&format!(r#"action="/news_update/{}""#, news_id)));

    // 3. POST /news_update/:id updates headline, body, last_modified
    let before_update = db.get_news(news_id).unwrap().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/news_update/{}", news_id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "headline=Maintenance+Scheduled&body=Downtime+at+midnight&announcement=1&view_state=10",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let after_update = db.get_news(news_id).unwrap().unwrap();
    assert_eq!(after_update.headline, "Maintenance Scheduled");
    assert_eq!(after_update.body, "Downtime at midnight");
    assert!(after_update.last_modified >= before_update.last_modified);
}

#[tokio::test]
async fn test_roadmap_subproject_versions_rollup_and_private_subproject_isolation() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Create Parent project (public) and two subprojects: PublicSub (public) & PrivateSub (private)
    let parent_id = db.create_project("ParentProj", 10, 10, "", true).unwrap();
    let pub_sub_id = db.create_project("PublicSub", 10, 10, "", true).unwrap();
    let priv_sub_id = db.create_project("PrivateSub", 50, 55, "", true).unwrap();

    db.add_subproject(parent_id, pub_sub_id, true).unwrap();
    db.add_subproject(parent_id, priv_sub_id, true).unwrap();

    // 2. Add versions with distinct date_orders
    // Parent: v-parent-1 (date_order 1000, Planned)
    db.add_version_with_order(
        parent_id,
        "v-parent-1",
        "Parent v1",
        false,
        false,
        Some(1000),
    )
    .unwrap();
    // Public Sub: v-sub-pub (date_order 2000, Released)
    db.add_version_with_order(
        pub_sub_id,
        "v-sub-pub",
        "Public Sub v1",
        true,
        false,
        Some(2000),
    )
    .unwrap();
    // Private Sub: v-sub-priv (date_order 3000, Planned)
    db.add_version_with_order(
        priv_sub_id,
        "v-sub-priv",
        "Private Sub v1",
        false,
        false,
        Some(3000),
    )
    .unwrap();

    // 3. Create bugs targeted for these versions
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    let b_parent = db
        .create_bug(
            parent_id,
            admin.id,
            1,
            "Parent issue",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_parent,
        admin.id,
        BugUpdate {
            target_version: Some("v-parent-1".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let b_pub = db
        .create_bug(
            pub_sub_id,
            admin.id,
            1,
            "Public sub issue",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_pub,
        admin.id,
        BugUpdate {
            target_version: Some("v-sub-pub".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let b_priv = db
        .create_bug(
            priv_sub_id,
            admin.id,
            1,
            "Private sub secret issue",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_priv,
        admin.id,
        BugUpdate {
            target_version: Some("v-sub-priv".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    // 4. Create a regular Reporter user (access level 25, no access to PrivateSub)
    let rep_uid = db
        .create_user(
            "sub_reporter",
            "Sub Reporter",
            "sr@example.com",
            "password123",
            25,
            true,
            false,
        )
        .unwrap();
    assert!(
        db.effective_access_level(&db.get_user_by_id(rep_uid).unwrap().unwrap(), priv_sub_id)
            .unwrap()
            < 10
    );

    // 5. Admin views roadmap for ParentProj
    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;
    let admin_proj_cookie = set_project_cookie(&app, &admin_cookie, parent_id).await;

    let res_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/roadmap_page")
                .header("cookie", &admin_proj_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_admin.status(), StatusCode::OK);
    let bytes_admin = axum::body::to_bytes(res_admin.into_body(), usize::MAX)
        .await
        .unwrap();
    let html_admin = String::from_utf8(bytes_admin.to_vec()).unwrap();

    // Admin sees all 3 versions sorted by date_order DESC: v-sub-priv (3000) > v-sub-pub (2000) > v-parent-1 (1000)
    let pos_priv = html_admin
        .find("Target Version: v-sub-priv")
        .expect("Admin must see v-sub-priv");
    let pos_pub = html_admin
        .find("Target Version: v-sub-pub")
        .expect("Admin must see v-sub-pub");
    let pos_parent = html_admin
        .find("Target Version: v-parent-1")
        .expect("Admin must see v-parent-1");
    assert!(
        pos_priv < pos_pub,
        "v-sub-priv (3000) must appear before v-sub-pub (2000)"
    );
    assert!(
        pos_pub < pos_parent,
        "v-sub-pub (2000) must appear before v-parent-1 (1000)"
    );

    assert!(html_admin.contains("Private sub secret issue"));
    assert!(html_admin.contains("Public sub issue"));
    assert!(html_admin.contains("Parent issue"));
    assert!(html_admin.contains("Released"));
    assert!(html_admin.contains("Planned"));

    // 6. Reporter views roadmap for ParentProj
    let (rep_cookie, _) = get_cookie(&app, "sub_reporter", "password123", false).await;
    let rep_proj_cookie = set_project_cookie(&app, &rep_cookie, parent_id).await;

    let res_rep = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/roadmap_page")
                .header("cookie", &rep_proj_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rep.status(), StatusCode::OK);
    let bytes_rep = axum::body::to_bytes(res_rep.into_body(), usize::MAX)
        .await
        .unwrap();
    let html_rep = String::from_utf8(bytes_rep.to_vec()).unwrap();

    // Reporter sees public subproject and parent versions
    assert!(html_rep.contains("Target Version: v-sub-pub"));
    assert!(html_rep.contains("Target Version: v-parent-1"));
    assert!(html_rep.contains("Public sub issue"));
    assert!(html_rep.contains("Parent issue"));

    // Reporter MUST NOT see private subproject version or secret issue
    assert!(
        !html_rep.contains("v-sub-priv"),
        "Private subproject version must not leak to unauthorized Reporter"
    );
    assert!(
        !html_rep.contains("Private sub secret issue"),
        "Private subproject issue must not leak to unauthorized Reporter"
    );
}

#[tokio::test]
async fn test_roadmap_all_projects_obsolete_scoping_per_project() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let (admin_cookie, _) = get_cookie(&app, "administrator", "root", false).await;
    let all_proj_cookie = set_project_cookie(&app, &admin_cookie, 0).await;

    // 1. Create Project Alpha and Project Beta
    let p_alpha = db.create_project("ProjectAlpha", 10, 10, "", true).unwrap();
    let p_beta = db.create_project("ProjectBeta", 10, 10, "", true).unwrap();

    // 2. Both have version "1.0", but Alpha marked it obsolete while Beta has it active
    db.add_version_with_order(p_alpha, "1.0", "Alpha v1.0", true, true, Some(1000))
        .unwrap();
    db.add_version_with_order(p_beta, "1.0", "Beta v1.0 active", false, false, Some(1000))
        .unwrap();

    // 3. Create bugs targeting "1.0" in each project
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    let b_alpha = db
        .create_bug(
            p_alpha,
            admin.id,
            1,
            "Alpha issue for obsolete 1.0",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_alpha,
        admin.id,
        BugUpdate {
            target_version: Some("1.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let b_beta = db
        .create_bug(
            p_beta,
            admin.id,
            1,
            "Beta active issue for 1.0",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.update_bug_fields(
        b_beta,
        admin.id,
        BugUpdate {
            target_version: Some("1.0".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    // 4. Request roadmap in All Projects view (active_project_id = 0)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/roadmap_page")
                .header("cookie", &all_proj_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();

    // Verification:
    // a) Project Beta's version "1.0" MUST be rendered
    assert!(
        html.contains("Target Version: 1.0"),
        "Active version 1.0 from Project Beta must render on roadmap"
    );
    assert!(
        html.contains("Beta active issue for 1.0"),
        "Issue for active version 1.0 must render on roadmap"
    );
    assert!(
        html.contains("ProjectBeta"),
        "ProjectBeta badge should render in multi-project view"
    );

    // b) Project Alpha's obsolete version "1.0" issue MUST be excluded
    assert!(
        !html.contains("Alpha issue for obsolete 1.0"),
        "Issue for Project Alpha obsolete 1.0 must be excluded from roadmap"
    );
}
