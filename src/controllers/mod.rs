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
pub mod tests {
    use super::*;

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool(Some("1")));
        assert!(parse_bool(Some("true")));
        assert!(parse_bool(Some("TRUE")));
        assert!(parse_bool(Some("  True  ")));
        assert!(parse_bool(Some("yes")));
        assert!(parse_bool(Some("YES")));
        assert!(parse_bool(Some("on")));
        assert!(parse_bool(Some("ON")));

        assert!(!parse_bool(Some("0")));
        assert!(!parse_bool(Some("false")));
        assert!(!parse_bool(Some("FALSE")));
        assert!(!parse_bool(Some("no")));
        assert!(!parse_bool(Some("off")));
        assert!(!parse_bool(Some("")));
        assert!(!parse_bool(Some("unknown")));
        assert!(!parse_bool(None));
    }
}
