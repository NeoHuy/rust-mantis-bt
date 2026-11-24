use super::*;

#[test]
fn test_news_add_form_deserialize() {
    let form: NewsAddForm = serde_json::from_str(
        r#"{
            "headline": "Sprint Freeze",
            "body": "Testing phase",
            "announcement": "1",
            "view_state": 10
        }"#,
    )
    .unwrap();
    assert_eq!(form.headline, "Sprint Freeze");
    assert_eq!(form.body, "Testing phase");
    assert_eq!(form.announcement.as_deref(), Some("1"));
    assert_eq!(form.view_state, Some(10));
}

#[test]
fn test_news_add_form_defaults() {
    let form: NewsAddForm = serde_json::from_str(
        r#"{
            "headline": "Sprint Freeze",
            "body": "Testing phase"
        }"#,
    )
    .unwrap();
    assert_eq!(form.headline, "Sprint Freeze");
    assert_eq!(form.body, "Testing phase");
    assert_eq!(form.announcement, None);
    assert_eq!(form.view_state, None);
    assert_eq!(form.project_id, None);
}

#[test]
fn test_planning_flash_query_deserialize() {
    let q: FlashQuery = serde_json::from_str(r#"{"msg": "Updated", "err": "Failed"}"#).unwrap();
    assert_eq!(q.msg.as_deref(), Some("Updated"));
    assert_eq!(q.err.as_deref(), Some("Failed"));

    let q_empty: FlashQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q_empty.msg, None);
    assert_eq!(q_empty.err, None);
}
