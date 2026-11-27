use crate::auth::AuthContext;
use crate::models::{AccessLevel, User};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::http::StatusCode;
use axum::response::Html;

pub fn render_manage_users_page(
    ctx: &AuthContext,
    users: &[User],
    search: &str,
    enabled_only: bool,
) -> Html<String> {
    render_manage_users_page_with_prefix(ctx, users, search, enabled_only, "")
}

pub fn render_manage_users_page_with_prefix(
    ctx: &AuthContext,
    users: &[User],
    search: &str,
    enabled_only: bool,
    prefix: &str,
) -> Html<String> {
    let mut rows_html = String::new();
    for (idx, u) in users.iter().enumerate() {
        let row_class = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
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
                    <div class="flex flex-wrap items-center gap-1">
                        <a href="/manage_user_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>
                        <form action="/manage_user_delete/{}" method="POST" class="inline-block m-0" data-confirm="Are you sure you want to delete this user?">
                            <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                        </form>
                    </div>
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

    let mut letter_links = String::new();
    let all_active = prefix.is_empty() || prefix.eq_ignore_ascii_case("all");
    let mut base_qs = String::new();
    if !search.is_empty() {
        base_qs.push_str(&format!("&search={}", crate::models::url_encode(search)));
    }
    if enabled_only {
        base_qs.push_str("&filter_enabled=1");
    }

    let all_cls = if all_active {
        "btn btn-primary btn-sm"
    } else {
        "btn btn-white btn-round btn-sm"
    };
    let all_href = if base_qs.is_empty() {
        "/manage_user_page".to_string()
    } else {
        format!("/manage_user_page?{}", base_qs.trim_start_matches('&'))
    };
    letter_links.push_str(&format!(
        r#"<a href="{}" class="{}">All</a>"#,
        all_href, all_cls
    ));

    let num_cls = if prefix == "0-9" {
        "btn btn-primary btn-sm"
    } else {
        "btn btn-white btn-round btn-sm"
    };
    letter_links.push_str(&format!(
        r#"<a href="/manage_user_page?prefix=0-9{}" class="{}">0-9</a>"#,
        base_qs, num_cls
    ));

    for ch in 'A'..='Z' {
        let s = ch.to_string();
        let active = prefix.eq_ignore_ascii_case(&s);
        let cls = if active {
            "btn btn-primary btn-sm"
        } else {
            "btn btn-white btn-round btn-sm"
        };
        letter_links.push_str(&format!(
            r#"<a href="/manage_user_page?prefix={}{}" class="{}">{}</a>"#,
            s, base_qs, cls, s
        ));
    }

    let content = format!(
        r#"
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-users" aria-hidden="true"></i> Manage Users ({})</h4>
                <div class="widget-toolbar">
                    <a href="/manage_user_create_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-user-plus" aria-hidden="true"></i> Create New Account</a>
                </div>
            </div>
            <div class="widget-body">
                <nav class="flex flex-wrap gap-1 mb-3" aria-label="Filter users by initial">
                    {}
                </nav>
                <form method="GET" action="/manage_user_page" class="flex flex-wrap items-center gap-3 mb-3 pb-3 border-b border-[#e5e5e5]">
                    <input type="hidden" name="prefix" value="{}" />
                    <label for="search">Search:</label>
                    <input type="text" name="search" id="search" value="{}" placeholder="Filter by username, real name or email" class="!w-64" />
                    <label class="flex items-center cursor-pointer">
                        <input type="checkbox" name="filter_enabled" value="1" {} /> Enabled only
                    </label>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-filter" aria-hidden="true"></i> Apply Filter</button>
                    <a href="/manage_user_page" class="btn btn-white btn-round btn-sm">Reset</a>
                </form>

                <div class="table-responsive">
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
                </div>
            </div>
        </div>
        "#,
        users.len(),
        letter_links,
        html_escape(prefix),
        html_escape(search),
        enabled_checked,
        rows_html
    );

    render_layout("Manage Users", ctx, None, &content)
}

pub fn render_user_create_page(ctx: &AuthContext) -> Html<String> {
    let content = r#"
    <div class="widget-box widget-color-blue2">
        <div class="widget-header">
            <h4 class="widget-title"><i class="fa fa-user-plus" aria-hidden="true"></i> Create User Account</h4>
        </div>
        <div class="widget-body">
            <form method="POST" action="/manage_user_create">
                <div class="table-responsive">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Username <span class="required">*</span></td>
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
                            <td class="label-col">Password <span class="required">*</span></td>
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
                </div>
                <div class="mt-3.5 flex gap-2">
                    <button type="submit" class="btn btn-primary"><i class="fa fa-user-plus" aria-hidden="true"></i> Create User</button>
                    <a href="/manage_user_page" class="btn btn-secondary">Cancel</a>
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
            <div class="mt-3 pt-3 border-t border-[#e0e6ed]">
                <form action="/manage_user_delete/{}" method="POST" class="inline-block m-0" data-confirm="Are you sure you want to delete this user?">
                    <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete User</button>
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
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-pencil" aria-hidden="true"></i> Edit User: {}</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/manage_user_update/{}">
                    <div class="table-responsive">
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
                    </div>
                    <div class="mt-3.5 flex gap-2">
                        <button type="submit" class="btn btn-primary"><i class="fa fa-save" aria-hidden="true"></i> Update User</button>
                        <a href="/manage_user_page" class="btn btn-secondary">Cancel</a>
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
        <p class="mt-3"><a href="/manage_user_create_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Create User Error", ctx, Some(err), &content),
    )
}

pub fn render_user_update_error(
    ctx: &AuthContext,
    id: i64,
    err: &str,
) -> (StatusCode, Html<String>) {
    let content = format!(
        r#"<p>Failed to update user: {}</p>
        <p class="mt-3"><a href="/manage_user_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
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
        <p class="mt-3"><a href="/manage_user_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a></p>"#,
        html_escape(err)
    );
    (
        StatusCode::BAD_REQUEST,
        render_layout("Delete User Error", ctx, Some(err), &content),
    )
}

#[cfg(test)]
#[path = "users_test.rs"]
mod tests;
