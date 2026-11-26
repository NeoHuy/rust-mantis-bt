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
    let form = UpdateColumnsForm {
        project_id: Some(0),
        view_issues_columns: "id, priority, summary".to_string(),
        print_issues_columns: "id, summary".to_string(),
        csv_columns: "id, summary".to_string(),
        excel_columns: "id, summary".to_string(),
    };
    assert_eq!(form.project_id, Some(0));
    assert!(form.view_issues_columns.contains("priority"));
}

#[tokio::test]
async fn test_columns_http_update_and_reset_cycle() {
    use crate::controllers::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let db = Db::open_in_memory().unwrap();
    let app = build_router(db.clone());
    let admin = db.authenticate("administrator", "root").unwrap().unwrap();
    let cookie = format!("MANTIS_STRING_COOKIE={}", admin.cookie_string);

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
