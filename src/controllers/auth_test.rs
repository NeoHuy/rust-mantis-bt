use super::*;

#[test]
fn test_login_form_deserialize() {
    let form: LoginForm =
        serde_json::from_str(r#"{"username":"admin","password":"secret"}"#).unwrap();
    assert_eq!(form.username, "admin");
    assert_eq!(form.password, "secret");
    assert_eq!(form.perm_login, None);

    let form_perm: LoginForm =
        serde_json::from_str(r#"{"username":"admin","password":"secret","perm_login":"1"}"#)
            .unwrap();
    assert_eq!(form_perm.perm_login.as_deref(), Some("1"));
}

#[test]
fn test_set_project_form_deserialize() {
    let form: SetProjectForm =
        serde_json::from_str(r#"{"project_id":42,"return_url":"/view_bug_page/1"}"#).unwrap();
    assert_eq!(form.project_id, 42);
    assert_eq!(form.return_url.as_deref(), Some("/view_bug_page/1"));
}

#[test]
fn test_account_update_form_deserialize() {
    let form: AccountUpdateForm = serde_json::from_str(
        r#"{"realname":"John Doe","email":"john@example.com","password":"newpass","password_confirm":"newpass"}"#,
    )
    .unwrap();
    assert_eq!(form.realname, "John Doe");
    assert_eq!(form.email, "john@example.com");
    assert_eq!(form.password.as_deref(), Some("newpass"));
    assert_eq!(form.password_confirm.as_deref(), Some("newpass"));
    assert_eq!(form.current_password, None);
}

#[tokio::test]
async fn test_login_submit_invalid_credentials() {
    let db = Db::open_in_memory().unwrap();
    let form = LoginForm {
        username: "nonexistent".to_string(),
        password: "wrong".to_string(),
        perm_login: None,
    };
    let resp = login_submit(State(db), Form(form)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_submit_valid_credentials() {
    let db = Db::open_in_memory().unwrap();

    // 1. Unchecked / omitted perm_login -> session cookie without Max-Age=
    let form_session = LoginForm {
        username: "administrator".to_string(),
        password: "root".to_string(),
        perm_login: None,
    };
    let resp = login_submit(State(db.clone()), Form(form_session)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let cookie_val = resp.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    assert!(!cookie_val.contains("Max-Age="));

    // 2. Checked perm_login -> persistent cookie with Max-Age=
    let form_perm = LoginForm {
        username: "administrator".to_string(),
        password: "root".to_string(),
        perm_login: Some("1".to_string()),
    };
    let resp_perm = login_submit(State(db), Form(form_perm)).await;
    assert_eq!(resp_perm.status(), StatusCode::SEE_OTHER);
    let cookie_perm_val = resp_perm
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie_perm_val.contains("Max-Age="));
}

#[tokio::test]
async fn test_logout_clears_cookie() {
    let db = Db::open_in_memory().unwrap();
    let user = db.authenticate("administrator", "root").unwrap().unwrap();
    let old_cookie = user.cookie_string.clone();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_str(&format!("MANTIS_STRING_COOKIE={}", old_cookie)).unwrap(),
    );

    let resp = logout(headers, State(db.clone())).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let cookie_val = resp.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie_val.contains("Max-Age=0"));
    assert!(db.get_user_by_cookie(&old_cookie).unwrap().is_none());
}

#[tokio::test]
async fn test_login_submit_lockout_blocks_login_without_enumeration() {
    let db = Db::open_in_memory().unwrap();
    for _ in 0..5 {
        let resp = login_submit(
            State(db.clone()),
            Form(LoginForm {
                username: "administrator".to_string(),
                password: "wrong".to_string(),
                perm_login: None,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // After 5 failed attempts, the account is locked; even with the correct password,
    // login is denied with 401 UNAUTHORIZED to avoid username enumeration
    let resp = login_submit(
        State(db.clone()),
        Form(LoginForm {
            username: "administrator".to_string(),
            password: "root".to_string(),
            perm_login: None,
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Non-existent username also returns identical 401 UNAUTHORIZED
    let resp_nonexistent = login_submit(
        State(db),
        Form(LoginForm {
            username: "nonexistent_victim".to_string(),
            password: "any_password".to_string(),
            perm_login: None,
        }),
    )
    .await;
    assert_eq!(resp_nonexistent.status(), StatusCode::UNAUTHORIZED);
}
