use crate::auth::AuthContext;
use crate::models::AccessLevel;
use crate::views::html_escape;
use crate::views::layout::{render_layout, render_unauthenticated_layout};
use axum::response::Html;

pub fn render_login_page(flash_error: Option<&str>) -> Html<String> {
    let form_html = r#"
    <div class="login-box">
        <div class="login-header">&#128274; Sign In</div>
        <div class="login-body">
            <form method="POST" action="/login">
                <div style="margin-bottom: 12px;">
                    <label for="username" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Username</label>
                    <input type="text" name="username" id="username" placeholder="Username" autofocus required style="width:100%;" />
                </div>
                <div style="margin-bottom: 14px;">
                    <label for="password" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Password</label>
                    <input type="password" name="password" id="password" placeholder="Password" required style="width:100%;" />
                </div>
                <div style="margin-bottom: 16px; display:flex; align-items:center; justify-content:space-between; font-size:12px; color:#555;">
                    <label style="cursor:pointer;"><input type="checkbox" name="perm_login" value="1" checked /> Remember my login on this browser</label>
                </div>
                <div>
                    <button type="submit" class="btn" style="width:100%; padding:8px; font-size:13px;">Login</button>
                </div>
            </form>
        </div>
    </div>
    "#;

    let title = if flash_error.is_some() {
        "Login Failed"
    } else {
        "Login"
    };

    render_unauthenticated_layout(title, flash_error, form_html)
}

pub fn render_account_page(ctx: &AuthContext, flash: Option<&str>) -> Html<String> {
    let access_level_name = AccessLevel::from_i32(ctx.user.access_level)
        .map(|a| a.as_str())
        .unwrap_or("unknown");

    let content = format!(
        r#"
        <div class="panel">
            <div class="panel-header">Edit Account</div>
            <div class="panel-body">
                <form method="POST" action="/account_update">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Username</td>
                            <td class="input-col"><input type="text" value="{}" disabled /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Real Name</td>
                            <td class="input-col"><input type="text" name="realname" value="{}" required /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Email</td>
                            <td class="input-col"><input type="email" name="email" value="{}" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Password</td>
                            <td class="input-col"><input type="password" name="password" placeholder="Leave blank to keep unchanged" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Confirm Password</td>
                            <td class="input-col"><input type="password" name="password_confirm" placeholder="Confirm new password" /></td>
                        </tr>
                        <tr>
                            <td class="label-col">Access Level</td>
                            <td class="input-col"><strong>{}</strong></td>
                        </tr>
                    </table>
                    <div style="margin-top: 14px;">
                        <button type="submit" class="btn">Update User</button>
                    </div>
                </form>
            </div>
        </div>
        "#,
        html_escape(&ctx.user.username),
        html_escape(&ctx.user.realname),
        html_escape(&ctx.user.email),
        html_escape(access_level_name),
    );

    render_layout("My Account", ctx, flash, &content)
}
