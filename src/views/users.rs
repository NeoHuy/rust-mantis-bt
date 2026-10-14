use crate::auth::AuthContext;
use crate::models::{AccessLevel, User};
use crate::views::layout::render_layout;
use crate::views::html_escape;
use axum::http::StatusCode;
use axum::response::Html;

pub fn render_manage_users_page(
    ctx: &AuthContext,
    users: &[User],
    search: &str,
    enabled_only: bool,
) -> Html<String> {
    let mut rows_html = String::new();
    for (idx, u) in users.iter().enumerate() {
        let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let lvl_str = AccessLevel::from_i32(u.access_level)
            .map(|a| a.as_str())
            .unwrap_or("unknown");
        let enabled_badge = if u.enabled == 1 {
            r#"<span class="badge badge-success">enabled</span>"#
        } else {
            r#"<span class="badge badge-danger">disabled</span>"#
        };
        let protected_badge = if u.protected == 1 {
            r#"<span class="badge badge-warning">protected</span>"#
        } else {
            ""
        };

        rows_html.push_str(&format!(
            r#"<tr class="{}">
                <td><a href="/manage_user_edit_page/{}">{}</a></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <a href="/manage_user_edit_page/{}" class="btn btn-secondary btn-sm">Edit</a>
                    <form action="/manage_user_delete/{}" method="POST" style="display:inline; margin-left:4px;" onsubmit="return confirm('Are you sure you want to delete this user?');">
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            row_class,
            u.id,
            html_escape(&u.username),
            html_escape(&u.realname),
            html_escape(&u.email),
            html_escape(lvl_str),
            enabled_badge,
            protected_badge,
            u.id,
            u.id,
        ));
    }

    let enabled_checked = if enabled_only { "checked" } else { "" };

    let content = format!(
        r#"
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:14px;">
            <h2 class="section-title" style="margin:0; border:none;">Manage Users</h2>
            <a href="/manage_user_create_page" class="btn">Create New Account</a>
        </div>

        <div class="panel" style="margin-bottom:14px;">
            <div class="panel-body" style="padding:10px 14px;">
                <form method="GET" action="/manage_user_page" style="display:flex; align-items:center; gap:12px; flex-wrap:wrap;">
                    <label for="search">Search:</label>
                    <input type="text" name="search" id="search" value="{}" placeholder="Filter by username, real name or email" style="width:250px;" />
                    <label style="display:flex; align-items:center; cursor:pointer;">
                        <input type="checkbox" name="filter_enabled" value="1" {} /> Enabled only
                    </label>
                    <button type="submit" class="btn btn-secondary">Apply Filter</button>
                    <a href="/manage_user_page" class="btn btn-secondary">Reset</a>
                </form>
            </div>
        </div>

        <table class="data-table">
            <thead>
                <tr>
                    <th>Username</th>
                    <th>Real Name</th>
                    <th>Email</th>
                    <th>Access Level</th>
                    <th>Status</th>
                    <th>Protection</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
        "#,
        html_escape(search),
        enabled_checked,
        rows_html
    );

    render_layout("Manage Users", ctx, None, &content)
}

pub fn render_user_create_page(ctx: &AuthContext) -> Html<String> {
    let content = r#"
    <div class="panel">
        <div class="panel-header">Create User Account</div>
        <div class="panel-body">
            <form method="POST" action="/manage_user_create">
                <table class="form-table">
                    <tr>
                        <td class="label-col">Username *</td>
                        <td class="input-col"><input type="text" name="username" required autofocus /></td>
                    </tr>
                    <tr>
                        <td class="label-col">Real Name</td>
                        <td class="input-col"><input type="text" name="realname" /></td>
                    </tr>
                    <tr>
                        <td class="label-col">Email</td>
                        <td class="input-col"><input type="email" name="email" /></td>
                    </tr>
                    <tr>
                        <td class="label-col">Password *</td>
                        <td class="input-col"><input type="password" name="password" required /></td>
                    </tr>
                    <tr>
                        <td class="label-col">Access Level</td>
                        <td class="input-col">
                            <select name="access_level">
                                <option value="10">viewer (10)</option>
                                <option value="25" selected>reporter (25)</option>
                                <option value="40">updater (40)</option>
                                <option value="55">developer (55)</option>
                                <option value="70">manager (70)</option>
                                <option value="90">administrator (90)</option>
                            </select>
                        </td>
                    </tr>
                    <tr>
                        <td class="label-col">Enabled</td>
                        <td class="input-col">
                            <label><input type="checkbox" name="enabled" value="1" checked /> Account is active and can log in</label>
                        </td>
                    </tr>
                    <tr>
                        <td class="label-col">Protected</td>
                        <td class="input-col">
                            <label><input type="checkbox" name="protected" value="1" /> Protected accounts cannot be modified or deleted</label>
                        </td>
                    </tr>
                </table>
                <div style="margin-top: 14px;">
                    <button type="submit" class="btn">Create User</button>
                    <a href="/manage_user_page" class="btn btn-secondary" style="margin-left:8px;">Cancel</a>
                </div>
            </form>
        </div>
    </div>
    "#;

    render_layout("Create User", ctx, None, content)
}

pub fn render_user_edit_page(ctx: &AuthContext, target_user: &User) -> Html<String> {
    let levels = [10, 25, 40, 55, 70, 90];
    let mut level_opts = String::new();
    for lvl in levels {
        let name = AccessLevel::from_i32(lvl)
            .map(|a| a.as_str())
            .unwrap_or("unknown");
        let selected = if target_user.access_level == lvl {
            "selected"
        } else {
            ""
        };
        level_opts.push_str(&format!(
            r#"<option value="{}" {}>{} ({})</option>"#,
            lvl, selected, name, lvl
        ));
    }

    let enabled_checked = if target_user.enabled == 1 {
        "checked"
    } else {
        ""
    };
    let protected_checked = if target_user.protected == 1 {
        "checked"
    } else {
        ""
    };

    let delete_btn = if target_user.protected == 0 {
        format!(
            r#"
            <div style="margin-top: 12px; border-top: 1px solid #e0e6ed; padding-top: 12px;">
                <form action="/manage_user_delete/{}" method="POST" style="display:inline;" onsubmit="return confirm('Are you sure you want to delete this user?');">
                    <button type="submit" class="btn btn-danger">Delete User</button>
                </form>
            </div>
            "#,
            target_user.id
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"
        <div class="panel">
            <div class="panel-header">Edit User: {}</div>
            <div class="panel-body">
                <form method="POST" action="/manage_user_update/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Username</td>
                            <td class="input-col"><input type="text" value="{}" disabled /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Real Name</td>
                            <td class="input-col"><input type="text" name="realname" value="{}" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Email</td>
                            <td class="input-col"><input type="email" name="email" value="{}" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Password</td>
                            <td class="input-col"><input type="password" name="password" placeholder="Leave blank to keep current password" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Access Level</td>
                            <td class="input-col">
                                <select name="access_level">
                                    {}
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
                            <td class="label-col">Protected</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="protected" value="1" {} /> Protected</label>
                            </td>
                        </tr>
                    </table>
                    <div style="margin-top: 14px;">
                        <button type="submit" class="btn">Update User</button>
                        <a href="/manage_user_page" class="btn btn-secondary" style="margin-left:8px;">Cancel</a>
                    </div>
                </form>
                {}
            </div>
        </div>
        "#,
        html_escape(&target_user.username),
        target_user.id,
        html_escape(&target_user.username),
        html_escape(&target_user.realname),
        html_escape(&target_user.email),
        level_opts,
        enabled_checked,
        protected_checked,
        delete_btn,
    );

    render_layout("Edit User", ctx, None, &content)
}

pub fn render_user_create_error(ctx: &AuthContext, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to create user: {}</p>
        <p style="margin-top:12px;"><a href="/manage_user_create_page" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Create User Error", ctx, Some(err), &content),
    )
}

pub fn render_user_update_error(ctx: &AuthContext, id: i64, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to update user: {}</p>
        <p style="margin-top:12px;"><a href="/manage_user_edit_page/{}" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err),
        id
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Update User Error", ctx, Some(err), &content),
    )
}

pub fn render_user_delete_error(ctx: &AuthContext, err: &str) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to delete user: {}</p>
        <p style="margin-top:12px;"><a href="/manage_user_page" class="btn btn-secondary">Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Delete User Error", ctx, Some(err), &content),
    )
}
