use crate::auth::AuthContext;
use crate::models::AccessLevel;
use crate::views::html_escape;
use crate::views::layout::{render_layout, render_unauthenticated_layout};
use axum::response::Html;

pub fn render_login_page(flash_error: Option<&str>, allow_signup: bool) -> Html<String> {
    let signup_link = if allow_signup {
        r#"<div style="margin-top: 14px; text-align: center; font-size: 12px;"><a href="/signup_page" style="color:#0066cc; text-decoration:none;">Signup for a new account</a></div>"#
    } else {
        ""
    };

    let form_html = format!(
        r#"
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
                {}
            </form>
        </div>
    </div>
    "#,
        signup_link
    );

    let title = if flash_error.is_some() {
        "Login Failed"
    } else {
        "Login"
    };

    render_unauthenticated_layout(title, flash_error, &form_html)
}

pub fn render_signup_page(
    flash_error: Option<&str>,
    username: &str,
    realname: &str,
    email: &str,
) -> Html<String> {
    let form_html = format!(
        r#"
    <div class="login-box">
        <div class="login-header">&#128100; Create New Account</div>
        <div class="login-body">
            <form method="POST" action="/signup">
                <div style="margin-bottom: 12px;">
                    <label for="username" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Username *</label>
                    <input type="text" name="username" id="username" value="{}" placeholder="Username" autofocus required style="width:100%;" />
                </div>
                <div style="margin-bottom: 12px;">
                    <label for="realname" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Real Name</label>
                    <input type="text" name="realname" id="realname" value="{}" placeholder="Real Name (optional)" style="width:100%;" />
                </div>
                <div style="margin-bottom: 12px;">
                    <label for="email" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Email *</label>
                    <input type="email" name="email" id="email" value="{}" placeholder="Email Address" required style="width:100%;" />
                </div>
                <div style="margin-bottom: 12px;">
                    <label for="password" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Password * (min 6 chars)</label>
                    <input type="password" name="password" id="password" placeholder="Password" required style="width:100%;" />
                </div>
                <div style="margin-bottom: 16px;">
                    <label for="password_confirm" style="display:block; margin-bottom:4px; font-weight:600; color:#555;">Confirm Password *</label>
                    <input type="password" name="password_confirm" id="password_confirm" placeholder="Confirm Password" required style="width:100%;" />
                </div>
                <div>
                    <button type="submit" class="btn" style="width:100%; padding:8px; font-size:13px;">Sign Up</button>
                </div>
                <div style="margin-top: 14px; text-align: center; font-size: 12px;">
                    <a href="/login" style="color:#0066cc; text-decoration:none;">Already have an account? Sign In</a>
                </div>
            </form>
        </div>
    </div>
    "#,
        html_escape(username),
        html_escape(realname),
        html_escape(email)
    );

    let title = if flash_error.is_some() {
        "Sign Up Failed"
    } else {
        "Create New Account"
    };

    render_unauthenticated_layout(title, flash_error, &form_html)
}

pub fn render_signup_disabled_page() -> Html<String> {
    render_unauthenticated_layout(
        "Registration Disabled",
        Some("User registration is currently disabled."),
        r#"<div class="login-box"><div class="login-body" style="text-align:center;"><p>User registration is currently disabled.</p><p style="margin-top:14px;"><a href="/login" class="btn btn-secondary">Back to Login</a></p></div></div>"#,
    )
}

pub fn render_signup_success_page(username: &str) -> Html<String> {
    let content = format!(
        r#"
    <div class="login-box">
        <div class="login-header" style="background-color: #5cb85c; color: white;">&#10004; Registration Successful</div>
        <div class="login-body" style="text-align: center; padding: 24px 20px;">
            <p style="font-size: 15px; font-weight: 600; color: #3c763d; margin-bottom: 12px;">
                Account created successfully!
            </p>
            <p style="color: #555; margin-bottom: 16px;">
                You are now logged in as <strong>{}</strong>.
            </p>
            <p style="color: #777; font-size: 13px; margin-bottom: 20px;">
                You will be redirected to the dashboard in <span id="countdown" style="font-weight: bold; color: #337ab7;">5</span> seconds...
            </p>
            <div>
                <a href="/my_view_page" class="btn" style="display: inline-block; padding: 8px 18px; text-decoration: none;">
                    Go to Dashboard Now &rarr;
                </a>
            </div>
        </div>
    </div>
    <script>
        let seconds = 5;
        const countdownEl = document.getElementById('countdown');
        const interval = setInterval(function() {{
            seconds--;
            if (countdownEl) {{
                countdownEl.textContent = seconds;
            }}
            if (seconds <= 0) {{
                clearInterval(interval);
                window.location.href = '/my_view_page';
            }}
        }}, 1000);
    </script>
    "#,
        html_escape(username)
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta http-equiv="refresh" content="5;url=/my_view_page">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Registration Successful - MantisBT</title>
    <style>{}</style>
</head>
<body class="login-layout">
    <div class="login-container">
        <div class="login-logo-title">&#128027; <span>Mantis</span>BT</div>
        {}
    </div>
</body>
</html>"#,
        crate::views::layout::CSS,
        content
    );

    Html(html)
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

#[cfg(test)]
#[path = "auth_test.rs"]
mod tests;
