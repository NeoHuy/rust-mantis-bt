use crate::auth::{make_cookie_header, parse_cookies, AuthContext};
use crate::db::Db;
use crate::views;
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
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

pub fn router() -> Router<Db> {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", get(logout).post(logout))
        .route("/set_project", post(set_project))
        .route("/account_page", get(account_page))
        .route("/account_update", post(account_update))
}

pub async fn login_page(headers: HeaderMap, State(db): State<Db>) -> Response {
    let cookies = parse_cookies(&headers);
    if let Some(cookie_str) = cookies.get("MANTIS_STRING_COOKIE") {
        if let Ok(Some(user)) = db.get_user_by_cookie(cookie_str) {
            if user.enabled == 1 {
                return Redirect::to("/my_view_page").into_response();
            }
        }
    }

    views::auth::render_login_page(None).into_response()
}

pub async fn login_submit(State(db): State<Db>, Form(form): Form<LoginForm>) -> Response {
    match db.authenticate(&form.username, &form.password) {
        Ok(Some(user)) => {
            let cookie_val =
                make_cookie_header("MANTIS_STRING_COOKIE", &user.cookie_string, true, None);
            let target_url = "/my_view_page";
            let mut res = Redirect::to(target_url).into_response();
            if let Ok(hv) = HeaderValue::from_str(&cookie_val) {
                res.headers_mut().insert(SET_COOKIE, hv);
            }
            res
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            views::auth::render_login_page(Some("Invalid username or password.")),
        )
            .into_response(),
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

pub async fn account_page(Query(query): Query<AccountQuery>, ctx: AuthContext) -> Html<String> {
    views::auth::render_account_page(&ctx, query.msg.as_deref())
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
                views::layout::render_layout(
                    "Error",
                    &ctx,
                    Some("Passwords do not match."),
                    content,
                ),
            )
                .into_response();
        }
        if ctx.user.protected == 1 {
            let content = "<p>Protected account password cannot be changed.</p>";
            return (
                StatusCode::FORBIDDEN,
                views::layout::render_layout(
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
            let content = format!(
                "<p>Failed to update account: {}</p>",
                views::html_escape(&e)
            );
            (
                StatusCode::BAD_REQUEST,
                views::layout::render_layout("Error", &ctx, Some(&e), &content),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
#[path = "auth_test.rs"]
mod tests;
