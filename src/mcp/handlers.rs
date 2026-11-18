use crate::db::Db;
use crate::mcp::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION};
use crate::mcp::session::SessionManager;
use crate::mcp::tools::{all_tool_definitions, dispatch_tool_call};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct McpState {
    pub db: Db,
    pub session_manager: SessionManager,
}

#[derive(Deserialize, Default)]
pub struct SseQuery {
    pub token: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct MessagesQuery {
    pub session_id: Option<String>,
}

pub async fn mcp_sse_handler(
    State(state): State<McpState>,
    headers: HeaderMap,
    Query(q): Query<SseQuery>,
) -> Response {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .unwrap_or("");

    let raw_token = if let Some(stripped) = auth_header.strip_prefix("Bearer ") {
        stripped.trim()
    } else if let Some(stripped) = auth_header.strip_prefix("bearer ") {
        stripped.trim()
    } else if !auth_header.is_empty() {
        auth_header
    } else if let Some(tok) = q.token.as_deref() {
        tok.trim()
    } else {
        ""
    };

    if raw_token.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "API token required" })),
        )
            .into_response();
    }

    let user = match state.db.authenticate_api_token(raw_token) {
        Ok(Some(u)) if u.enabled == 1 => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid or revoked API token" })),
            )
                .into_response();
        }
    };

    // Lazily cleanup stale sessions older than 30 minutes
    let _ = state
        .session_manager
        .cleanup_stale(std::time::Duration::from_secs(1800))
        .await;

    let (session_id, rx) = state.session_manager.create_session(user).await;

    let initial_event = Event::default()
        .event("endpoint")
        .data(format!("/mcp/messages?session_id={}", session_id));

    let stream = tokio_stream::once(Ok::<Event, Infallible>(initial_event))
        .chain(ReceiverStream::new(rx).map(Ok));

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub async fn mcp_messages_handler(
    State(state): State<McpState>,
    Query(q): Query<MessagesQuery>,
    body: Result<Json<JsonRpcRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let session_id = match q.session_id.as_deref() {
        Some(s) if !s.is_empty() => s,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Session ID required" })),
            )
                .into_response();
        }
    };

    let user = match state.session_manager.get_user(session_id).await {
        Some(u) => u,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Session not found" })),
            )
                .into_response();
        }
    };

    let req = match body {
        Ok(Json(r)) => r,
        Err(_) => {
            let err_resp =
                JsonRpcResponse::error(None, error_codes::PARSE_ERROR, "Invalid JSON-RPC request");
            return (StatusCode::BAD_REQUEST, Json(err_resp)).into_response();
        }
    };

    let is_notification = req.id.is_none();

    let resp = match req.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            req.id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "rust-mantis-bt",
                    "version": "0.1.0"
                }
            }),
        ),

        "notifications/initialized" => {
            return StatusCode::ACCEPTED.into_response();
        }

        "ping" => JsonRpcResponse::success(req.id, json!({})),

        "tools/list" => {
            let tools = all_tool_definitions();
            JsonRpcResponse::success(req.id, json!({ "tools": tools }))
        }

        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = dispatch_tool_call(&state.db, &user, name, arguments);
            JsonRpcResponse::success(req.id, serde_json::to_value(result).unwrap_or_default())
        }

        _ => JsonRpcResponse::error(
            req.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method '{}' not found", req.method),
        ),
    };

    if is_notification {
        return StatusCode::ACCEPTED.into_response();
    }

    // Deliver response to active SSE stream
    if let Ok(json_str) = serde_json::to_string(&resp) {
        let _ = state
            .session_manager
            .send_event(session_id, Event::default().event("message").data(json_str))
            .await;
    }

    (StatusCode::OK, Json(resp)).into_response()
}
