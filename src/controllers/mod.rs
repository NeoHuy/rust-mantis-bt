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

pub fn router() -> Router<Db> {
    Router::new()
        .route("/", get(root_handler))
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
    router().with_state(db)
}

#[cfg(test)]
#[path = "controllers_test.rs"]
mod tests;
