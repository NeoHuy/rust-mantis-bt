use crate::auth::AuthContext;
use crate::models::AccessLevel;
use crate::views::html_escape;
use crate::views::layout::{render_layout, render_unauthenticated_layout};
use axum::response::Html;

pub fn render_login_page(flash_error: Option<&str>, allow_signup: bool) -> Html<String> {
    let signup_toolbar = if allow_signup {
        r#"<div class="login-toolbar"><a href="/signup_page"><i class="fa fa-arrow-right" aria-hidden="true"></i> Signup for a new account</a></div>"#
    } else {
        ""
    };

    let form_html = format!(
        r#"
    <div class="login-box">
        <div class="login-header">
            <i class="fa fa-sign-in" aria-hidden="true"></i>
            <span>Login</span>
        </div>
        <div class="login-body">
            <form method="POST" action="/login">
                <div class="login-field">
                    <label for="username">Username</label>
                    <span class="input-icon input-icon-right">
                        <input type="text" name="username" id="username" placeholder="Username" autofocus required />
                        <i class="ace-icon fa fa-user" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-field">
                    <label for="password">Password</label>
                    <span class="input-icon input-icon-right">
                        <input type="password" name="password" id="password" placeholder="Password" required />
                        <i class="ace-icon fa fa-lock" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-submit-row">
                    <label class="inline-flex items-center cursor-pointer text-xs text-neutral-600">
                        <input type="checkbox" name="perm_login" value="1" checked /> Remember my login on this browser
                    </label>
                    <button type="submit" class="btn-login-charcoal">
                        <i class="fa fa-key" aria-hidden="true"></i> Login
                    </button>
                </div>
            </form>
        </div>
        {signup_toolbar}
    </div>
    "#,
        signup_toolbar = signup_toolbar
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
        <div class="login-header">
            <i class="fa fa-users" aria-hidden="true"></i>
            <span>Create New Account</span>
        </div>
        <div class="login-body">
            <form method="POST" action="/signup">
                <div class="login-field">
                    <label for="username">Username *</label>
                    <span class="input-icon input-icon-right">
                        <input type="text" name="username" id="username" value="{}" placeholder="Username" autofocus required />
                        <i class="ace-icon fa fa-user" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-field">
                    <label for="realname">Real Name</label>
                    <span class="input-icon input-icon-right">
                        <input type="text" name="realname" id="realname" value="{}" placeholder="Real Name (optional)" />
                        <i class="ace-icon fa fa-id-card-o" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-field">
                    <label for="email">Email *</label>
                    <span class="input-icon input-icon-right">
                        <input type="email" name="email" id="email" value="{}" placeholder="Email Address" required />
                        <i class="ace-icon fa fa-envelope" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-field">
                    <label for="password">Password * (min 6 chars)</label>
                    <span class="input-icon input-icon-right">
                        <input type="password" name="password" id="password" placeholder="Password" required />
                        <i class="ace-icon fa fa-lock" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-field">
                    <label for="password_confirm">Confirm Password *</label>
                    <span class="input-icon input-icon-right">
                        <input type="password" name="password_confirm" id="password_confirm" placeholder="Confirm Password" required />
                        <i class="ace-icon fa fa-retweet" aria-hidden="true"></i>
                    </span>
                </div>
                <div class="login-submit-row">
                    <span></span>
                    <button type="submit" class="btn-login-charcoal">
                        <i class="fa fa-user-plus" aria-hidden="true"></i> Sign Up
                    </button>
                </div>
            </form>
        </div>
        <div class="login-toolbar">
            <a href="/login"><i class="fa fa-arrow-left" aria-hidden="true"></i> Already have an account? Sign In</a>
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
        <div class="login-header" style="background-color: #629b58; color: #ffffff;">
            <i class="fa fa-check-circle" aria-hidden="true"></i>
            <span>Registration Successful</span>
        </div>
        <div class="login-body" style="text-align: center; padding: 24px 20px;">
            <p style="font-size: 15px; font-weight: 600; color: #3c763d; margin-bottom: 12px;">
                Account created successfully!
            </p>
            <p style="color: #555555; margin-bottom: 16px;">
                You are now logged in as <strong>{}</strong>.
            </p>
            <p style="color: #777777; font-size: 13px; margin-bottom: 20px;">
                You will be redirected to the dashboard in <span id="countdown" style="font-weight: bold; color: #337ab7;">5</span> seconds...
            </p>
            <div>
                <a href="/my_view_page" class="btn-login-charcoal" style="text-decoration: none;">
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
    <link rel="icon" type="image/png" href="/images/mantis_logo.png">
    <link rel="stylesheet" href="/assets/tailwind.css">
    <link rel="stylesheet" href="/assets/font-awesome.min.css">
</head>
<body class="login-layout light-login">
    <div class="login-container">
        <div class="login-logo-title">
            <img src="/assets/mantis_logo.png" alt="MantisBT Logo" class="login-logo">
        </div>
        {}
    </div>
</body>
</html>"#,
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
                            <td class="label-col">Current Password</td>
                            <td class="input-col"><input type="password" name="current_password" placeholder="Required if changing password" /></td>
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
