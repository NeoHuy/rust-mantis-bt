use crate::auth::AuthContext;
use crate::models::{
    AccessLevel, Category, Project, ProjectStatus, ProjectVersion, User, ViewState,
};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::http::StatusCode;
use axum::response::Html;

pub fn render_manage_proj_page(
    ctx: &AuthContext,
    projects_hierarchy: &[(Project, usize)],
    global_cats: &[(Category, String)],
    users: &[User],
) -> Html<String> {
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

    let mut cat_rows = String::new();
    for (idx, (c, assignee_name)) in global_cats.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };

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
            html_escape(assignee_name),
            c.id
        ));
    }

    let mut user_opts = String::from(r#"<option value="0">- None -</option>"#);
    for u in users {
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
        proj_rows, cat_rows, user_opts
    );

    render_layout("Manage Projects", ctx, None, &content)
}

pub fn render_proj_create_page(ctx: &AuthContext) -> Html<String> {
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

    render_layout("Create Project", ctx, None, content)
}

#[allow(clippy::too_many_arguments)]
pub fn render_proj_edit_page(
    ctx: &AuthContext,
    project: &Project,
    subprojects: &[(Project, bool)],
    candidate_subprojs: &[Project],
    direct_cats: &[(Category, String)],
    versions: &[ProjectVersion],
    proj_users: &[(User, i32)],
    all_users: &[User],
) -> Html<String> {
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
            project.id,
            child.id
        ));
    }

    let mut candidate_subproj_opts = String::new();
    for p in candidate_subprojs {
        candidate_subproj_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            p.id,
            html_escape(&p.name)
        ));
    }

    let mut cat_rows = String::new();
    for (idx, (c, assignee_name)) in direct_cats.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };

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
            html_escape(assignee_name),
            c.id,
            project.id
        ));
    }

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
            project.id
        ));
    }

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
            project.id,
            u.id
        ));
    }

    let mut user_select_opts = String::new();
    for u in all_users {
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
    let inherit_global_checked = if project.inherit_global == 1 {
        "checked"
    } else {
        ""
    };

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
        if project.view_state == 10 {
            "selected"
        } else {
            ""
        },
        if project.view_state == 50 {
            "selected"
        } else {
            ""
        },
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
            for u in all_users {
                opts.push_str(&format!(
                    r#"<option value="{}">{}</option>"#,
                    u.id,
                    html_escape(&u.username)
                ));
            }
            opts
        },
        ver_rows,
        project.id,
        user_rows,
        project.id,
        user_select_opts
    );

    render_layout("Edit Project", ctx, None, &content)
}

pub fn render_proj_create_error(ctx: &AuthContext, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to create project: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_create_page" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Create Project Error", ctx, Some(err), &content),
    )
}

pub fn render_proj_update_error(
    ctx: &AuthContext,
    id: i64,
    err: &str,
) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to update project: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        id
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Update Project Error", ctx, Some(err), &content),
    )
}

pub fn render_proj_delete_error(ctx: &AuthContext, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to delete project: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_page" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Delete Project Error", ctx, Some(err), &content),
    )
}

pub fn render_subproj_error(ctx: &AuthContext, id: i64, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to modify subproject: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        id
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Subproject Error", ctx, Some(err), &content),
    )
}

pub fn render_category_error(
    ctx: &AuthContext,
    target_url: &str,
    err: &str,
) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to process category: {}</p>
        <p style="margin-top:12px;"><a href="{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        target_url
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Category Error", ctx, Some(err), &content),
    )
}

pub fn render_version_error(
    ctx: &AuthContext,
    project_id: i64,
    err: &str,
) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to process version: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        project_id
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Version Error", ctx, Some(err), &content),
    )
}

pub fn render_proj_user_error(ctx: &AuthContext, id: i64, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to process project user: {}</p>
        <p style="margin-top:12px;"><a href="/manage_proj_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        id
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Project User Error", ctx, Some(err), &content),
    )
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod tests;
