pub mod handlers;
pub mod protocol;
pub mod session;
pub mod tools;

use crate::db::Db;
use axum::routing::{get, post};
use axum::Router;
pub use handlers::McpState;
pub use session::SessionManager;

pub fn router(db: Db, session_manager: SessionManager) -> Router {
    let state = McpState {
        db,
        session_manager,
    };
    Router::new()
        .route("/mcp/sse", get(handlers::mcp_sse_handler))
        .route("/mcp/messages", post(handlers::mcp_messages_handler))
        .with_state(state)
}
