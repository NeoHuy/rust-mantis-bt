pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
pub mod filters;
pub mod jira;
pub mod planning;
pub mod projects;
pub mod tags;
pub mod users;

use crate::auth::AuthContext;
use crate::db::Db;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;

pub fn parse_bool(opt: Option<&str>) -> bool {
    match opt {
        Some(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "on" || v == "yes"
        }
        None => false,
    }
}

pub async fn root_handler(ctx: AuthContext) -> impl IntoResponse {
    if ctx.user.access_level >= 70 {
        Redirect::to("/manage_proj_page")
    } else {
        Redirect::to("/account_page")
    }
}

async fn favicon_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../../assets/favicon.ico"),
    )
}

async fn logo_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../assets/mantis_logo_notext.png"),
    )
}

fn extract_url_authority(url_str: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url_str.trim()).ok()?;
    let host = parsed.host_str()?;
    Some(match parsed.port() {
        Some(port) => format!("{}:{}", host, port),
        None => host.to_string(),
    })
}

fn host_matches_url(host_header: &str, url_str: &str) -> bool {
    let Some(authority) = extract_url_authority(url_str) else {
        return false;
    };
    authority.eq_ignore_ascii_case(host_header.trim())
}

fn csrf_forbidden(msg: &'static str) -> axum::response::Response {
    (
        axum::http::StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

pub async fn security_headers_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; form-action 'self'; frame-ancestors 'self'; base-uri 'self'",
        ),
    );
    let secure_cookie = std::env::var("MANTIS_COOKIE_SECURE")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true"
        })
        .unwrap_or(false);
    if secure_cookie {
        headers.insert(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

pub async fn csrf_protection_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method();
    let is_state_changing = matches!(
        *method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::PATCH
            | axum::http::Method::DELETE
    );
    if !is_state_changing {
        return next.run(request).await;
    }

    let path = request.uri().path();
    if path.starts_with("/api/rest/") || path == "/mcp" || path.starts_with("/mcp/") {
        return next.run(request).await;
    }

    let headers = request.headers();

    // Check 1: Fetch Metadata Sec-Fetch-Site
    if let Some(sec_fetch_site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        let sfs = sec_fetch_site.trim().to_ascii_lowercase();
        if sfs != "same-origin" && sfs != "none" {
            return csrf_forbidden("CSRF validation failed: cross-site request rejected");
        }
    }

    let host_hdr = headers
        .get("x-forwarded-host")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get(axum::http::header::HOST)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim())
        })
        .or_else(|| request.uri().authority().map(|a| a.as_str()));

    let public_origin = std::env::var("MANTIS_PUBLIC_ORIGIN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Check 2: Origin header
    if let Some(origin_val) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        let trimmed_origin = origin_val.trim();
        if trimmed_origin.eq_ignore_ascii_case("null") {
            return csrf_forbidden("CSRF validation failed: null Origin rejected");
        }
        let matches_host = host_hdr
            .map(|host| host_matches_url(host, trimmed_origin))
            .unwrap_or(false);
        let matches_public = public_origin
            .as_deref()
            .map(|po| {
                if let (Ok(u1), Ok(u2)) =
                    (reqwest::Url::parse(po), reqwest::Url::parse(trimmed_origin))
                {
                    u1.scheme().eq_ignore_ascii_case(u2.scheme())
                        && u1
                            .host_str()
                            .unwrap_or("")
                            .eq_ignore_ascii_case(u2.host_str().unwrap_or(""))
                        && u1.port_or_known_default() == u2.port_or_known_default()
                } else if let (Some(a1), Some(a2)) = (
                    extract_url_authority(po),
                    extract_url_authority(trimmed_origin),
                ) {
                    a1.eq_ignore_ascii_case(&a2)
                } else {
                    trimmed_origin.eq_ignore_ascii_case(po)
                }
            })
            .unwrap_or(false);

        if !matches_host && !matches_public {
            return csrf_forbidden("CSRF validation failed: Origin mismatch");
        }
    } else if let Some(referer_val) = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
    {
        // Check 3: Referer fallback when Origin is absent
        let matches_host = host_hdr
            .map(|host| host_matches_url(host, referer_val.trim()))
            .unwrap_or(false);
        let matches_public = public_origin
            .as_deref()
            .map(|po| {
                if let (Ok(u1), Ok(u2)) = (
                    reqwest::Url::parse(po),
                    reqwest::Url::parse(referer_val.trim()),
                ) {
                    u1.scheme().eq_ignore_ascii_case(u2.scheme())
                        && u1
                            .host_str()
                            .unwrap_or("")
                            .eq_ignore_ascii_case(u2.host_str().unwrap_or(""))
                        && u1.port_or_known_default() == u2.port_or_known_default()
                } else if let (Some(a1), Some(a2)) = (
                    extract_url_authority(po),
                    extract_url_authority(referer_val.trim()),
                ) {
                    a1.eq_ignore_ascii_case(&a2)
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if !matches_host && !matches_public {
            return csrf_forbidden("CSRF validation failed: Referer mismatch");
        }
    }

    // Check 4: If X-CSRF-Token header is provided (e.g. from AJAX/fetch requests), validate it
    if let Some(token_hdr) = headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) {
        let cookies = crate::auth::parse_cookies(headers);
        let session_cookie = cookies
            .get("MANTIS_STRING_COOKIE")
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty());

        match session_cookie {
            Some(cookie_str) if crate::auth::verify_csrf_token(cookie_str, token_hdr) => {}
            _ => return csrf_forbidden("CSRF validation failed: invalid X-CSRF-Token"),
        }
    }

    next.run(request).await
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/", get(root_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/images/mantis_logo.png", get(logo_handler))
        .merge(auth::router())
        .merge(users::router())
        .merge(projects::router())
        .merge(bugs::router())
        .merge(tags::router())
        .merge(custom_fields::router())
        .merge(filters::router())
        .merge(planning::router())
        .merge(api::router())
        .merge(jira::router())
        .layer(axum::middleware::from_fn(csrf_protection_middleware))
}

pub fn build_router(db: Db) -> Router {
    let mcp_router = crate::mcp::router(db.clone(), crate::mcp::SessionManager::new());
    router()
        .with_state(db)
        .merge(mcp_router)
        .layer(axum::middleware::from_fn(security_headers_middleware))
}

#[cfg(test)]
#[path = "controllers_test.rs"]
mod tests;
