pub mod auth;
pub mod projects;
pub mod users;

use crate::auth::AuthContext;
use crate::db::Db;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
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

async fn root_handler(ctx: AuthContext) -> impl IntoResponse {
    if ctx.user.access_level >= 70 {
        Redirect::to("/manage_proj_page")
    } else {
        Redirect::to("/account_page")
    }
}

pub fn build_router(db: Db) -> Router {
    Router::new()
        .route("/", get(root_handler))
        // Auth routes
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route("/logout", get(auth::logout).post(auth::logout))
        .route("/set_project", post(auth::set_project))
        .route("/account_page", get(auth::account_page))
        .route("/account_update", post(auth::account_update))
        // User management routes
        .route("/manage_user_page", get(users::manage_user_page))
        .route("/manage_user_create_page", get(users::manage_user_create_page))
        .route("/manage_user_create", post(users::manage_user_create))
        .route("/manage_user_edit_page/:id", get(users::manage_user_edit_page))
        .route("/manage_user_update/:id", post(users::manage_user_update))
        .route("/manage_user_delete/:id", post(users::manage_user_delete))
        // Project management routes
        .route("/manage_proj_page", get(projects::manage_proj_page))
        .route("/manage_proj_create_page", get(projects::manage_proj_create_page))
        .route("/manage_proj_create", post(projects::manage_proj_create))
        .route("/manage_proj_edit_page/:id", get(projects::manage_proj_edit_page))
        .route("/manage_proj_update/:id", post(projects::manage_proj_update))
        .route("/manage_proj_delete/:id", post(projects::manage_proj_delete))
        .route("/manage_proj_subproj_add/:id", post(projects::manage_proj_subproj_add))
        .route("/manage_proj_subproj_delete/:id/:child_id", post(projects::manage_proj_subproj_delete))
        .route("/manage_proj_cat_add", post(projects::manage_proj_cat_add))
        .route("/manage_proj_cat_update/:cat_id", post(projects::manage_proj_cat_update))
        .route("/manage_proj_cat_delete/:cat_id", post(projects::manage_proj_cat_delete))
        .route("/manage_proj_ver_add/:id", post(projects::manage_proj_ver_add))
        .route("/manage_proj_ver_update/:ver_id", post(projects::manage_proj_ver_update))
        .route("/manage_proj_ver_delete/:ver_id", post(projects::manage_proj_ver_delete))
        .route("/manage_proj_user_add/:id", post(projects::manage_proj_user_add))
        .route("/manage_proj_user_remove/:id/:user_id", post(projects::manage_proj_user_remove))
        .with_state(db)
}
