pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
pub mod filters;
pub mod jira;
pub mod planning;
pub mod projects;
pub mod tags;
pub mod users;

use crate::auth::AuthContext;
use crate::db::Db;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;

pub fn parse_bool(opt: Option<&str>) -> bool {
    match opt {
        Some(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "on" || v == "yes"
        }
        None => false,
    }
}

pub async fn root_handler(ctx: AuthContext) -> impl IntoResponse {
    if ctx.user.access_level >= 70 {
        Redirect::to("/manage_proj_page")
    } else {
        Redirect::to("/account_page")
    }
}

async fn favicon_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../../assets/favicon.ico"),
    )
}

async fn logo_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../assets/mantis_logo_notext.png"),
    )
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/", get(root_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/images/mantis_logo.png", get(logo_handler))
        .merge(auth::router())
        .merge(users::router())
        .merge(projects::router())
        .merge(bugs::router())
        .merge(tags::router())
        .merge(custom_fields::router())
        .merge(filters::router())
        .merge(planning::router())
        .merge(api::router())
        .merge(jira::router())
}

pub fn build_router(db: Db) -> Router {
    let mcp_router = crate::mcp::router(db.clone(), crate::mcp::SessionManager::new());
    router().with_state(db).merge(mcp_router)
}

#[cfg(test)]
#[path = "controllers_test.rs"]
mod tests;
