use crate::auth::AuthContext;
use crate::controllers::parse_bool;
use crate::db::Db;
use crate::views;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

fn require_manager(ctx: &AuthContext) -> Option<Response> {
    if ctx.user.access_level < 70 {
        Some(
            (
                StatusCode::FORBIDDEN,
                views::layout::render_layout(
                    "Access Denied",
                    ctx,
                    Some("Access denied. Manager privileges required."),
                    "<p>Access denied. Manager privileges required.</p>",
                ),
            )
                .into_response(),
        )
    } else {
        None
    }
}

#[derive(Deserialize)]
pub struct CreateProjForm {
    pub name: String,
    pub status: Option<i32>,
    pub view_state: Option<i32>,
    pub description: Option<String>,
    pub inherit_global: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProjForm {
    pub name: String,
    pub status: i32,
    pub enabled: Option<String>,
    pub view_state: i32,
    pub description: Option<String>,
    pub inherit_global: Option<String>,
}

#[derive(Deserialize)]
pub struct AddSubprojForm {
    pub child_id: i64,
    pub inherit_parent: Option<String>,
}

#[derive(Deserialize)]
pub struct AddCatForm {
    pub project_id: i64,
    pub name: String,
    pub user_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateCatForm {
    pub project_id: Option<i64>,
    pub name: String,
    pub user_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct DeleteCatForm {
    pub project_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct AddVerForm {
    pub version: String,
    pub description: Option<String>,
    pub released: Option<String>,
    pub obsolete: Option<String>,
    pub date_order: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateVerForm {
    pub project_id: i64,
    pub version: String,
    pub description: Option<String>,
    pub released: Option<String>,
    pub obsolete: Option<String>,
    pub date_order: Option<i64>,
}

#[derive(Deserialize)]
pub struct DeleteVerForm {
    pub project_id: i64,
}

#[derive(Deserialize)]
pub struct AddUserProjForm {
    pub user_id: i64,
    pub access_level: i32,
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_proj_page", get(manage_proj_page))
        .route("/manage_proj_create_page", get(manage_proj_create_page))
        .route("/manage_proj_create", post(manage_proj_create))
        .route("/manage_proj_edit_page/:id", get(manage_proj_edit_page))
        .route("/manage_proj_update/:id", post(manage_proj_update))
        .route("/manage_proj_delete/:id", post(manage_proj_delete))
        .route(
            "/manage_proj_subproj_add/:id",
            post(manage_proj_subproj_add),
        )
        .route(
            "/manage_proj_subproj_delete/:id/:child_id",
            post(manage_proj_subproj_delete),
        )
        .route("/manage_proj_cat_add", post(manage_proj_cat_add))
        .route(
            "/manage_proj_cat_update/:cat_id",
            post(manage_proj_cat_update),
        )
        .route(
            "/manage_proj_cat_delete/:cat_id",
            post(manage_proj_cat_delete),
        )
        .route("/manage_proj_ver_add/:id", post(manage_proj_ver_add))
        .route(
            "/manage_proj_ver_update/:ver_id",
            post(manage_proj_ver_update),
        )
        .route(
            "/manage_proj_ver_delete/:ver_id",
            post(manage_proj_ver_delete),
        )
        .route("/manage_proj_user_add/:id", post(manage_proj_user_add))
        .route(
            "/manage_proj_user_remove/:id/:user_id",
            post(manage_proj_user_remove),
        )
}

pub async fn manage_proj_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let projects_hierarchy = match db.list_visible_projects(&ctx.user) {
        Ok(list) => list,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Error",
                    Some(&e.to_string()),
                    "<p>Error loading projects.</p>",
                ),
            )
                .into_response();
        }
    };

    let global_cats = db.list_effective_categories(0).unwrap_or_default();
    let mut resolved_cats = Vec::new();
    for c in global_cats {
        let assignee_name = if c.user_id > 0 {
            db.get_user_by_id(c.user_id)
                .ok()
                .flatten()
                .map(|u| u.username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            "-".to_string()
        };
        resolved_cats.push((c, assignee_name));
    }

    let users = db.list_users(None, true).unwrap_or_default();

    views::projects::render_manage_proj_page(&ctx, &projects_hierarchy, &resolved_cats, &users)
        .into_response()
}

pub async fn manage_proj_create_page(ctx: AuthContext) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    views::projects::render_proj_create_page(&ctx).into_response()
}

pub async fn manage_proj_create(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<CreateProjForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let inherit_global = parse_bool(form.inherit_global.as_deref());
    let status = form.status.unwrap_or(10);
    let view_state = form.view_state.unwrap_or(10);
    let description = form.description.unwrap_or_default();

    match db.create_project(&form.name, status, view_state, &description, inherit_global) {
        Ok(_) => Redirect::to("/manage_proj_page").into_response(),
        Err(e) => views::projects::render_proj_create_error(&ctx, &e).into_response(),
    }
}

pub async fn manage_proj_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let project = match db.get_project_by_id(id) {
        Ok(Some(p)) => p,
        Ok(None) => return views::render_not_found_page(&ctx, "Project not found"),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Error",
                    Some(&e.to_string()),
                    "<p>Error loading project.</p>",
                ),
            )
                .into_response();
        }
    };

    let subprojects = db.list_subprojects(id).unwrap_or_default();

    let all_projs = db.list_all_projects().unwrap_or_default();
    let candidate_subprojs: Vec<_> = all_projs
        .into_iter()
        .filter(|p| p.id != id && !subprojects.iter().any(|(c, _)| c.id == p.id))
        .collect();

    let direct_cats = db.list_direct_categories(id).unwrap_or_default();
    let mut resolved_cats = Vec::new();
    for c in direct_cats {
        let assignee_name = if c.user_id > 0 {
            db.get_user_by_id(c.user_id)
                .ok()
                .flatten()
                .map(|u| u.username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            "-".to_string()
        };
        resolved_cats.push((c, assignee_name));
    }

    let versions = db.list_versions(id).unwrap_or_default();
    let proj_users = db.list_project_users(id).unwrap_or_default();
    let all_users = db.list_users(None, true).unwrap_or_default();

    views::projects::render_proj_edit_page(
        &ctx,
        &project,
        &subprojects,
        &candidate_subprojs,
        &resolved_cats,
        &versions,
        &proj_users,
        &all_users,
    )
    .into_response()
}

pub async fn manage_proj_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Form(form): Form<UpdateProjForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let enabled = parse_bool(form.enabled.as_deref());
    let inherit_global = parse_bool(form.inherit_global.as_deref());

    match db.update_project(
        id,
        &form.name,
        form.status,
        enabled,
        form.view_state,
        form.description.as_deref().unwrap_or(""),
        inherit_global,
    ) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_proj_update_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_proj_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    match db.delete_project(id) {
        Ok(_) => Redirect::to("/manage_proj_page").into_response(),
        Err(e) => views::projects::render_proj_delete_error(&ctx, &e).into_response(),
    }
}

pub async fn manage_proj_subproj_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Form(form): Form<AddSubprojForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let inherit = parse_bool(form.inherit_parent.as_deref());
    match db.add_subproject(id, form.child_id, inherit) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_subproj_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_proj_subproj_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((id, child_id)): Path<(i64, i64)>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    match db.remove_subproject(id, child_id) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_subproj_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_proj_cat_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<AddCatForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let target_url = if form.project_id > 0 {
        format!("/manage_proj_edit_page/{}", form.project_id)
    } else {
        "/manage_proj_page".to_string()
    };

    match db.add_category(form.project_id, &form.name, form.user_id.unwrap_or(0)) {
        Ok(_) => Redirect::to(&target_url).into_response(),
        Err(e) => views::projects::render_category_error(&ctx, &target_url, &e).into_response(),
    }
}

pub async fn manage_proj_cat_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(cat_id): Path<i64>,
    Form(form): Form<UpdateCatForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let pid = form.project_id.unwrap_or(0);
    let target_url = if pid > 0 {
        format!("/manage_proj_edit_page/{}", pid)
    } else {
        "/manage_proj_page".to_string()
    };

    match db.update_category(cat_id, &form.name, form.user_id.unwrap_or(0)) {
        Ok(_) => Redirect::to(&target_url).into_response(),
        Err(e) => views::projects::render_category_error(&ctx, &target_url, &e).into_response(),
    }
}

pub async fn manage_proj_cat_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(cat_id): Path<i64>,
    Form(form): Form<DeleteCatForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let pid = form.project_id.unwrap_or(0);
    let target_url = if pid > 0 {
        format!("/manage_proj_edit_page/{}", pid)
    } else {
        "/manage_proj_page".to_string()
    };

    match db.delete_category(cat_id) {
        Ok(_) => Redirect::to(&target_url).into_response(),
        Err(e) => views::projects::render_category_error(&ctx, &target_url, &e).into_response(),
    }
}

pub async fn manage_proj_ver_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Form(form): Form<AddVerForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let released = parse_bool(form.released.as_deref());
    let obsolete = parse_bool(form.obsolete.as_deref());
    let desc = form.description.as_deref().unwrap_or("");

    match db.add_version(id, &form.version, desc, released, obsolete) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_version_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_proj_ver_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(ver_id): Path<i64>,
    Form(form): Form<UpdateVerForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let released = parse_bool(form.released.as_deref());
    let obsolete = parse_bool(form.obsolete.as_deref());
    let desc = form.description.as_deref().unwrap_or("");

    match db.update_version(ver_id, &form.version, desc, released, obsolete) {
        Ok(_) => {
            Redirect::to(&format!("/manage_proj_edit_page/{}", form.project_id)).into_response()
        }
        Err(e) => views::projects::render_version_error(&ctx, form.project_id, &e).into_response(),
    }
}

pub async fn manage_proj_ver_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(ver_id): Path<i64>,
    Form(form): Form<DeleteVerForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    match db.delete_version(ver_id) {
        Ok(_) => {
            Redirect::to(&format!("/manage_proj_edit_page/{}", form.project_id)).into_response()
        }
        Err(e) => views::projects::render_version_error(&ctx, form.project_id, &e).into_response(),
    }
}

pub async fn manage_proj_user_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Form(form): Form<AddUserProjForm>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    match db.set_project_user(id, form.user_id, form.access_level) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_proj_user_error(&ctx, id, &e).into_response(),
    }
}

pub async fn manage_proj_user_remove(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((id, user_id)): Path<(i64, i64)>,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    match db.remove_project_user(id, user_id) {
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", id)).into_response(),
        Err(e) => views::projects::render_proj_user_error(&ctx, id, &e).into_response(),
    }
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod tests;
