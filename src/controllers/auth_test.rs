use super::*;

#[test]
fn test_login_form_deserialize() {
    let form: LoginForm =
        serde_json::from_str(r#"{"username":"admin","password":"secret"}"#).unwrap();
    assert_eq!(form.username, "admin");
    assert_eq!(form.password, "secret");
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
}

#[tokio::test]
async fn test_login_submit_invalid_credentials() {
    let db = Db::open_in_memory().unwrap();
    let form = LoginForm {
        username: "nonexistent".to_string(),
        password: "wrong".to_string(),
    };
    let resp = login_submit(State(db), Form(form)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_submit_valid_credentials() {
    let db = Db::open_in_memory().unwrap();
    let form = LoginForm {
        username: "administrator".to_string(),
        password: "root".to_string(),
    };
    let resp = login_submit(State(db), Form(form)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(resp.headers().contains_key(SET_COOKIE));
}

#[tokio::test]
async fn test_logout_clears_cookie() {
    let resp = logout().await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let cookie_val = resp.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie_val.contains("Max-Age=0"));
}
