use crate::auth::AuthContext;
use crate::models::{
    AccessLevel, Category, Project, ProjectStatus, ProjectVersion, User, ViewState,
};
use crate::views::layout::render_layout;
use crate::views::{format_date, html_escape};
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
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
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
                    <a href="/manage_proj_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>
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
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };

        cat_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_cat_delete/{}" method="POST" class="inline-block m-0" data-confirm="Delete category?">
                        <input type="hidden" name="project_id" value="0" />
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
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
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-folder-open" aria-hidden="true"></i> Projects</h4>
                <div class="widget-toolbar">
                    <a href="/manage_proj_create_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-plus" aria-hidden="true"></i> Create New Project</a>
                </div>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
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
                </div>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-tags" aria-hidden="true"></i> Global Categories</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
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
                </div>

                <form method="POST" action="/manage_proj_cat_add" class="mt-3.5 flex flex-wrap items-center gap-2">
                    <input type="hidden" name="project_id" value="0" />
                    <input type="text" name="name" placeholder="New Global Category" required class="!w-52" />
                    <select name="user_id" class="!w-44">
                        {}
                    </select>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-plus" aria-hidden="true"></i> Add Category</button>
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
    <div class="widget-box widget-color-blue2">
        <div class="widget-header">
            <h4 class="widget-title"><i class="fa fa-plus-circle" aria-hidden="true"></i> Create Project</h4>
        </div>
        <div class="widget-body">
            <form method="POST" action="/manage_proj_create">
                <div class="table-responsive">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Project Name <span class="required">*</span></td>
                            <td class="input-col"><input type="text" name="name" required autofocus /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Status</td>
                            <td class="input-col">
                                <select name="status">
                                    <option value="10" selected>development</option>
                                    <option value="30">release</option>
                                    <option value="50">stable</option>
                                    <option value="70">obsolete</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" selected>public</option>
                                    <option value="50">private</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Inherit Global Categories</td>
                            <td class="input-col">
                                <label class="inline-flex items-center cursor-pointer"><input type="checkbox" name="inherit_global" value="1" checked /> Inherit global categories</label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description"></textarea></td>
                        </tr>
                    </table>
                </div>
                <div class="mt-3.5 flex flex-wrap gap-2">
                    <button type="submit" class="btn btn-primary"><i class="fa fa-plus" aria-hidden="true"></i> Add Project</button>
                    <a href="/manage_proj_page" class="btn btn-white btn-round">Cancel</a>
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
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
        let inherit_str = if *inherit_parent { "yes" } else { "no" };
        subproj_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><a href="/manage_proj_edit_page/{}">{}</a></td>
                <td>{}</td>
                <td>
                    <form action="/manage_proj_subproj_delete/{}/{}" method="POST" class="inline-block m-0" data-confirm="Remove subproject relationship?">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Unlink</button>
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
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };

        let mut cat_user_opts = String::new();
        cat_user_opts.push_str(r#"<option value="0">- None -</option>"#);
        for u in all_users {
            let sel = if u.id == c.user_id { "selected" } else { "" };
            cat_user_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                u.id,
                sel,
                html_escape(&u.username)
            ));
        }

        cat_rows.push_str(&format!(
            r#"<tr class="{row_class}">
                <td><strong>{name}</strong></td>
                <td>{assignee}</td>
                <td>
                    <form action="/manage_proj_cat_delete/{cat_id}" method="POST" class="inline-block m-0" data-confirm="Delete category?">
                        <input type="hidden" name="project_id" value="{proj_id}" />
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                    </form>
                </td>
            </tr>
            <tr class="{row_class} edit-row bg-[#f9fbfd]">
                <td colspan="3" class="px-3 py-1.5">
                    <form method="POST" action="/manage_proj_cat_update/{cat_id}" class="inline-flex flex-wrap items-center gap-2 m-0">
                        <input type="hidden" name="project_id" value="{proj_id}" />
                        <span class="text-xs text-gray-600 font-semibold">Edit:</span>
                        <input type="text" name="name" value="{name}" required class="!w-44" placeholder="Category Name" />
                        <label class="text-xs text-gray-600 ml-1">Assign To:</label>
                        <select name="user_id" class="!w-40">
                            {cat_user_opts}
                        </select>
                        <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-save" aria-hidden="true"></i> Update Category</button>
                    </form>
                </td>
            </tr>"#,
            row_class = row_class,
            name = html_escape(&c.name),
            assignee = html_escape(assignee_name),
            cat_id = c.id,
            proj_id = project.id,
            cat_user_opts = cat_user_opts
        ));
    }

    let mut ver_rows = String::new();
    for (idx, v) in versions.iter().enumerate() {
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
        let rel_badge = if v.released == 1 { "yes" } else { "no" };
        let obs_badge = if v.obsolete == 1 { "yes" } else { "no" };
        let rel_checked = if v.released == 1 { "checked" } else { "" };
        let obs_checked = if v.obsolete == 1 { "checked" } else { "" };
        let date_str = format_date(v.date_order);
        let date_display = if date_str.is_empty() {
            "-".to_string()
        } else {
            date_str.clone()
        };

        ver_rows.push_str(&format!(
            r#"<tr class="{row_class}">
                <td><strong>{version}</strong></td>
                <td>{desc}</td>
                <td>{date_display}</td>
                <td>{rel_badge}</td>
                <td>{obs_badge}</td>
                <td>
                    <form action="/manage_proj_ver_delete/{ver_id}" method="POST" class="inline-block m-0" data-confirm="Delete version?">
                        <input type="hidden" name="project_id" value="{proj_id}" />
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                    </form>
                </td>
            </tr>
            <tr class="{row_class} edit-row bg-[#f9fbfd]">
                <td colspan="6" class="px-3 py-1.5">
                    <form method="POST" action="/manage_proj_ver_update/{ver_id}" class="inline-flex flex-wrap items-center gap-2 m-0">
                        <input type="hidden" name="project_id" value="{proj_id}" />
                        <span class="text-xs text-gray-600 font-semibold">Edit:</span>
                        <input type="text" name="version" value="{version}" required class="!w-36" placeholder="Version" />
                        <input type="text" name="description" value="{desc}" placeholder="Description" class="!w-48" />
                        <input type="date" name="date_order" value="{date_str}" class="!w-32" title="Date Order" />
                        <label class="flex items-center gap-1 text-xs cursor-pointer"><input type="checkbox" name="released" value="1" {rel_checked} /> Released</label>
                        <label class="flex items-center gap-1 text-xs cursor-pointer"><input type="checkbox" name="obsolete" value="1" {obs_checked} /> Obsolete</label>
                        <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-save" aria-hidden="true"></i> Update Version</button>
                    </form>
                </td>
            </tr>"#,
            row_class = row_class,
            version = html_escape(&v.version),
            desc = html_escape(&v.description),
            date_display = date_display,
            rel_badge = rel_badge,
            obs_badge = obs_badge,
            ver_id = v.id,
            proj_id = project.id,
            date_str = date_str,
            rel_checked = rel_checked,
            obs_checked = obs_checked,
        ));
    }

    let mut user_rows = String::new();
    for (idx, (u, access_level)) in proj_users.iter().enumerate() {
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
        let lvl_name = AccessLevel::from_i32(*access_level)
            .map(|a| a.as_str())
            .unwrap_or("unknown");
        user_rows.push_str(&format!(
            r#"<tr class="{}">
                <td>{}</td>
                <td>{} ({})</td>
                <td>
                    <form action="/manage_proj_user_remove/{}/{}" method="POST" class="inline-block m-0" data-confirm="Remove user from project?">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Remove</button>
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
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-pencil" aria-hidden="true"></i> Edit Project: {}</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/manage_proj_update/{}">
                    <div class="table-responsive">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Project Name <span class="required">*</span></td>
                                <td class="input-col"><input type="text" name="name" value="{}" required /></td>
                            </tr>
                            <tr>
                                <td class="label-col">Status</td>
                                <td class="input-col">
                                    <select name="status">
                                        <option value="10" {}>development</option>
                                        <option value="30" {}>release</option>
                                        <option value="50" {}>stable</option>
                                        <option value="70" {}>obsolete</option>
                                    </select>
                                </td>
                            </tr>
                            <tr>
                                <td class="label-col">Enabled</td>
                                <td class="input-col">
                                    <label class="inline-flex items-center cursor-pointer"><input type="checkbox" name="enabled" value="1" {} /> Enabled</label>
                                </td>
                            </tr>
                            <tr>
                                <td class="label-col">View Status</td>
                                <td class="input-col">
                                    <select name="view_state">
                                        <option value="10" {}>public</option>
                                        <option value="50" {}>private</option>
                                    </select>
                                </td>
                            </tr>
                        <tr>
                            <td class="label-col">Inherit Global Categories</td>
                            <td class="input-col">
                                <label class="inline-flex items-center cursor-pointer"><input type="checkbox" name="inherit_global" value="1" {} /> Inherit global categories</label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description">{}</textarea></td>
                        </tr>
                    </table>
                    </div>
                    <div class="mt-3.5 flex flex-wrap gap-2">
                        <button type="submit" class="btn btn-primary"><i class="fa fa-save" aria-hidden="true"></i> Update Project</button>
                        <a href="/manage_proj_page" class="btn btn-white btn-round">Back to Projects</a>
                    </div>
                </form>
                <div class="mt-3 pt-3 border-t border-[#e0e6ed]">
                    <form action="/manage_proj_delete/{}" method="POST" class="inline-block m-0" data-confirm="Are you sure you want to delete this project? All associated data will be removed.">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete Project</button>
                    </form>
                </div>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-sitemap" aria-hidden="true"></i> Subprojects</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
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
                </div>

                <form method="POST" action="/manage_proj_subproj_add/{}" class="mt-3.5 flex flex-wrap items-center gap-2">
                    <select name="child_id" required class="!w-64">
                        <option value="">- Select Subproject to Link -</option>
                        {}
                    </select>
                    <label class="flex items-center cursor-pointer">
                        <input type="checkbox" name="inherit_parent" value="1" checked /> Inherit Parent Categories
                    </label>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-link" aria-hidden="true"></i> Link Subproject</button>
                </form>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-tags" aria-hidden="true"></i> Categories</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
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
                </div>

                <form method="POST" action="/manage_proj_cat_add" class="mt-3.5 flex flex-wrap items-center gap-2">
                    <input type="hidden" name="project_id" value="{}" />
                    <input type="text" name="name" placeholder="New Category Name" required class="!w-52" />
                    <select name="user_id" class="!w-44">
                        <option value="0">- None -</option>
                        {}
                    </select>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-plus" aria-hidden="true"></i> Add Category</button>
                </form>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-code-fork" aria-hidden="true"></i> Versions</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>Version</th>
                                <th>Description</th>
                                <th>Date Order</th>
                                <th>Released</th>
                                <th>Obsolete</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {}
                        </tbody>
                    </table>
                </div>

                <form method="POST" action="/manage_proj_ver_add/{}" class="mt-3.5 flex flex-wrap items-center gap-2">
                    <input type="text" name="version" placeholder="Version (e.g. 1.0.0)" required class="!w-40" />
                    <input type="text" name="description" placeholder="Description" class="!w-56" />
                    <input type="date" name="date_order" class="!w-36" placeholder="YYYY-MM-DD" title="Date Order" />
                    <label class="flex items-center cursor-pointer"><input type="checkbox" name="released" value="1" /> Released</label>
                    <label class="flex items-center cursor-pointer"><input type="checkbox" name="obsolete" value="1" /> Obsolete</label>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-plus" aria-hidden="true"></i> Add Version</button>
                </form>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-users" aria-hidden="true"></i> User Access &amp; Overrides</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
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
                </div>

                <form method="POST" action="/manage_proj_user_add/{}" class="mt-3.5 flex flex-wrap items-center gap-2">
                    <select name="user_id" required class="!w-56">
                        <option value="">- Select User -</option>
                        {}
                    </select>
                    <select name="access_level" required class="!w-40">
                        <option value="10">viewer (10)</option>
                        <option value="25" selected>reporter (25)</option>
                        <option value="40">updater (40)</option>
                        <option value="55">developer (55)</option>
                        <option value="70">manager (70)</option>
                        <option value="90">administrator (90)</option>
                    </select>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-user-plus" aria-hidden="true"></i> Add User</button>
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
        <p class="mt-3"><a href="/manage_proj_create_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_proj_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_proj_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_proj_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_proj_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_proj_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
