use super::*;

#[test]
fn test_enum_obj_helper() {
    let obj = enum_obj(10, "new");
    assert_eq!(obj["id"], 10);
    assert_eq!(obj["name"], "new");
    assert_eq!(obj["label"], "new");
}

#[test]
fn test_api_token_create_form_deserialize() {
    let form: ApiTokenCreateForm =
        serde_json::from_str(r#"{"name":"ci-token"}"#).expect("deserialize token form");
    assert_eq!(form.name, "ci-token");
}

#[test]
fn test_plugin_toggle_form_deserialize() {
    let form: PluginToggleForm =
        serde_json::from_str(r#"{"enabled":"1"}"#).expect("deserialize toggle form");
    assert_eq!(form.enabled.as_deref(), Some("1"));
}

#[test]
fn test_issues_list_query_deserialize() {
    let q: IssuesListQuery =
        serde_json::from_str(r#"{"project_id":2,"page":3,"page_size":20}"#).unwrap();
    assert_eq!(q.project_id, Some(2));
    assert_eq!(q.page, Some(3));
    assert_eq!(q.page_size, Some(20));
}

#[test]
fn test_rest_create_issue_payload_deserialize() {
    let json_data = r#"{
        "summary": "API issue",
        "description": "Details",
        "project": {"id": 1, "name": "Mantis"},
        "priority": {"id": 40},
        "severity": {"id": 60}
    }"#;
    let payload: RestCreateIssuePayload =
        serde_json::from_str(json_data).expect("deserialize rest create payload");
    assert_eq!(payload.summary, "API issue");
    assert_eq!(payload.description.as_deref(), Some("Details"));
    assert_eq!(payload.project.id, Some(1));
    assert_eq!(payload.priority.as_ref().and_then(|p| p.id), Some(40));
    assert_eq!(payload.severity.as_ref().and_then(|s| s.id), Some(60));
}

#[test]
fn test_rest_patch_issue_payload_deserialize() {
    let json_data = r#"{
        "summary": "Updated summary",
        "status": {"id": 50},
        "target_version": "1.2.0"
    }"#;
    let payload: RestPatchIssuePayload =
        serde_json::from_str(json_data).expect("deserialize patch payload");
    assert_eq!(payload.summary.as_deref(), Some("Updated summary"));
    assert_eq!(payload.status.as_ref().and_then(|s| s.id), Some(50));
    assert_eq!(payload.target_version.as_deref(), Some("1.2.0"));
    assert!(payload.description.is_none());
}

#[test]
fn test_rest_add_note_payload_deserialize() {
    let json_data = r#"{
        "text": "Added note via REST",
        "view_state": {"id": 10},
        "time_tracking": 45
    }"#;
    let payload: RestAddNotePayload =
        serde_json::from_str(json_data).expect("deserialize note payload");
    assert_eq!(payload.text, "Added note via REST");
    assert_eq!(payload.view_state.as_ref().and_then(|v| v.id), Some(10));
    assert_eq!(payload.time_tracking, Some(45));
}

#[tokio::test]
async fn test_rest_update_issue_and_add_note_access_guards() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("PublicProj", 10, 10, "", true).unwrap();
    let reporter_id = db
        .create_user("rep1", "Reporter", "rep@test.com", "pass", 25, true, false)
        .unwrap();
    let updater_id = db
        .create_user("upd1", "Updater", "upd@test.com", "pass", 40, true, false)
        .unwrap();
    let reporter = db.get_user_by_id(reporter_id).unwrap().unwrap();
    let updater = db.get_user_by_id(updater_id).unwrap().unwrap();

    // Private bug (view_state = 50)
    let priv_bug = db
        .create_bug(
            proj_id,
            1,
            1,
            "Secret Bug",
            "Confidential",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();

    // Updater (40) cannot PATCH a private bug
    let resp = rest_update_issue(
        State(db.clone()),
        ApiAuth {
            user: updater.clone(),
        },
        Path(priv_bug),
        Json(RestPatchIssuePayload {
            summary: Some("Hacked".to_string()),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Public bug created by reporter
    let pub_bug = db
        .create_bug(
            proj_id,
            reporter_id,
            1,
            "Public Bug",
            "Desc",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Reporter (25) cannot assign handler via PATCH
    let resp = rest_update_issue(
        State(db.clone()),
        ApiAuth {
            user: reporter.clone(),
        },
        Path(pub_bug),
        Json(RestPatchIssuePayload {
            handler: Some(IdNameRef {
                id: Some(1),
                name: None,
            }),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Reporter (25) cannot resolve (80) or close (90) via PATCH
    let resp = rest_update_issue(
        State(db.clone()),
        ApiAuth {
            user: reporter.clone(),
        },
        Path(pub_bug),
        Json(RestPatchIssuePayload {
            status: Some(IdNameRef {
                id: Some(80),
                name: None,
            }),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Reporter (25) cannot add a private bugnote (view_state = 50)
    let resp = rest_add_issue_note(
        State(db),
        ApiAuth { user: reporter },
        Path(pub_bug),
        Json(RestAddNotePayload {
            text: "Secret note".to_string(),
            view_state: Some(IdNameRef {
                id: Some(50),
                name: None,
            }),
            time_tracking: None,
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
