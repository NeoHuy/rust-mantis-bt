use crate::auth::{make_cookie_header, parse_cookies, AuthContext};
use crate::db::Db;
use crate::models::AccessLevel;
use crate::ui::{html_escape, render_layout, render_unauthenticated_layout};
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SetProjectForm {
    pub project_id: i64,
    pub return_url: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountUpdateForm {
    pub realname: String,
    pub email: String,
    pub password: Option<String>,
    pub password_confirm: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountQuery {
    pub msg: Option<String>,
}

pub async fn login_page(headers: HeaderMap, State(db): State<Db>) -> Response {
    let cookies = parse_cookies(&headers);
    if let Some(cookie_str) = cookies.get("MANTIS_STRING_COOKIE") {
        if let Ok(Some(user)) = db.get_user_by_cookie(cookie_str) {
            if user.enabled == 1 {
                if user.access_level >= 70 {
                    return Redirect::to("/manage_proj_page").into_response();
                } else {
                    return Redirect::to("/account_page").into_response();
                }
            }
        }
    }

    let form_html = r#"
    <div class="login-box">
        <div class="login-header">MantisBT Login</div>
        <div class="login-body">
            <form method="POST" action="/login">
                <div style="margin-bottom: 12px;">
                    <label for="username" style="display:block; margin-bottom:4px; font-weight:600;">Username:</label>
                    <input type="text" name="username" id="username" autofocus required style="width:100%;" />
                </div>
                <div style="margin-bottom: 16px;">
                    <label for="password" style="display:block; margin-bottom:4px; font-weight:600;">Password:</label>
                    <input type="password" name="password" id="password" required style="width:100%;" />
                </div>
                <div>
                    <button type="submit" class="btn" style="width:100%; padding:8px; font-size:14px;">Login</button>
                </div>
            </form>
        </div>
    </div>
    "#;

    render_unauthenticated_layout("Login", None, form_html).into_response()
}

pub async fn login_submit(
    State(db): State<Db>,
    Form(form): Form<LoginForm>,
) -> Response {
    match db.authenticate(&form.username, &form.password) {
        Ok(Some(user)) => {
            let cookie_val = make_cookie_header("MANTIS_STRING_COOKIE", &user.cookie_string, true, None);
            let target_url = if user.access_level >= 70 {
                "/manage_proj_page"
            } else {
                "/account_page"
            };
            let mut res = Redirect::to(target_url).into_response();
            if let Ok(hv) = HeaderValue::from_str(&cookie_val) {
                res.headers_mut().insert(SET_COOKIE, hv);
            }
            res
        }
        _ => {
            let form_html = r#"
            <div class="login-box">
                <div class="login-header">MantisBT Login</div>
                <div class="login-body">
                    <form method="POST" action="/login">
                        <div style="margin-bottom: 12px;">
                            <label for="username" style="display:block; margin-bottom:4px; font-weight:600;">Username:</label>
                            <input type="text" name="username" id="username" autofocus required style="width:100%;" />
                        </div>
                        <div style="margin-bottom: 16px;">
                            <label for="password" style="display:block; margin-bottom:4px; font-weight:600;">Password:</label>
                            <input type="password" name="password" id="password" required style="width:100%;" />
                        </div>
                        <div>
                            <button type="submit" class="btn" style="width:100%; padding:8px; font-size:14px;">Login</button>
                        </div>
                    </form>
                </div>
            </div>
            "#;
            (
                StatusCode::UNAUTHORIZED,
                render_unauthenticated_layout("Login Failed", Some("Invalid username or password."), form_html),
            )
                .into_response()
        }
    }
}

pub async fn logout() -> Response {
    let clear_auth = make_cookie_header("MANTIS_STRING_COOKIE", "", true, Some(0));
    let clear_proj = make_cookie_header("MANTIS_PROJECT_COOKIE", "", false, Some(0));

    let mut res = Redirect::to("/login").into_response();
    if let Ok(hv) = HeaderValue::from_str(&clear_auth) {
        res.headers_mut().insert(SET_COOKIE, hv);
    }
    if let Ok(hv) = HeaderValue::from_str(&clear_proj) {
        res.headers_mut().append(SET_COOKIE, hv);
    }
    res
}

pub async fn set_project(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<SetProjectForm>,
) -> Response {
    let mut new_pid = 0;
    if form.project_id > 0 {
        if let Ok(level) = db.effective_access_level(&ctx.user, form.project_id) {
            if level > 0 {
                new_pid = form.project_id;
            }
        }
    }

    let cookie_val = make_cookie_header("MANTIS_PROJECT_COOKIE", &new_pid.to_string(), false, None);

    let redirect_url = match form.return_url {
        Some(ref url) if url.starts_with('/') && !url.starts_with("//") => url.as_str(),
        _ => {
            if ctx.user.access_level >= 70 {
                "/manage_proj_page"
            } else {
                "/account_page"
            }
        }
    };

    let mut res = Redirect::to(redirect_url).into_response();
    if let Ok(hv) = HeaderValue::from_str(&cookie_val) {
        res.headers_mut().insert(SET_COOKIE, hv);
    }
    res
}

pub async fn account_page(
    Query(query): Query<AccountQuery>,
    ctx: AuthContext,
) -> Html<String> {
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

    render_layout("My Account", &ctx, query.msg.as_deref(), &content)
}

pub async fn account_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<AccountUpdateForm>,
) -> Response {
    let new_password = form.password.as_ref().filter(|p| !p.trim().is_empty());
    if let Some(pass) = new_password {
        let confirm = form.password_confirm.as_deref().unwrap_or("");
        if pass != confirm {
            let content = "<p>Passwords do not match.</p>";
            return (
                StatusCode::BAD_REQUEST,
                render_layout("Error", &ctx, Some("Passwords do not match."), content),
            )
                .into_response();
        }
        if ctx.user.protected == 1 {
            let content = "<p>Protected account password cannot be changed.</p>";
            return (
                StatusCode::FORBIDDEN,
                render_layout(
                    "Error",
                    &ctx,
                    Some("Protected account password cannot be changed."),
                    content,
                ),
            )
                .into_response();
        }
    }

    match db.update_user(
        ctx.user.id,
        &form.realname,
        &form.email,
        new_password.map(|s| s.as_str()),
        ctx.user.access_level,
        ctx.user.enabled == 1,
        ctx.user.protected == 1,
    ) {
        Ok(_) => Redirect::to("/account_page?msg=Account+updated+successfully").into_response(),
        Err(e) => {
            let content = format!("<p>Failed to update account: {}</p>", html_escape(&e));
            (
                StatusCode::BAD_REQUEST,
                render_layout("Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
    }
}
