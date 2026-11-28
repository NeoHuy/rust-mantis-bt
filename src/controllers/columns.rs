use crate::auth::AuthContext;
use crate::controllers::{sanitize_error_message, url_encode};
use crate::db::Db;
use crate::models::{parse_columns_input, ColumnContext};
use crate::views;
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct ColumnsPageQuery {
    pub project_id: Option<String>,
    pub msg: Option<String>,
    pub err: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct UpdateColumnsForm {
    pub project_id: Option<i64>,
    #[serde(default)]
    pub view_issues_columns: String,
    #[serde(default)]
    pub print_issues_columns: String,
    #[serde(default)]
    pub csv_columns: String,
    #[serde(default)]
    pub excel_columns: String,
}

#[derive(Deserialize)]
pub struct ResetColumnsForm {
    pub project_id: Option<i64>,
}

fn forbidden(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Access Denied: {}</div>"#,
        views::html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        views::layout::render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

pub async fn account_manage_columns_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<ColumnsPageQuery>,
) -> Response {
    let parsed_pid = q
        .project_id
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok());
    let project_id = parsed_pid
        .filter(|&p| p >= 0)
        .unwrap_or(ctx.active_project_id);

    if project_id > 0
        && db
            .effective_access_level(&ctx.user, project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this project.");
    }

    let accessible_cfs = db
        .list_accessible_custom_fields_for_user(&ctx.user, project_id)
        .unwrap_or_default();
    let accessible_cf_ids: std::collections::HashSet<i64> =
        accessible_cfs.iter().map(|c| c.id).collect();
    let view_cols = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            project_id,
            ColumnContext::View,
            &accessible_cf_ids,
        )
        .unwrap_or_default();
    let print_cols = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            project_id,
            ColumnContext::Print,
            &accessible_cf_ids,
        )
        .unwrap_or_default();
    let csv_cols = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            project_id,
            ColumnContext::Csv,
            &accessible_cf_ids,
        )
        .unwrap_or_default();
    let excel_cols = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            project_id,
            ColumnContext::Excel,
            &accessible_cf_ids,
        )
        .unwrap_or_default();

    views::columns::render_manage_columns_page(
        &ctx,
        project_id,
        &accessible_cfs,
        &view_cols,
        &print_cols,
        &csv_cols,
        &excel_cols,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn account_manage_columns_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<UpdateColumnsForm>,
) -> Response {
    let project_id = form.project_id.filter(|&p| p >= 0).unwrap_or(0);
    if project_id > 0
        && db
            .effective_access_level(&ctx.user, project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this project.");
    }

    let entries = vec![
        (
            ColumnContext::View,
            parse_columns_input(&form.view_issues_columns),
        ),
        (
            ColumnContext::Print,
            parse_columns_input(&form.print_issues_columns),
        ),
        (ColumnContext::Csv, parse_columns_input(&form.csv_columns)),
        (
            ColumnContext::Excel,
            parse_columns_input(&form.excel_columns),
        ),
    ];

    if let Err(e) = db.set_user_columns_batch(&ctx.user, project_id, &entries) {
        return Redirect::to(&format!(
            "/account_manage_columns_page?project_id={}&err={}",
            project_id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    Redirect::to(&format!(
        "/account_manage_columns_page?project_id={}&msg=Columns+updated+successfully",
        project_id
    ))
    .into_response()
}

pub async fn account_manage_columns_reset(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<ResetColumnsForm>,
) -> Response {
    let project_id = form.project_id.filter(|&p| p >= 0).unwrap_or(0);
    if project_id > 0
        && db
            .effective_access_level(&ctx.user, project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this project.");
    }

    match db.reset_user_columns(ctx.user.id, project_id) {
        Ok(()) => Redirect::to(&format!(
            "/account_manage_columns_page?project_id={}&msg=Columns+reset+to+defaults",
            project_id
        ))
        .into_response(),
        Err(e) => Redirect::to(&format!(
            "/account_manage_columns_page?project_id={}&err={}",
            project_id,
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response(),
    }
}

pub fn router() -> Router<Db> {
    Router::new()
        .route(
            "/account_manage_columns_page",
            get(account_manage_columns_page),
        )
        .route(
            "/account_manage_columns_update",
            post(account_manage_columns_update),
        )
        .route(
            "/account_manage_columns_reset",
            post(account_manage_columns_reset),
        )
}

#[cfg(test)]
#[path = "columns_test.rs"]
mod tests;
