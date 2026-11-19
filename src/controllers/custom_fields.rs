use crate::auth::AuthContext;
use crate::controllers::parse_bool;
use crate::db::Db;
use crate::views;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
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

pub async fn manage_custom_field_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    let fields = db.list_custom_fields().unwrap_or_default();
    let fields_with_counts = fields
        .into_iter()
        .map(|cf| {
            let linked_count = db
                .list_projects_for_custom_field(cf.id)
                .map(|v| v.len())
                .unwrap_or(0);
            (cf, linked_count)
        })
        .collect::<Vec<_>>();

    views::custom_fields::render_manage_custom_field_page(
        &ctx,
        &fields_with_counts,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct CustomFieldForm {
    pub name: String,
    pub field_type: Option<i32>,
    pub possible_values: Option<String>,
    pub default_value: Option<String>,
    pub valid_regexp: Option<String>,
    pub length_min: Option<i32>,
    pub length_max: Option<i32>,
    pub require_report: Option<String>,
    pub require_update: Option<String>,
    pub display_report: Option<String>,
    pub display_update: Option<String>,
}

pub async fn manage_custom_field_create(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<CustomFieldForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    match db.create_custom_field(
        &form.name,
        form.field_type.unwrap_or(0),
        form.possible_values.as_deref().unwrap_or(""),
        form.default_value.as_deref().unwrap_or(""),
        form.valid_regexp.as_deref().unwrap_or(""),
        form.length_min.unwrap_or(0),
        form.length_max.unwrap_or(0),
        parse_bool(form.require_report.as_deref()),
        parse_bool(form.require_update.as_deref()),
        parse_bool(form.display_report.as_deref()),
        parse_bool(form.display_update.as_deref()),
    ) {
        Ok(id) => Redirect::to(&format!(
            "/manage_custom_field_edit_page/{}?msg=Custom+field+created",
            id
        ))
        .into_response(),
        Err(_) => Redirect::to("/manage_custom_field_page?err=Failed+to+create+custom+field")
            .into_response(),
    }
}

pub async fn manage_custom_field_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    let cf = match db.get_custom_field_by_id(id) {
        Ok(Some(c)) => c,
        _ => return Redirect::to("/manage_custom_field_page?err=Not+found").into_response(),
    };

    let linked = db.list_projects_for_custom_field(id).unwrap_or_default();
    views::custom_fields::render_manage_custom_field_edit_page(
        &ctx,
        &cf,
        &linked,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn manage_custom_field_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<CustomFieldForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.update_custom_field(
        id,
        &form.name,
        form.field_type.unwrap_or(0),
        form.possible_values.as_deref().unwrap_or(""),
        form.default_value.as_deref().unwrap_or(""),
        form.valid_regexp.as_deref().unwrap_or(""),
        form.length_min.unwrap_or(0),
        form.length_max.unwrap_or(0),
        parse_bool(form.require_report.as_deref()),
        parse_bool(form.require_update.as_deref()),
        parse_bool(form.display_report.as_deref()),
        parse_bool(form.display_update.as_deref()),
    );
    Redirect::to(&format!(
        "/manage_custom_field_edit_page/{}?msg=Updated",
        id
    ))
    .into_response()
}

pub async fn manage_custom_field_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.delete_custom_field(id);
    Redirect::to("/manage_custom_field_page?msg=Custom+field+deleted").into_response()
}

#[derive(Deserialize)]
pub struct LinkProjectForm {
    pub project_id: i64,
    pub sequence: Option<i32>,
}

pub async fn manage_custom_field_proj_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<LinkProjectForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.link_custom_field_to_project(id, form.project_id, form.sequence.unwrap_or(0));
    Redirect::to(&format!("/manage_custom_field_edit_page/{}?msg=Linked", id)).into_response()
}

pub async fn manage_custom_field_proj_remove(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((id, project_id)): Path<(i64, i64)>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.unlink_custom_field_from_project(id, project_id);
    Redirect::to(&format!(
        "/manage_custom_field_edit_page/{}?msg=Unlinked",
        id
    ))
    .into_response()
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_custom_field_page", get(manage_custom_field_page))
        .route(
            "/manage_custom_field_create",
            post(manage_custom_field_create),
        )
        .route(
            "/manage_custom_field_edit_page/:id",
            get(manage_custom_field_edit_page),
        )
        .route(
            "/manage_custom_field_update/:id",
            post(manage_custom_field_update),
        )
        .route(
            "/manage_custom_field_delete/:id",
            post(manage_custom_field_delete),
        )
        .route(
            "/manage_custom_field_proj_add/:id",
            post(manage_custom_field_proj_add),
        )
        .route(
            "/manage_custom_field_proj_remove/:id/:project_id",
            post(manage_custom_field_proj_remove),
        )
}

#[cfg(test)]
#[path = "custom_fields_test.rs"]
mod tests;
