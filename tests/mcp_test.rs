use axum::body::Body;
use axum::http::{Request, StatusCode};
use rust_mantis_bt::{build_router, db::Db};
use tower::ServiceExt;

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
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

#[tokio::test]
async fn test_token_governance_and_admin_oversight() {
    let db = Db::open_in_memory().unwrap();
    // Default admin already created (id=1, administrator/root, level=90)
    // Create Reporter user (level=25)
    let reporter_id = db
        .create_user(
            "reporter_user",
            "Reporter User",
            "reporter@test.com",
            "password123",
            25,
            true,
            false,
        )
        .unwrap();
    // Create Viewer user (level=10)
    let _viewer_id = db
        .create_user(
            "viewer_user",
            "Viewer User",
            "viewer@test.com",
            "password123",
            10,
            true,
            false,
        )
        .unwrap();

    let app = build_router(db.clone());

    let viewer_cookie = login(&app, "viewer_user", "password123").await;
    let reporter_cookie = login(&app, "reporter_user", "password123").await;
    let admin_cookie = login(&app, "administrator", "root").await;

    // 1. Viewer cannot view api_tokens_page (403 Forbidden)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Viewer cannot create API token (403 Forbidden)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &viewer_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ViewerToken"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Reporter can view api_tokens_page (200 OK)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &reporter_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(!body_str.contains("System-Wide API Tokens"));

    // 4. Reporter creates an API token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api_token_create")
                .header("cookie", &reporter_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=ReporterToken"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify token exists via db
    let reporter_tokens = db.list_api_tokens(reporter_id).unwrap();
    assert_eq!(reporter_tokens.len(), 1);
    let token_id = reporter_tokens[0].id;

    // 5. Admin lists all system tokens via Db
    let all_tokens = db.list_all_api_tokens().unwrap();
    assert!(all_tokens
        .iter()
        .any(|(t, username, _)| t.id == token_id && username == "reporter_user"));

    // 6. Admin visits api_tokens_page and sees System-Wide API Tokens table
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api_tokens_page")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("System-Wide API Tokens (Administrator View)"));
    assert!(body_str.contains("reporter_user"));

    // 7. Admin revokes Reporter's token
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api_token_revoke/{}", token_id))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Verify token is gone
    let reporter_tokens = db.list_api_tokens(reporter_id).unwrap();
    assert_eq!(reporter_tokens.len(), 0);
}

#[test]
fn test_cli_token_creation() {
    let db = Db::open_in_memory().unwrap();
    db.create_user(
        "reporter_user",
        "Reporter User",
        "rep@test.com",
        "pass",
        25,
        true,
        false,
    )
    .unwrap();
    db.create_user(
        "viewer_user",
        "Viewer User",
        "view@test.com",
        "pass",
        10,
        true,
        false,
    )
    .unwrap();

    // 1. Normal server start (no create-token arg) returns Ok(None)
    let res = rust_mantis_bt::handle_cli_command(&db, &["mantis".to_string()]);
    assert_eq!(res, Ok(None));

    // 2. Non-existent user returns Err
    let res = rust_mantis_bt::handle_cli_command(
        &db,
        &[
            "mantis".to_string(),
            "create-token".to_string(),
            "--user".to_string(),
            "ghost".to_string(),
        ],
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("not found"));

    // 3. Viewer user (< 25) returns Err
    let res = rust_mantis_bt::handle_cli_command(
        &db,
        &[
            "mantis".to_string(),
            "create-token".to_string(),
            "--user".to_string(),
            "viewer_user".to_string(),
        ],
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .contains("Reporter (25) or higher is required"));

    // 4. Reporter user (>= 25) returns Ok(Some(output))
    let res = rust_mantis_bt::handle_cli_command(
        &db,
        &[
            "mantis".to_string(),
            "create-token".to_string(),
            "--user".to_string(),
            "reporter_user".to_string(),
            "--name".to_string(),
            "Terminal Token".to_string(),
        ],
    );
    assert!(res.is_ok());
    let out = res.unwrap().unwrap();
    assert!(out.contains("Terminal Token"));
    assert!(out.contains("reporter_user"));

    // Verify token is authenticatable
    let user = db.get_user_by_username("reporter_user").unwrap().unwrap();
    let tokens = db.list_api_tokens(user.id).unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "Terminal Token");
}

#[test]
fn test_mcp_tools_and_rbac_dispatch() {
    use rust_mantis_bt::mcp::tools::{all_tool_definitions, dispatch_tool_call};
    use serde_json::json;

    let db = Db::open_in_memory().unwrap();
    let dev_id = db
        .create_user(
            "dev_user",
            "Developer",
            "dev@test.com",
            "pass",
            55,
            true,
            false,
        )
        .unwrap();
    let dev_user = db.get_user_by_id(dev_id).unwrap().unwrap();

    let mgr_id = db
        .create_user(
            "mgr_user",
            "Manager",
            "mgr@test.com",
            "pass",
            70,
            true,
            false,
        )
        .unwrap();
    let mgr_user = db.get_user_by_id(mgr_id).unwrap().unwrap();

    let updater_id = db
        .create_user(
            "updater_user",
            "Updater",
            "up@test.com",
            "pass",
            40,
            true,
            false,
        )
        .unwrap();
    let updater_user = db.get_user_by_id(updater_id).unwrap().unwrap();

    let rep_id = db
        .create_user(
            "rep_user",
            "Reporter",
            "rep@test.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let rep_user = db.get_user_by_id(rep_id).unwrap().unwrap();

    let rep2_id = db
        .create_user(
            "rep2_user",
            "Reporter 2",
            "rep2@test.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();
    let rep2_user = db.get_user_by_id(rep2_id).unwrap().unwrap();

    let view_id = db
        .create_user(
            "view_user",
            "Viewer",
            "view@test.com",
            "pass",
            10,
            true,
            false,
        )
        .unwrap();
    let view_user = db.get_user_by_id(view_id).unwrap().unwrap();

    // Projects: 1 public (view_state=10), 1 private (view_state=50)
    let pub_proj_id = db
        .create_project("Public Proj", 10, 10, "Public", true)
        .unwrap();
    let cat_id = db.add_category(pub_proj_id, "General", 0).unwrap();

    let priv_proj_id = db
        .create_project("Private Proj", 10, 50, "Private", true)
        .unwrap();
    // Assign dev_user to private proj with level 55
    db.set_project_user(priv_proj_id, dev_id, 55).unwrap();

    // Verify all 8 tool definitions
    let tools = all_tool_definitions();
    assert_eq!(tools.len(), 8);

    // 1. whoami
    let res = dispatch_tool_call(&db, &rep_user, "whoami", json!({}));
    assert!(!res.is_error);
    assert!(res.content[0].text.contains("rep_user"));
    assert!(res.content[0].text.contains("25"));

    // 2. list_projects
    let res = dispatch_tool_call(&db, &rep_user, "list_projects", json!({}));
    assert!(!res.is_error);
    assert!(res.content[0].text.contains("Public Proj"));
    assert!(!res.content[0].text.contains("Private Proj"));

    let res = dispatch_tool_call(&db, &dev_user, "list_projects", json!({}));
    assert!(!res.is_error);
    assert!(res.content[0].text.contains("Public Proj"));
    assert!(res.content[0].text.contains("Private Proj"));

    // 3. get_project_metadata
    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "get_project_metadata",
        json!({ "project_id": pub_proj_id }),
    );
    assert!(!res.is_error);
    assert!(res.content[0].text.contains("General"));

    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "get_project_metadata",
        json!({ "project_id": priv_proj_id }),
    );
    assert!(res.is_error);
    assert!(res.content[0].text.contains("Permission Denied"));

    // 4. create_issue
    // project_id <= 0 fails
    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "create_issue",
        json!({
            "project_id": 0,
            "category_id": cat_id,
            "summary": "Bug under 0",
            "description": "Desc"
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0]
        .text
        .contains("Project ID must be greater than 0"));

    // invalid category_id fails
    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "create_issue",
        json!({
            "project_id": pub_proj_id,
            "category_id": 9999,
            "summary": "Bug with wrong cat",
            "description": "Desc"
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0]
        .text
        .contains("not found or does not belong to project"));

    // Viewer fails
    let res = dispatch_tool_call(
        &db,
        &view_user,
        "create_issue",
        json!({
            "project_id": pub_proj_id,
            "category_id": cat_id,
            "summary": "Bug by viewer",
            "description": "Desc"
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0]
        .text
        .contains("Reporter (25) or higher access required"));

    // Reporter succeeds
    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "create_issue",
        json!({
            "project_id": pub_proj_id,
            "category_id": cat_id,
            "summary": "Bug by reporter",
            "description": "Desc"
        }),
    );
    assert!(!res.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&res.content[0].text).unwrap();
    let issue_id = parsed["issue_id"].as_i64().unwrap();

    let created_bug = db.get_bug(issue_id).unwrap().unwrap();
    assert_eq!(created_bug.reporter_id, rep_user.id);

    // 5. add_issue_note
    // Reporter cannot record time tracking
    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "add_issue_note",
        json!({
            "issue_id": issue_id,
            "text": "Note with time",
            "time_tracking": 120
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0]
        .text
        .contains("Developer (55) or higher required to record time tracking"));

    // Developer can record time tracking
    let res = dispatch_tool_call(
        &db,
        &dev_user,
        "add_issue_note",
        json!({
            "issue_id": issue_id,
            "text": "Dev note with time",
            "time_tracking": 60
        }),
    );
    assert!(!res.is_error);

    let res = dispatch_tool_call(
        &db,
        &rep_user,
        "add_issue_note",
        json!({
            "issue_id": issue_id,
            "text": "Note from reporter"
        }),
    );
    assert!(!res.is_error);
    let notes = db.list_bugnotes(issue_id, 90).unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].reporter_id, dev_user.id);
    assert_eq!(notes[1].reporter_id, rep_user.id);

    // 6. update_issue
    // Different reporter (rep2_user) tries to update -> Permission Denied
    let res = dispatch_tool_call(
        &db,
        &rep2_user,
        "update_issue",
        json!({
            "issue_id": issue_id,
            "status": 50
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0].text.contains("Permission Denied"));

    // Developer updates issue and assigns handler -> OK
    let res = dispatch_tool_call(
        &db,
        &dev_user,
        "update_issue",
        json!({
            "issue_id": issue_id,
            "status": 50,
            "handler_id": dev_id
        }),
    );
    assert!(!res.is_error);
    let updated_bug = db.get_bug(issue_id).unwrap().unwrap();
    assert_eq!(updated_bug.status, 50);
    assert_eq!(updated_bug.handler_id, dev_id);

    // Developer tries to close issue (status=90) without Manager level (70) -> Denied
    let res = dispatch_tool_call(
        &db,
        &dev_user,
        "update_issue",
        json!({
            "issue_id": issue_id,
            "status": 90
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0].text.contains("Access level 70 required"));

    // Manager can close issue (status=90) -> OK
    let res = dispatch_tool_call(
        &db,
        &mgr_user,
        "update_issue",
        json!({
            "issue_id": issue_id,
            "status": 90
        }),
    );
    assert!(!res.is_error);
    let closed_bug = db.get_bug(issue_id).unwrap().unwrap();
    assert_eq!(closed_bug.status, 90);

    // Reopen to feedback (status=20) by developer -> OK
    let res = dispatch_tool_call(
        &db,
        &dev_user,
        "update_issue",
        json!({
            "issue_id": issue_id,
            "status": 20
        }),
    );
    assert!(!res.is_error);

    // 7. Private issue visibility
    let priv_issue_res = dispatch_tool_call(
        &db,
        &dev_user,
        "create_issue",
        json!({
            "project_id": pub_proj_id,
            "category_id": cat_id,
            "summary": "Secret private issue",
            "description": "Secret details",
            "view_state": 50
        }),
    );
    assert!(!priv_issue_res.is_error);
    let parsed_priv: serde_json::Value =
        serde_json::from_str(&priv_issue_res.content[0].text).unwrap();
    let priv_issue_id = parsed_priv["issue_id"].as_i64().unwrap();

    // Viewer gets denied on get_issue
    let res = dispatch_tool_call(
        &db,
        &view_user,
        "get_issue",
        json!({ "issue_id": priv_issue_id }),
    );
    assert!(res.is_error);
    assert!(res.content[0].text.contains("Access Denied"));

    // Updater (< 55 and not reporter) gets denied on update_issue for private issue
    let res = dispatch_tool_call(
        &db,
        &updater_user,
        "update_issue",
        json!({
            "issue_id": priv_issue_id,
            "summary": "Hacked private summary"
        }),
    );
    assert!(res.is_error);
    assert!(res.content[0]
        .text
        .contains("Access Denied to private issue"));

    // Viewer search_issues does NOT return priv_issue
    let res = dispatch_tool_call(
        &db,
        &view_user,
        "search_issues",
        json!({ "project_id": pub_proj_id }),
    );
    assert!(!res.is_error);
    assert!(!res.content[0].text.contains("Secret private issue"));

    // Developer can get_issue
    let res = dispatch_tool_call(
        &db,
        &dev_user,
        "get_issue",
        json!({ "issue_id": priv_issue_id }),
    );
    assert!(!res.is_error);
    assert!(res.content[0].text.contains("Secret private issue"));
}

#[tokio::test]
async fn test_mcp_sse_and_messages_e2e() {
    use serde_json::json;

    let db = Db::open_in_memory().unwrap();
    let rep_id = db
        .create_user(
            "mcp_reporter",
            "MCP Reporter",
            "mcprep@test.com",
            "pass",
            25,
            true,
            false,
        )
        .unwrap();

    let (token_id, raw_token) = db.create_api_token(rep_id, "MCP Test Token").unwrap();
    let app = build_router(db.clone());

    // 1. Unauthenticated GET /mcp/sse returns 401
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp/sse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Invalid token returns 401
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp/sse?token=bogus_token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 3. Valid token connects and returns SSE endpoint event
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/mcp/sse?token={}", raw_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    use http_body_util::BodyExt;
    let mut body = res.into_body();
    let frame = body.frame().await.unwrap().unwrap();
    let data = frame.into_data().unwrap();
    let sse_text = String::from_utf8_lossy(&data);
    assert!(sse_text.contains("event: endpoint"));
    assert!(sse_text.contains("/mcp/messages?session_id="));

    // Extract session_id from SSE stream output
    let session_prefix = "/mcp/messages?session_id=";
    let start_idx = sse_text.find(session_prefix).unwrap() + session_prefix.len();
    let end_idx = sse_text[start_idx..]
        .find(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
        .map(|offset| start_idx + offset)
        .unwrap_or(sse_text.len());
    let session_id = &sse_text[start_idx..end_idx];
    assert!(!session_id.is_empty());

    // 4. POST /mcp/messages with bogus session_id returns 404
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/messages?session_id=invalid-session")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 5. POST initialize -> returns 202 Accepted, and response is delivered via SSE stream
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/messages?session_id={}", session_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": {},
                            "clientInfo": { "name": "test-client", "version": "1.0" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);

    // Verify SSE stream received the message event!
    let frame2 = body.frame().await.unwrap().unwrap();
    let data2 = frame2.into_data().unwrap();
    let sse_msg = String::from_utf8_lossy(&data2);
    assert!(sse_msg.contains("event: message"));
    assert!(sse_msg.contains(r#""protocolVersion":"2024-11-05""#));
    assert!(sse_msg.contains("rust-mantis-bt"));

    // 6. POST tools/list
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/messages?session_id={}", session_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/list"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);

    let frame3 = body.frame().await.unwrap().unwrap();
    let data3 = frame3.into_data().unwrap();
    let sse_tools = String::from_utf8_lossy(&data3);
    assert!(sse_tools.contains("event: message"));
    assert!(sse_tools.contains("whoami"));
    assert!(sse_tools.contains("create_issue"));

    // 7. POST tools/call for whoami
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/messages?session_id={}", session_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 3,
                        "method": "tools/call",
                        "params": {
                            "name": "whoami",
                            "arguments": {}
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);

    let frame4 = body.frame().await.unwrap().unwrap();
    let data4 = frame4.into_data().unwrap();
    let sse_call = String::from_utf8_lossy(&data4);
    assert!(sse_call.contains("event: message"));
    assert!(sse_call.contains("mcp_reporter"));

    // 8. Test dynamic token revocation on open session
    // Revoke the token in DB
    assert!(db.revoke_api_token(token_id, rep_id).unwrap());
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/messages?session_id={}", session_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 4,
                        "method": "ping"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 9. Header Authorization: Bearer creates another working session
    let (_tok2_id, raw_tok2) = db.create_api_token(rep_id, "Bearer Token").unwrap();
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp/sse")
                .header("authorization", format!("Bearer {}", raw_tok2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    // 10. Drop guard: dropping the SSE stream cleans up the session
    let mut body2 = res2.into_body();
    let frame_end = body2.frame().await.unwrap().unwrap();
    let end_data = frame_end.into_data().unwrap();
    let sse_end_text = String::from_utf8_lossy(&end_data);
    let s_prefix = "/mcp/messages?session_id=";
    let s_start = sse_end_text.find(s_prefix).unwrap() + s_prefix.len();
    let s_end = sse_end_text[s_start..]
        .find(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
        .map(|offset| s_start + offset)
        .unwrap_or(sse_end_text.len());
    let session2_id = sse_end_text[s_start..s_end].to_string();

    // Drop the body stream
    drop(body2);
    // Allow spawned background task to run cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/messages?session_id={}", session2_id))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
