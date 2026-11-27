use super::*;
use crate::models::User;

fn test_user(id: i64, access_level: i32) -> User {
    User {
        id,
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
        cookie_string: String::new(),
        last_visit: 0,
        date_created: 0,
    }
}

#[test]
fn test_columns_update_form_deserializes() {
    let form: UpdateColumnsForm = serde_json::from_str(
        r#"{"project_id":0,"view_issues_columns":"id, priority, summary","print_issues_columns":"id, summary"}"#,
    )
    .unwrap();
    assert_eq!(form.project_id, Some(0));
    assert!(form.view_issues_columns.contains("priority"));
    assert_eq!(form.csv_columns, "");
    assert_eq!(form.excel_columns, "");

    let q_bad: ColumnsPageQuery = serde_json::from_str(r#"{"project_id":"abc"}"#).unwrap();
    assert_eq!(q_bad.project_id.as_deref(), Some("abc"));
}

#[tokio::test]
async fn test_columns_http_update_reset_and_inaccessible_project() {
    use crate::controllers::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let admin = db.authenticate("administrator", "root").unwrap().unwrap();
    let cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

    // Lenient query with non-numeric project_id returns 200
    let res_bad_q = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/account_manage_columns_page?project_id=abc")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad_q.status(), StatusCode::OK);

    // Reporter updating columns for inaccessible private project gets 403 Forbidden
    let rep_id = db
        .create_user(
            "col_rep",
            "Col Rep",
            "colrep@x.com",
            "pw123",
            25,
            true,
            false,
        )
        .unwrap();
    let rep = db.authenticate("col_rep", "pw123").unwrap().unwrap();
    let rep_cookie = format!("MANTIS_STRING_COOKIE={}", rep.cookie_string);
    let priv_proj = db.create_project("PrivColProj", 10, 50, "", true).unwrap();
    let _ = rep_id;
    let res_deny = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_manage_columns_update")
                .header("cookie", &rep_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "project_id={}&view_issues_columns=id,summary&print_issues_columns=id,summary&csv_columns=id,summary&excel_columns=id,summary",
                    priv_proj
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_deny.status(), StatusCode::FORBIDDEN);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_manage_columns_update")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "project_id=0&view_issues_columns=id,summary&print_issues_columns=id,summary&csv_columns=id,summary&excel_columns=id,summary",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let saved = db
        .get_user_columns(&test_user(admin.id, 90), 0, ColumnContext::View)
        .unwrap();
    assert_eq!(saved, vec!["id", "summary"]);

    let res_reset = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/account_manage_columns_reset")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("project_id=0"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_reset.status(), StatusCode::SEE_OTHER);

    let after_reset = db
        .get_user_columns(&test_user(admin.id, 90), 0, ColumnContext::View)
        .unwrap();
    assert!(after_reset.contains(&"selection".to_string()));
}
