use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{AccessLevel, ProjectStatus, ViewState};
use crate::ui::{html_escape, render_layout};
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use super::parse_bool;

fn require_manager(ctx: &AuthContext) -> Option<Response> {
    if ctx.user.access_level < 70 {
        Some(
            (
                StatusCode::FORBIDDEN,
                render_layout(
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

pub async fn manage_proj_page(
    State(db): State<Db>,
    ctx: AuthContext,
) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let projects_hierarchy = match db.list_visible_projects(&ctx.user) {
        Ok(list) => list,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                render_layout("Error", &ctx, Some(&e.to_string()), "<p>Error loading projects.</p>"),
            )
                .into_response();
        }
    };

    let mut proj_rows = String::new();
    for (idx, (p, depth)) in projects_hierarchy.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let mut indent_str = String::new();
        for _ in 0..*depth {
            indent_str.push_str("&nbsp;&nbsp;&nbsp;&nbsp;");
        }
        let prefix = if *depth > 0 { "» " } else { "" };
        let status_str = ProjectStatus::from_i32(p.status)
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let view_str = ViewState::from_i32(p.view_state)
            .map(|v| v.as_str())
            .unwrap_or("unknown");
        let enabled_badge = if p.enabled == 1 {
            r#"<span class="badge badge-success">enabled</span>"#
        } else {
            r#"<span class="badge badge-danger">disabled</span>"#
        };

        proj_rows.push_str(&format!(
            r#"<tr class="{}">
                <td>{}{}<a href="/manage_proj_edit_page/{}"><strong>{}</strong></a></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <a href="/manage_proj_edit_page/{}" class="btn btn-secondary btn-sm">Edit</a>
                </td>
            </tr>"#,
            row_class,
            indent_str,
            prefix,
            p.id,
            html_escape(&p.name),
            html_escape(status_str),
            enabled_badge,
            html_escape(view_str),
            html_escape(&p.description),
            p.id
        ));
    }

    // Global categories
    let global_cats = db.list_effective_categories(0).unwrap_or_default();
    let mut cat_rows = String::new();
    for (idx, c) in global_cats.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let assignee_name = if c.user_id > 0 {
            db.get_user_by_id(c.user_id)
                .ok()
                .flatten()
                .map(|u| u.username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            "-".to_string()
        };

        cat_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_cat_delete/{}" method="POST" style="display:inline;" onsubmit="return confirm('Delete category?');">
                        <input type="hidden" name="project_id" value="0" />
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            html_escape(&c.name),
            html_escape(&assignee_name),
            c.id
        ));
    }

    // User options for global category add
    let users = db.list_users(None, true).unwrap_or_default();
    let mut user_opts = String::from(r#"<option value="0">- None -</option>"#);
    for u in &users {
        user_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            u.id,
            html_escape(&u.username)
        ));
    }

    let content = format!(
        r#"
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:14px;">
            <h2 class="section-title" style="margin:0; border:none;">Projects</h2>
            <a href="/manage_proj_create_page" class="btn">Create New Project</a>
        </div>

        <table class="data-table" style="margin-bottom:24px;">
            <thead>
                <tr>
                    <th>Name</th>
                    <th>Status</th>
                    <th>Enabled</th>
                    <th>View Status</th>
                    <th>Description</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>

        <div class="panel">
            <div class="panel-header">Global Categories</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Category</th>
                            <th>Assign To</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>

                <form method="POST" action="/manage_proj_cat_add" style="margin-top:14px; display:flex; align-items:center; gap:8px;">
                    <input type="hidden" name="project_id" value="0" />
                    <input type="text" name="name" placeholder="New Global Category" required style="width:200px;" />
                    <select name="user_id" style="width:180px;">
                        {}
                    </select>
                    <button type="submit" class="btn">Add Category</button>
                </form>
            </div>
        </div>
        "#,
        proj_rows,
        cat_rows,
        user_opts
    );

    render_layout("Manage Projects", &ctx, None, &content).into_response()
}

pub async fn manage_proj_create_page(ctx: AuthContext) -> Response {
    if let Some(res) = require_manager(&ctx) {
        return res;
    }

    let content = r#"
    <div class="panel">
        <div class="panel-header">Create Project</div>
        <div class="panel-body">
            <form method="POST" action="/manage_proj_create">
                <table class="form-table">
                    <tr>
                        <td class="label-col">Project Name *</td>
                        <td class="input-col"><input type="text" name="name" required autofocus /></td>
                    </tr>
                    <tr>
                        <td class="label-col">Status</td>
                        <td class="input-col">
                            <select name="status">
                                <option value="10" selected>development (10)</option>
                                <option value="30">release (30)</option>
                                <option value="50">stable (50)</option>
                                <option value="70">obsolete (70)</option>
                            </select>
                        </td>
                    </tr>
                    <tr>
                        <td class="label-col">View Status</td>
                        <td class="input-col">
                            <select name="view_state">
                                <option value="10" selected>public (10)</option>
                                <option value="50">private (50)</option>
                            </select>
                        </td>
                    </tr>
                    <tr>
                        <td class="label-col">Inherit Global Categories</td>
                        <td class="input-col">
                            <label><input type="checkbox" name="inherit_global" value="1" checked /> Inherit global categories</label>
                        </td>
                    </tr>
                    <tr>
                        <td class="label-col">Description</td>
                        <td class="input-col"><textarea name="description"></textarea></td>
                    </tr>
                </table>
                <div style="margin-top: 14px;">
                    <button type="submit" class="btn">Add Project</button>
                    <a href="/manage_proj_page" class="btn btn-secondary" style="margin-left:8px;">Cancel</a>
                </div>
            </form>
        </div>
    </div>
    "#;

    render_layout("Create Project", &ctx, None, content).into_response()
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to create project: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_create_page" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e)
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Create Project Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                render_layout("Project Not Found", &ctx, Some("Project not found"), "<p>Project not found.</p>"),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                render_layout("Error", &ctx, Some(&e.to_string()), "<p>Error loading project.</p>"),
            )
                .into_response();
        }
    };

    // Subprojects
    let subprojects = db.list_subprojects(id).unwrap_or_default();
    let mut subproj_rows = String::new();
    for (idx, (child, inherit_parent)) in subprojects.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let inherit_str = if *inherit_parent { "yes" } else { "no" };
        subproj_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><a href="/manage_proj_edit_page/{}">{}</a></td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_subproj_delete/{}/{}" method="POST" style="display:inline;" onsubmit="return confirm('Remove subproject relationship?');">
                        <button type="submit" class="btn btn-danger btn-sm">Unlink</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            child.id,
            html_escape(&child.name),
            inherit_str,
            id,
            child.id
        ));
    }

    // Potential subprojects (all projects except self and existing children)
    let all_projs = db.list_all_projects().unwrap_or_default();
    let mut candidate_subproj_opts = String::new();
    for p in &all_projs {
        if p.id != id && !subprojects.iter().any(|(c, _)| c.id == p.id) {
            candidate_subproj_opts.push_str(&format!(
                r#"<option value="{}">{}</option>"#,
                p.id,
                html_escape(&p.name)
            ));
        }
    }

    // Categories
    let direct_cats = db.list_direct_categories(id).unwrap_or_default();
    let mut cat_rows = String::new();
    for (idx, c) in direct_cats.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let assignee_name = if c.user_id > 0 {
            db.get_user_by_id(c.user_id)
                .ok()
                .flatten()
                .map(|u| u.username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            "-".to_string()
        };

        cat_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_cat_delete/{}" method="POST" style="display:inline;" onsubmit="return confirm('Delete category?');">
                        <input type="hidden" name="project_id" value="{}" />
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            html_escape(&c.name),
            html_escape(&assignee_name),
            c.id,
            id
        ));
    }

    // Versions
    let versions = db.list_versions(id).unwrap_or_default();
    let mut ver_rows = String::new();
    for (idx, v) in versions.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let rel_badge = if v.released == 1 { "yes" } else { "no" };
        let obs_badge = if v.obsolete == 1 { "yes" } else { "no" };
        ver_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_ver_delete/{}" method="POST" style="display:inline;" onsubmit="return confirm('Delete version?');">
                        <input type="hidden" name="project_id" value="{}" />
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            html_escape(&v.version),
            html_escape(&v.description),
            rel_badge,
            obs_badge,
            v.id,
            id
        ));
    }

    // Project Users
    let proj_users = db.list_project_users(id).unwrap_or_default();
    let mut user_rows = String::new();
    for (idx, (u, access_level)) in proj_users.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let lvl_name = AccessLevel::from_i32(*access_level)
            .map(|a| a.as_str())
            .unwrap_or("unknown");
        user_rows.push_str(&format!(
            r#"<tr class="{}">
                <td>{}</td>
                <td>{} ({})</td>
                <td>
                    <form action="/manage_proj_user_remove/{}/{}" method="POST" style="display:inline;" onsubmit="return confirm('Remove user from project?');">
                        <button type="submit" class="btn btn-danger btn-sm">Remove</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            html_escape(&u.username),
            html_escape(lvl_name),
            access_level,
            id,
            u.id
        ));
    }

    // All users for select
    let all_users = db.list_users(None, true).unwrap_or_default();
    let mut user_select_opts = String::new();
    for u in &all_users {
        if !proj_users.iter().any(|(pu, _)| pu.id == u.id) {
            user_select_opts.push_str(&format!(
                r#"<option value="{}">{} ({})</option>"#,
                u.id,
                html_escape(&u.username),
                html_escape(&u.realname)
            ));
        }
    }

    let enabled_checked = if project.enabled == 1 { "checked" } else { "" };
    let inherit_global_checked = if project.inherit_global == 1 { "checked" } else { "" };

    let content = format!(
        r#"
        <div class="panel">
            <div class="panel-header">Edit Project: {}</div>
            <div class="panel-body">
                <form method="POST" action="/manage_proj_update/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Project Name *</td>
                            <td class="input-col"><input type="text" name="name" value="{}" required /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Status</td>
                            <td class="input-col">
                                <select name="status">
                                    <option value="10" {}>development (10)</option>
                                    <option value="30" {}>release (30)</option>
                                    <option value="50" {}>stable (50)</option>
                                    <option value="70" {}>obsolete (70)</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Enabled</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="enabled" value="1" {} /> Enabled</label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" {}>public (10)</option>
                                    <option value="50" {}>private (50)</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Inherit Global Categories</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="inherit_global" value="1" {} /> Inherit global categories</label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description">{}</textarea></td>
                        </tr>
                    </table>
                    <div style="margin-top: 14px;">
                        <button type="submit" class="btn">Update Project</button>
                        <a href="/manage_proj_page" class="btn btn-secondary" style="margin-left:8px;">Back to Projects</a>
                    </div>
                </form>
                <div style="margin-top: 12px; border-top: 1px solid #e0e6ed; padding-top: 12px;">
                    <form action="/manage_proj_delete/{}" method="POST" style="display:inline;" onsubmit="return confirm('Are you sure you want to delete this project? All associated data will be removed.');">
                        <button type="submit" class="btn btn-danger">Delete Project</button>
                    </form>
                </div>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Subprojects</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Subproject Name</th>
                            <th>Inherit Parent Categories</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>

                <form method="POST" action="/manage_proj_subproj_add/{}" style="margin-top:14px; display:flex; align-items:center; gap:8px;">
                    <select name="child_id" required style="width:250px;">
                        <option value="">- Select Subproject to Link -</option>
                        {}
                    </select>
                    <label style="display:flex; align-items:center; cursor:pointer;">
                        <input type="checkbox" name="inherit_parent" value="1" checked /> Inherit Parent Categories
                    </label>
                    <button type="submit" class="btn">Link Subproject</button>
                </form>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Categories</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Category</th>
                            <th>Assign To</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>

                <form method="POST" action="/manage_proj_cat_add" style="margin-top:14px; display:flex; align-items:center; gap:8px;">
                    <input type="hidden" name="project_id" value="{}" />
                    <input type="text" name="name" placeholder="New Category Name" required style="width:200px;" />
                    <select name="user_id" style="width:180px;">
                        <option value="0">- None -</option>
                        {}
                    </select>
                    <button type="submit" class="btn">Add Category</button>
                </form>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Versions</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Version</th>
                            <th>Description</th>
                            <th>Released</th>
                            <th>Obsolete</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>

                <form method="POST" action="/manage_proj_ver_add/{}" style="margin-top:14px; display:flex; align-items:center; gap:8px; flex-wrap:wrap;">
                    <input type="text" name="version" placeholder="Version (e.g. 1.0.0)" required style="width:160px;" />
                    <input type="text" name="description" placeholder="Description" style="width:220px;" />
                    <label style="display:flex; align-items:center;"><input type="checkbox" name="released" value="1" checked /> Released</label>
                    <label style="display:flex; align-items:center;"><input type="checkbox" name="obsolete" value="1" /> Obsolete</label>
                    <button type="submit" class="btn">Add Version</button>
                </form>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">User Access & Overrides</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Username</th>
                            <th>Project Access Level</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>

                <form method="POST" action="/manage_proj_user_add/{}" style="margin-top:14px; display:flex; align-items:center; gap:8px;">
                    <select name="user_id" required style="width:220px;">
                        <option value="">- Select User -</option>
                        {}
                    </select>
                    <select name="access_level" required style="width:160px;">
                        <option value="10">viewer (10)</option>
                        <option value="25" selected>reporter (25)</option>
                        <option value="40">updater (40)</option>
                        <option value="55">developer (55)</option>
                        <option value="70">manager (70)</option>
                        <option value="90">administrator (90)</option>
                    </select>
                    <button type="submit" class="btn">Add User</button>
                </form>
            </div>
        </div>
        "#,
        html_escape(&project.name),
        project.id,
        html_escape(&project.name),
        if project.status == 10 { "selected" } else { "" },
        if project.status == 30 { "selected" } else { "" },
        if project.status == 50 { "selected" } else { "" },
        if project.status == 70 { "selected" } else { "" },
        enabled_checked,
        if project.view_state == 10 { "selected" } else { "" },
        if project.view_state == 50 { "selected" } else { "" },
        inherit_global_checked,
        html_escape(&project.description),
        project.id,
        subproj_rows,
        project.id,
        candidate_subproj_opts,
        cat_rows,
        project.id,
        {
            let mut opts = String::new();
            for u in &all_users {
                opts.push_str(&format!(r#"<option value="{}">{}</option>"#, u.id, html_escape(&u.username)));
            }
            opts
        },
        ver_rows,
        project.id,
        user_rows,
        project.id,
        user_select_opts
    );

    render_layout("Edit Project", &ctx, None, &content).into_response()
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to update project: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Update Project Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to delete project: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_page" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e)
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Delete Project Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to add subproject: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Subproject Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to remove subproject: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Subproject Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to add category: {}</p>
                <p style="margin-top:12px;"><a href="{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                target_url
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Category Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to update category: {}</p>
                <p style="margin-top:12px;"><a href="{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                target_url
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Category Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to delete category: {}</p>
                <p style="margin-top:12px;"><a href="{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                target_url
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Category Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to add version: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Version Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", form.project_id)).into_response(),
        Err(e) => {
            let content = format!(
                r#"<p>Failed to update version: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                form.project_id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Version Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Ok(_) => Redirect::to(&format!("/manage_proj_edit_page/{}", form.project_id)).into_response(),
        Err(e) => {
            let content = format!(
                r#"<p>Failed to delete version: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                form.project_id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Version Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to add user to project: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Project User Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
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
        Err(e) => {
            let content = format!(
                r#"<p>Failed to remove user from project: {}</p>
                <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
                html_escape(&e),
                id
            );
            (
                StatusCode::BAD_REQUEST,
                render_layout("Project User Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
    }
}
