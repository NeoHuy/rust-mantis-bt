use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

#[tokio::test]
async fn test_login_project_creation_and_private_project_cookie_guard() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Unauthenticated GET / redirects to /login
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 2. POST /login with administrator/root sets MANTIS_STRING_COOKIE
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=root"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_hdr.contains("MANTIS_STRING_COOKIE="));

    // 3. Create a private project via POST /manage_proj_create
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_proj_create")
                .header("cookie", &cookie_hdr)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "name=SecretCore&status=10&view_state=50&description=Top+secret&inherit_global=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 4. Create a Reporter user and verify they cannot switch active project to SecretCore (id=1)
    let uid = db
        .create_user(
            "rep1",
            "Reporter One",
            "r@x.com",
            "pass123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep = db.get_user_by_id(uid).unwrap().unwrap();
    let rep_cookie = format!(
        "MANTIS_STRING_COOKIE={}; MANTIS_PROJECT_COOKIE=1",
        rep.cookie_string
    );

    let res = app
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // SecretCore must not appear in Reporter's HTML response
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(!html.contains("SecretCore"));
}

#[tokio::test]
async fn test_auth_flow_login_failure_logout_and_account_update() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // 1. Bad password returns 401 Unauthorized
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=wrongpassword"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Successful login returns 303 and sets cookie
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=root"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 3. GET /account_page with valid cookie returns 200 OK
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", &cookie_hdr)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Update account with mismatched passwords returns 400
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_update")
                .header("cookie", &cookie_hdr)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("realname=Admin+Renamed&email=admin@test.com&password=newpass&password_confirm=mismatch"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 5. Update account successfully (realname & email)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_update")
                .header("cookie", &cookie_hdr)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("realname=Admin+Renamed&email=admin@test.com"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let updated = db.get_user_by_username("administrator").unwrap().unwrap();
    assert_eq!(updated.realname, "Admin Renamed");
    assert_eq!(updated.email, "admin@test.com");

    // 6. POST /logout returns 303 and clears cookie
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/logout")
                .header("cookie", &cookie_hdr)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let logout_cookies = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(logout_cookies.contains("Max-Age=0"));

    // 7. Requesting /account_page with empty cookie redirects to /login
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/account_page")
                .header("cookie", "MANTIS_STRING_COOKIE=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get("location").unwrap(), "/login");
}

#[tokio::test]
async fn test_admin_user_crud_and_access_control() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    // Create a regular developer user
    let dev_id = db
        .create_user(
            "dev_user",
            "Developer One",
            "d@x.com",
            "pass1",
            55,
            true,
            false,
        )
        .unwrap();
    let dev_user = db.get_user_by_id(dev_id).unwrap().unwrap();
    let dev_cookie = format!("MANTIS_STRING_COOKIE={}", dev_user.cookie_string);

    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

    // 1. Developer is forbidden from /manage_user_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_user_page")
                .header("cookie", &dev_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Developer is forbidden from /manage_user_create
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_user_create")
                .header("cookie", &dev_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=hacker&password=pass&access_level=90"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Admin can view /manage_user_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_user_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Admin can create a new user via POST /manage_user_create
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_user_create")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=newdev&realname=New+Dev&email=nd@x.com&password=devpass123&access_level=55&enabled=1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let created = db.get_user_by_username("newdev").unwrap().unwrap();
    assert_eq!(created.access_level, 55);

    // 5. Admin can update the user via POST /manage_user_update/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_user_update/{}", created.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "realname=Updated+Dev&email=updated@x.com&access_level=70&enabled=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let updated = db.get_user_by_id(created.id).unwrap().unwrap();
    assert_eq!(updated.realname, "Updated Dev");
    assert_eq!(updated.access_level, 70);

    // 6. Admin can delete the user via POST /manage_user_delete/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_user_delete/{}", created.id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(db.get_user_by_id(created.id).unwrap().is_none());

    // 7. Admin cannot delete the sole administrator
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_user_delete/{}", admin.id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_project_management_subprojects_categories_versions_users() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

    let rep_id = db
        .create_user("rep_user", "Reporter", "r@x.com", "pass", 25, true, false)
        .unwrap();
    let rep_user = db.get_user_by_id(rep_id).unwrap().unwrap();
    let rep_cookie = format!("MANTIS_STRING_COOKIE={}", rep_user.cookie_string);

    // 1. Reporter forbidden from project management
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_proj_page")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Admin creates Parent Project (ProjA)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_proj_create")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ProjectAlpha&status=10&view_state=10&description=Parent+Project&inherit_global=1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 3. Admin creates Child Project (ProjB)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_proj_create")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ProjectBeta&status=10&view_state=10&description=Child+Project&inherit_global=1"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let proj_a = db
        .list_all_projects()
        .unwrap()
        .into_iter()
        .find(|p| p.name == "ProjectAlpha")
        .unwrap();
    let proj_b = db
        .list_all_projects()
        .unwrap()
        .into_iter()
        .find(|p| p.name == "ProjectBeta")
        .unwrap();

    // 4. Link ProjB as subproject of ProjA
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_subproj_add/{}", proj_a.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "child_id={}&inherit_parent=1",
                    proj_b.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // 5. Cyclic hierarchy attempt (linking ProjA under ProjB) returns 400 Bad Request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_subproj_add/{}", proj_b.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "child_id={}&inherit_parent=1",
                    proj_a.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 6. Add category to ProjA
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_proj_cat_add")
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}&name=Backend", proj_a.id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let cats = db.list_direct_categories(proj_a.id).unwrap();
    assert!(cats.iter().any(|c| c.name == "Backend"));

    // 7. Add version to ProjA
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_ver_add/{}", proj_a.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "version=v1.0.0&description=First+release&released=1",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let vers = db.list_versions(proj_a.id).unwrap();
    assert!(vers.iter().any(|v| v.version == "v1.0.0"));

    // 8. Add project user mapping (rep_user -> Manager 70 on ProjA)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_user_add/{}", proj_a.id))
                .header("cookie", &admin_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("user_id={}&access_level=70", rep_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let effective_level = db.effective_access_level(&rep_user, proj_a.id).unwrap();
    assert_eq!(effective_level, 70);

    // 9. Remove project user mapping
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/manage_proj_user_remove/{}/{}", proj_a.id, rep_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let effective_level_after = db.effective_access_level(&rep_user, proj_a.id).unwrap();
    assert_eq!(effective_level_after, 25); // back to global access level
}

#[tokio::test]
async fn test_set_project_cookie_and_access_resolution() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let p_pub_id = db
        .create_project("PublicProj", 10, 10, "Public", true)
        .unwrap();
    let p_priv_id = db
        .create_project("PrivateProj", 10, 50, "Private", true)
        .unwrap();

    let rep_id = db
        .create_user("rep_tester", "Rep", "r@t.com", "pass", 25, true, false)
        .unwrap();
    let rep_user = db.get_user_by_id(rep_id).unwrap().unwrap();
    let rep_cookie = format!("MANTIS_STRING_COOKIE={}", rep_user.cookie_string);

    // 1. Reporter switches to PublicProj (id = p_pub_id) -> cookie set to p_pub_id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}", p_pub_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_hdr.contains(&format!("MANTIS_PROJECT_COOKIE={}", p_pub_id)));

    // 2. Reporter switches to PrivateProj (id = p_priv_id) without explicit access -> cookie falls back to 0
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/set_project")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("project_id={}", p_priv_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let cookie_hdr = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_hdr.contains("MANTIS_PROJECT_COOKIE=0"));

    // 3. GET / as Reporter redirects to /account_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("cookie", &rep_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get("location").unwrap(), "/account_page");

    // 4. GET / as Admin redirects to /manage_proj_page
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers().get("location").unwrap(), "/manage_proj_page");
}

fn assert_no_nested_forms(html: &str) {
    let mut depth: i32 = 0;
    let mut rest = html;
    while !rest.is_empty() {
        let open_pos = rest.find("<form");
        let close_pos = rest.find("</form");
        match (open_pos, close_pos) {
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

#[tokio::test]
async fn test_no_nested_forms_in_edit_pages() {
    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());

    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

    let uid = db
        .create_user("testuser", "Test User", "t@x.com", "pass", 25, true, false)
        .unwrap();
    let pid = db.create_project("TestProj", 10, 10, "Desc", true).unwrap();

    // 1. Check /manage_user_edit_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/manage_user_edit_page/{}", uid))
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

    // 2. Check /manage_proj_edit_page/:id
    let res = app
        .clone()
        .oneshot(
            Request::builder()
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

    // 3. Check /manage_proj_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_proj_page")
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

    // 4. Check /manage_user_page
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/manage_user_page")
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
}
