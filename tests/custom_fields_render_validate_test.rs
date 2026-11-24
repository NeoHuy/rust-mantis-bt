use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login_user(app: &axum::Router, username: &str, password: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}",
                    username, password
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    res.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
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
        .get("set-cookie")
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
async fn test_require_report_and_require_update_validation() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("RequireProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let admin_cookie = login_user(&app, "administrator", "root").await;
    let cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // Create required on report & update custom field
    db.create_custom_field_full(
        "ReqField", 0, // String
        "", "", "", 10, 25, 1, 100, true, // require_report
        true, // require_update
        true, // display_report
        true, // display_update
        false, false, false, false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // 1. Bug report without required custom field should fail and redirect with error
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=1&summary=Missing+CF&description=Desc",
                    proj_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("err="));
    assert!(loc.contains("ReqField"));
    assert_eq!(db.list_bugs(proj_id, 0, 10).unwrap().len(), 0);

    // 2. Bug report with valid required custom field succeeds
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&category_id=1&summary=Valid+CF&description=Desc&custom_field_{}=ValidValue",
                    proj_id, cf_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bugs = db.list_bugs(proj_id, 0, 10).unwrap();
    assert_eq!(bugs.len(), 1);
    let bug_id = bugs[0].id;

    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values.len(), 1);
    assert_eq!(cf_values[0].value, "ValidValue");

    // 3. Bug update attempting to clear required field should fail
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "summary=Valid+CF&description=Desc&custom_field_{}=",
                    cf_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("err="));

    // Value in DB should remain unchanged
    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "ValidValue");
}

#[tokio::test]
async fn test_multivalue_checkbox_and_multilist_submission() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("MultiProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let admin_cookie = login_user(&app, "administrator", "root").await;
    let cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // Create Checkbox field (Type 5)
    db.create_custom_field_full(
        "Platforms",
        5, // Checkbox
        "Windows|macOS|Linux",
        "",
        "",
        10,
        25,
        0,
        0,
        false,
        false,
        true,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // 1. Report bug with multiple selected checkboxes
    let form = format!(
        "project_id={}&category_id=1&summary=CrossPlatform&description=Multi&custom_field_{}_present=1&custom_field_{}=Windows&custom_field_{}=Linux",
        proj_id, cf_id, cf_id, cf_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let bug_id = db.list_bugs(proj_id, 0, 10).unwrap()[0].id;

    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "Windows|Linux");

    // 2. Update bug to uncheck all checkboxes (only presence field sent)
    let update_form = format!(
        "summary=CrossPlatform&description=Multi&custom_field_{}_present=1",
        cf_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "");
}

#[tokio::test]
async fn test_readonly_access_rw_protection() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("SecureProj", 10, 10, "", true).unwrap();

    // Create a regular user with reporter access (25)
    let reporter_id = db
        .create_user(
            "rep_user",
            "Reporter User",
            "rep@test.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let app = build_router(db.clone());

    // Create a custom field with rw = 70 (Manager required to edit)
    db.create_custom_field_full(
        "ManagerNotes",
        0, // String
        "",
        "",
        "",
        10, // Readable by all
        70, // RW requires Manager
        0,
        0,
        false,
        false,
        true,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // Create a bug directly with initial custom field value
    let bug_id = db
        .create_bug(
            proj_id,
            reporter_id,
            1,
            "Bug Title",
            "Bug Desc",
            "",
            "",
            50,
            10,
            30,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.set_custom_field_value(cf_id, bug_id, 1, "InitialManagerValue")
        .unwrap();

    // Login as reporter (access 25)
    let rep_cookie = login_user(&app, "rep_user", "password").await;
    let cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    // View update bug page: custom field should render as read-only span
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/update_bug_page/{}", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&bytes);
    assert!(html.contains(r#"<span class="readonly-custom-field">InitialManagerValue</span>"#));

    // Reporter attempts to tamper with/update ManagerNotes via POST
    let tamper_form = format!(
        "summary=Bug+Title&description=Bug+Desc&custom_field_{}=HackedValue",
        cf_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(tamper_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Value in DB MUST remain "InitialManagerValue" because effective access (25) < access_level_rw (70)
    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "InitialManagerValue");
}

#[tokio::test]
async fn test_require_resolved_and_closed_status_transitions() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("StatusProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let admin_cookie = login_user(&app, "administrator", "root").await;
    let cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // Create custom field required on resolve (80)
    db.create_custom_field_full(
        "ResolutionDetails",
        0, // String
        "",
        "",
        "",
        10,
        25,
        0,
        0,
        false,
        false,
        false,
        false,
        true, // require_resolved
        true, // display_resolved
        false,
        false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // Create a bug with status 10 (New)
    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Status Bug",
            "Desc",
            "",
            "",
            50,
            10,
            30,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Attempt to change status to 80 (Resolved) without required field
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("new_status=80"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("err="));
    assert!(loc.contains("ResolutionDetails"));

    // Bug status should still be 10 (New)
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 10);

    // Provide the required field when resolving
    let form = format!("new_status=80&custom_field_{}=Fixed+in+build+42", cf_id);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Bug status is now 80 (Resolved) and value saved
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);
    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "Fixed in build 42");
}

#[tokio::test]
async fn test_resolve_status_transition_skips_when_user_cannot_write() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("DevResolveProj", 10, 10, "", true)
        .unwrap();

    // Create Developer user (access 55)
    let dev_id = db
        .create_user(
            "dev_user",
            "Developer",
            "dev@test.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let app = build_router(db.clone());

    // Create custom field required on resolve, but only writable by Manager (70)
    db.create_custom_field_full(
        "ManagerSignoff",
        0, // String
        "",
        "",
        "",
        10,
        70, // RW requires Manager (70)
        0,
        0,
        false,
        false,
        false,
        false,
        true, // require_resolved
        true, // display_resolved
        false,
        false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // Create a bug
    let bug_id = db
        .create_bug(
            proj_id, dev_id, 1, "Dev Bug", "Desc", "", "", 50, 10, 30, "", "", "", "", 10,
        )
        .unwrap();

    // Developer logs in
    let dev_cookie = login_user(&app, "dev_user", "password").await;
    let cookie = set_project_cookie(&app, &dev_cookie, proj_id).await;

    // Developer changes status to 80 (Resolved).
    // Because Developer cannot write ManagerSignoff (55 < 70), the required check is skipped,
    // and Developer is not blocked from resolving issues!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("new_status=80"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("msg=Status+updated"));

    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);
}

#[tokio::test]
async fn test_required_radio_unselected_on_update_fails() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("RadioProj", 10, 10, "", true).unwrap();
    let app = build_router(db.clone());

    let admin_cookie = login_user(&app, "administrator", "root").await;
    let cookie = set_project_cookie(&app, &admin_cookie, proj_id).await;

    // Create Radio field (Type 9) required on update
    db.create_custom_field_full(
        "PriorityLevel",
        9, // Radio
        "Low|Medium|High",
        "Medium",
        "",
        10,
        25,
        0,
        0,
        false,
        true, // require_update
        true,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let cf_id = db.list_custom_fields().unwrap()[0].id;
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // Create a bug with initial value "Medium"
    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Radio Bug",
            "Desc",
            "",
            "",
            50,
            10,
            30,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.set_custom_field_value(cf_id, bug_id, 1, "Medium")
        .unwrap();

    // Submit update with radio present marker but no selection
    let update_form = format!(
        "summary=Radio+Bug&description=Desc&custom_field_{}_present=1",
        cf_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/update_bug/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("err="));
    assert!(loc.contains("PriorityLevel"));

    // DB value should remain "Medium"
    let cf_values = db.get_custom_field_values_for_bug(bug_id).unwrap();
    assert_eq!(cf_values[0].value, "Medium");
}

#[tokio::test]
async fn test_rest_api_filters_custom_fields_by_access_level_r() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("ApiFilterProj", 10, 10, "", true)
        .unwrap();

    // Create Viewer user (access 10)
    let viewer_id = db
        .create_user(
            "view_user",
            "Viewer",
            "view@test.com",
            "password",
            10,
            true,
            false,
        )
        .unwrap();
    let (_token_id, raw_token) = db.create_api_token(viewer_id, "Viewer API Token").unwrap();
    let app = build_router(db.clone());

    // 1. Public custom field (readable by Viewer: r=10)
    db.create_custom_field_full(
        "PublicField",
        0,
        "",
        "",
        "",
        10,
        25,
        0,
        0,
        false,
        false,
        true,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    // 2. Secret custom field (readable by Developer only: r=55)
    db.create_custom_field_full(
        "DevSecretField",
        0,
        "",
        "",
        "",
        55, // access_level_r = 55
        55,
        0,
        0,
        false,
        false,
        true,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    let all_cfs = db.list_custom_fields().unwrap();
    let cf_public_id = all_cfs.iter().find(|c| c.name == "PublicField").unwrap().id;
    let cf_secret_id = all_cfs
        .iter()
        .find(|c| c.name == "DevSecretField")
        .unwrap()
        .id;

    db.link_custom_field_to_project(cf_public_id, proj_id, 1)
        .unwrap();
    db.link_custom_field_to_project(cf_secret_id, proj_id, 2)
        .unwrap();

    // Create a bug
    let bug_id = db
        .create_bug(
            proj_id, 1, 1, "API Bug", "Desc", "", "", 50, 10, 30, "", "", "", "", 10,
        )
        .unwrap();
    db.set_custom_field_value(cf_public_id, bug_id, 1, "VisibleData")
        .unwrap();
    db.set_custom_field_value(cf_secret_id, bug_id, 1, "SecretData")
        .unwrap();

    // Viewer calls REST API GET /api/rest/issues/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/rest/issues/{}", bug_id))
                .header("authorization", format!("Bearer {}", raw_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json_val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let cfs = json_val["issues"][0]["custom_fields"].as_array().unwrap();
    // Only PublicField should be returned, DevSecretField MUST be filtered out!
    assert_eq!(cfs.len(), 1);
    assert_eq!(cfs[0]["field"]["name"], "PublicField");
    assert_eq!(cfs[0]["value"], "VisibleData");
}

#[tokio::test]
async fn test_readonly_default_value_does_not_abort_report_when_invalid() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;
    let proj_id = db
        .create_project("ReadonlyDefaultProj", 10, 10, "", true)
        .unwrap();
    let _cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // Create an Enum custom field with a default value that is NOT in possible_values.
    // access_level_rw = 70 (Manager), so a Reporter (25) cannot write it.
    let cf_id = db
        .create_custom_field_full(
            "TargetStage",
            3, // Enum
            "alpha|beta|gamma",
            "deprecated_stage", // invalid option
            "",
            10,
            70,
            0,
            0,
            false,
            false,
            true, // display_report
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    let _ = db
        .create_user(
            "reporter_user",
            "Reporter User",
            "rep@x.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_cookie = login_user(&app, "reporter_user", "pass").await;
    let rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    // Reporter reports an issue without submitting custom_field_TargetStage (since user cannot write it)
    let body = format!(
        "project_id={}&category_id=1&summary=ReportIssueOk&description=Desc",
        proj_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/bug_report")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should redirect to /view_bug_page/1 without failing on the invalid default
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.starts_with("/view_bug_page/"));
    assert!(!loc.contains("err="));
}

#[tokio::test]
async fn test_bug_change_status_ignores_writes_to_undisplayed_fields() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;
    let proj_id = db
        .create_project("StatusIgnoreProj", 10, 10, "", true)
        .unwrap();
    let cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // Field 1: display_resolved = 0
    let cf1_id = db
        .create_custom_field_full(
            "UpdateOnlyField",
            0, // String
            "",
            "",
            "",
            10,
            25,
            0,
            0,
            false,
            true,  // display_update
            false, // display_resolved
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    db.link_custom_field_to_project(cf1_id, proj_id, 1).unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "Status bug",
            "Desc",
            "",
            "",
            50,
            10,
            30,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Change status to resolved (80), attempting to sneakily set UpdateOnlyField
    let body = format!(
        "new_status=80&resolution=20&custom_field_{}=SneakyVal",
        cf1_id
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/bug_change_status/{}", bug_id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify status changed to 80, but custom field was NOT saved
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);

    let cf_vals = db.get_custom_field_values_for_bug(bug_id).unwrap();
    let cf1_val = cf_vals
        .iter()
        .find(|v| v.field_id == cf1_id)
        .map(|v| v.value.as_str())
        .unwrap_or("");
    assert_eq!(cf1_val, "");
}

#[tokio::test]
async fn test_view_bug_page_hides_fields_above_access_level_r() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;
    let proj_id = db.create_project("AccessRProj", 10, 10, "", true).unwrap();
    let cookie = set_project_cookie(&app, &cookie, proj_id).await;

    // Field: access_level_r = 70 (Manager)
    let cf_id = db
        .create_custom_field_full(
            "ManagerSecretNotes",
            0,
            "",
            "",
            "",
            70,
            70,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            1,
            1,
            "ViewBugTest",
            "Desc",
            "",
            "",
            50,
            10,
            30,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.set_custom_field_value(cf_id, bug_id, 1, "ConfidentialPayload")
        .unwrap();

    // Reporter (eff 25) views bug: ManagerSecretNotes should NOT appear in HTML
    let _ = db
        .create_user(
            "viewer_rep",
            "Viewer Rep",
            "v@x.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_cookie = login_user(&app, "viewer_rep", "pass").await;
    let rep_cookie = set_project_cookie(&app, &rep_cookie, proj_id).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&bytes);
    assert!(!html.contains("ManagerSecretNotes"));
    assert!(!html.contains("ConfidentialPayload"));

    // Administrator (eff 90) views bug: ManagerSecretNotes SHOULD appear
    let res_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/view_bug_page/{}", bug_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_admin.status(), StatusCode::OK);
    let bytes_admin = res_admin.into_body().collect().await.unwrap().to_bytes();
    let html_admin = String::from_utf8_lossy(&bytes_admin);
    assert!(html_admin.contains("ManagerSecretNotes"));
    assert!(html_admin.contains("ConfidentialPayload"));
}

#[tokio::test]
async fn test_manage_custom_field_admin_crud_saves_new_access_and_display_flags() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let cookie = login_user(&app, "administrator", "root").await;

    // Create custom field via HTTP POST /manage_custom_field_create
    let form_body = "name=WorkflowTag&field_type=0&access_level_r=25&access_level_rw=55&valid_regexp=%5E%5BA-Z%5D%2B%24&display_resolved=1&require_resolved=1";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_custom_field_create")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let fields = db.list_custom_fields().unwrap();
    let cf = fields
        .iter()
        .find(|f| f.name == "WorkflowTag")
        .expect("WorkflowTag should exist");
    assert_eq!(cf.access_level_r, 25);
    assert_eq!(cf.access_level_rw, 55);
    assert_eq!(cf.display_resolved, 1);
    assert_eq!(cf.require_resolved, 1);
    assert_eq!(cf.valid_regexp, "^[A-Z]+$");

    // Update custom field via HTTP POST /manage_custom_field_update/{id}
    let update_body = "name=WorkflowTag&field_type=0&access_level_r=25&access_level_rw=70&display_resolved=1&require_resolved=1&display_closed=1&require_closed=1";
    let res_upd = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_custom_field_update/{}", cf.id))
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_upd.status(), StatusCode::SEE_OTHER);

    let cf_updated = db.get_custom_field_by_id(cf.id).unwrap().unwrap();
    assert_eq!(cf_updated.access_level_rw, 70);
    assert_eq!(cf_updated.display_closed, 1);
    assert_eq!(cf_updated.require_closed, 1);
}
