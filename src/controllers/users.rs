use crate::auth::AuthContext;
use crate::controllers::parse_bool;
use crate::db::Db;
use crate::views;
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

fn require_admin(ctx: &AuthContext) -> Option<Response> {
    if ctx.user.access_level < 90 {
        Some(views::render_access_denied_page(ctx))
    } else {
        None
    }
}

#[derive(Deserialize)]
pub struct ManageUserQuery {
    pub search: Option<String>,
    pub filter_enabled: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateUserForm {
    pub username: String,
    pub realname: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub access_level: i32,
    pub enabled: Option<String>,
    pub protected: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateUserForm {
    pub realname: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub access_level: i32,
    pub enabled: Option<String>,
    pub protected: Option<String>,
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_user_page", get(manage_user_page))
        .route("/manage_user_create_page", get(manage_user_create_page))
        .route("/manage_user_create", post(manage_user_create))
        .route("/manage_user_edit_page/:id", get(manage_user_edit_page))
        .route("/manage_user_update/:id", post(manage_user_update))
        .route("/manage_user_delete/:id", post(manage_user_delete))
}

pub async fn manage_user_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(query): Query<ManageUserQuery>,
) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    let search_trimmed = query.search.as_deref().filter(|s| !s.trim().is_empty());
    let enabled_only = query.filter_enabled.is_some();
    let prefix_trimmed = query
        .prefix
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let users = match db.list_users_filtered(search_trimmed, enabled_only, prefix_trimmed) {
        Ok(list) => list,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Error",
                    Some(&e.to_string()),
                    "<p>Error loading users.</p>",
                ),
            )
                .into_response();
        }
    };

    views::users::render_manage_users_page_with_prefix(
        &ctx,
        &users,
        query.search.as_deref().unwrap_or(""),
        enabled_only,
        prefix_trimmed.unwrap_or(""),
    )
    .into_response()
}

pub async fn manage_user_create_page(ctx: AuthContext) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    views::users::render_user_create_page(&ctx).into_response()
}

pub async fn manage_user_create(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<CreateUserForm>,
) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    let enabled = parse_bool(form.enabled.as_deref());
    let protected = parse_bool(form.protected.as_deref());

    let db_create = db.clone();
    let username = form.username.clone();
    let realname = form.realname.clone().unwrap_or_default();
    let email = form.email.clone().unwrap_or_default();
    let password = form.password.clone();
    let access_level = form.access_level;

    let create_res = tokio::task::spawn_blocking(move || {
        db_create.create_user(
            &username,
            &realname,
            &email,
            &password,
            access_level,
            enabled,
            protected,
        )
    })
    .await
    .unwrap_or_else(|_| Err("Create user task failed".to_string()));

    match create_res {
        Ok(_) => Redirect::to("/manage_user_page").into_response(),
        Err(e) => views::users::render_user_create_error(&ctx, &e).into_response(),
    }
}

pub async fn manage_user_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    let target_user = match db.get_user_by_id(id) {
        Ok(Some(u)) => u,
        Ok(None) => return views::render_not_found_page(&ctx, "User not found"),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Error",
                    Some(&e.to_string()),
                    "<p>Error loading user.</p>",
                ),
            )
                .into_response();
        }
    };

    views::users::render_user_edit_page(&ctx, &target_user).into_response()
}

pub async fn manage_user_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Form(form): Form<UpdateUserForm>,
) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    let enabled = parse_bool(form.enabled.as_deref());
    let protected = parse_bool(form.protected.as_deref());
    let password = form.password.clone().filter(|p| !p.trim().is_empty());

    let db_update = db.clone();
    let realname = form.realname.clone().unwrap_or_default();
    let email = form.email.clone().unwrap_or_default();
    let access_level = form.access_level;

    let update_res = tokio::task::spawn_blocking(move || {
        db_update.update_user(
            id,
            &realname,
            &email,
            password.as_deref(),
            access_level,
            enabled,
            protected,
        )
    })
    .await
    .unwrap_or_else(|_| Err("Update user task failed".to_string()));

    match update_res {
        Ok(_) => Redirect::to("/manage_user_page").into_response(),
        Err(e) => views::users::render_user_update_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_user_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if let Some(res) = require_admin(&ctx) {
        return res;
    }

    match db.delete_user(id) {
        Ok(_) => Redirect::to("/manage_user_page").into_response(),
        Err(e) => views::users::render_user_delete_error(&ctx, &e).into_response(),
    }
}

#[cfg(test)]
#[path = "users_test.rs"]
mod tests;
