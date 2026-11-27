use crate::auth::{extract_bearer_token, is_session_expired, parse_cookies};
use crate::controllers::bugs::{resolve_bug_filter_for_user, FilterResolveError};
use crate::db::Db;
use crate::models::{
    build_issues_rss_url_for, parse_query_pairs, sort_bugs, Bug, ListBugsQuery, User,
};
use crate::views;
use crate::views::rss::{render_issues_rss, render_news_rss, IssueRssEntry, NewsRssEntry};
use axum::extract::{RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/issues_rss", get(issues_rss))
        .route("/news_rss", get(news_rss))
}

struct RssAuthContext {
    user: User,
    cookie_project_id: i64,
}

#[derive(Default)]
struct RssExtraParams {
    token: Option<String>,
    key: Option<String>,
    username: Option<String>,
    token_param_seen: bool,
    project_id: Option<i64>,
    limit: Option<usize>,
}

impl RssExtraParams {
    fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut out = Self::default();
        for (k, v) in pairs {
            match k.as_str() {
                "token" => {
                    out.token_param_seen = true;
                    out.token = Some(v.trim().to_string());
                }
                "key" => {
                    out.token_param_seen = true;
                    out.key = Some(v.trim().to_string());
                }
                "username" => {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        out.username = Some(trimmed.to_string());
                    }
                }
                "project_id" => {
                    if let Ok(pid) = v.trim().parse::<i64>() {
                        out.project_id = Some(pid);
                    }
                }
                "limit" => {
                    if let Ok(l) = v.trim().parse::<usize>() {
                        if l > 0 {
                            out.limit = Some(l);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}

fn rss_text_error(status: StatusCode, msg: &str, with_auth_challenge: bool) -> Response {
    if with_auth_challenge {
        (
            status,
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::CACHE_CONTROL, "private, no-store"),
                (header::WWW_AUTHENTICATE, "Bearer"),
            ],
            msg.to_string(),
        )
            .into_response()
    } else {
        (
            status,
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::CACHE_CONTROL, "private, no-store"),
            ],
            msg.to_string(),
        )
            .into_response()
    }
}

fn rss_unauthorized(msg: &str) -> Response {
    rss_text_error(StatusCode::UNAUTHORIZED, msg, true)
}

fn allow_full_query_token() -> bool {
    std::env::var("MANTIS_ALLOW_QUERY_TOKEN")
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false)
}

fn authenticate_rss_request(
    db: &Db,
    headers: &HeaderMap,
    extra: &RssExtraParams,
) -> Result<RssAuthContext, Box<Response>> {
    if let Some(auth_hdr) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let raw = extract_bearer_token(auth_hdr);
        if raw.is_empty() {
            return Err(Box::new(rss_unauthorized("API token required")));
        }
        return match db.authenticate_api_token_with_scope(raw) {
            Ok(Some((_, user, _))) => Ok(RssAuthContext {
                user,
                cookie_project_id: 0,
            }),
            _ => Err(Box::new(rss_unauthorized("Invalid or revoked API token"))),
        };
    }

    if extra.token_param_seen {
        let candidate = extra
            .token
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| extra.key.as_deref().filter(|s| !s.is_empty()))
            .unwrap_or("");
        if candidate.is_empty() {
            return Err(Box::new(rss_unauthorized("RSS feed token required")));
        }
        return match db.authenticate_api_token_with_scope(candidate) {
            Ok(Some((_, user, scope))) => {
                if let Some(ref req_user) = extra.username {
                    if !user.username.eq_ignore_ascii_case(req_user) {
                        return Err(Box::new(rss_unauthorized(
                            "Invalid or revoked RSS feed token",
                        )));
                    }
                }
                if scope != "rss" && !allow_full_query_token() {
                    return Err(Box::new(rss_unauthorized(
                        "Full-access API tokens are disabled in RSS query parameters. Create a read-only RSS token or use 'Authorization: Bearer <token>'.",
                    )));
                }
                Ok(RssAuthContext {
                    user,
                    cookie_project_id: 0,
                })
            }
            _ => Err(Box::new(rss_unauthorized(
                "Invalid or revoked RSS feed token",
            ))),
        };
    }

    let cookies = parse_cookies(headers);
    if let Some(cookie_str) = cookies
        .get("MANTIS_STRING_COOKIE")
        .filter(|s| !s.is_empty())
    {
        if let Ok(Some(user)) = db.get_user_by_cookie(cookie_str) {
            if user.enabled == 1 && !is_session_expired(&user) {
                let mut cookie_project_id = 0;
                if let Some(pid_str) = cookies.get("MANTIS_PROJECT_COOKIE") {
                    if let Ok(pid) = pid_str.parse::<i64>() {
                        if pid > 0 && db.effective_access_level(&user, pid).unwrap_or(0) > 0 {
                            cookie_project_id = pid;
                        }
                    }
                }
                return Ok(RssAuthContext {
                    user,
                    cookie_project_id,
                });
            }
        }
    }

    Err(Box::new(rss_unauthorized(
        "Authentication required for RSS feed",
    )))
}

fn resolve_rss_project_id(
    db: &Db,
    user: &User,
    requested_pid: Option<i64>,
    cookie_pid: i64,
) -> Result<(i64, Option<String>), Box<Response>> {
    if let Some(pid) = requested_pid {
        if pid < 0 {
            return Err(Box::new(rss_text_error(
                StatusCode::BAD_REQUEST,
                "Invalid project_id",
                false,
            )));
        }
        if pid == 0 {
            return Ok((0, None));
        }
        return match db.get_project_by_id(pid) {
            Ok(Some(p)) if p.enabled == 1 => {
                if db.effective_access_level(user, pid).unwrap_or(0) < 10 {
                    Err(Box::new(rss_text_error(
                        StatusCode::FORBIDDEN,
                        "You do not have access to this project.",
                        false,
                    )))
                } else {
                    Ok((pid, Some(p.name)))
                }
            }
            Ok(Some(_)) => Err(Box::new(rss_text_error(
                StatusCode::FORBIDDEN,
                "You do not have access to this project.",
                false,
            ))),
            Ok(None) => Err(Box::new(rss_text_error(
                StatusCode::NOT_FOUND,
                "Project not found.",
                false,
            ))),
            Err(_) => Err(Box::new(rss_text_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load project.",
                false,
            ))),
        };
    }

    if cookie_pid > 0 {
        if let Ok(Some(p)) = db.get_project_by_id(cookie_pid) {
            if p.enabled == 1 && db.effective_access_level(user, cookie_pid).unwrap_or(0) >= 10 {
                return Ok((cookie_pid, Some(p.name)));
            }
        }
        // Stale or disabled project cookie falls back to All Projects (matching AuthContext).
        return Ok((0, None));
    }

    Ok((0, None))
}

fn rss_xml_response(xml: String) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        xml,
    )
        .into_response()
}

pub async fn issues_rss(
    State(db): State<Db>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let pairs = parse_query_pairs(raw_query.as_deref().unwrap_or(""));
    let extra = RssExtraParams::from_pairs(&pairs);
    let auth = match authenticate_rss_request(&db, &headers, &extra) {
        Ok(a) => a,
        Err(res) => return *res,
    };

    let (active_project_id, active_project_name) =
        match resolve_rss_project_id(&db, &auth.user, extra.project_id, auth.cookie_project_id) {
            Ok(v) => v,
            Err(res) => return *res,
        };

    let mut q = ListBugsQuery::from_urlencoded_pairs(&pairs);
    // Keep last project_id=0 aligned between RssExtraParams and ListBugsQuery
    if extra.project_id == Some(0) {
        q.project_id = None;
    } else if let Some(pid) = extra.project_id.filter(|&v| v > 0) {
        q.project_id = Some(pid);
    }

    let filter = match resolve_bug_filter_for_user(&db, &auth.user, active_project_id, &q) {
        Ok(f) => f,
        Err(FilterResolveError::Forbidden(msg)) => {
            return rss_text_error(StatusCode::FORBIDDEN, msg, false);
        }
        Err(FilterResolveError::NotFound(msg)) => {
            return rss_text_error(StatusCode::NOT_FOUND, msg, false);
        }
        Err(FilterResolveError::Internal(msg)) => {
            return rss_text_error(StatusCode::INTERNAL_SERVER_ERROR, msg, false);
        }
    };
    if let Some(final_pid) = filter.project_id.filter(|&v| v > 0) {
        if let Err(res) = resolve_rss_project_id(&db, &auth.user, Some(final_pid), 0) {
            return *res;
        }
    }

    let mut bugs: Vec<Bug> = match db.filter_bugs(&filter, &auth.user) {
        Ok(b) => b,
        Err(_) => {
            return rss_text_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to query issues.",
                false,
            );
        }
    };

    let effective_sort = q
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(filter.sort.as_deref());
    let effective_dir = q
        .dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(filter.dir.as_deref());
    sort_bugs(&mut bugs, effective_sort, effective_dir);

    let limit = extra.limit.unwrap_or(50).clamp(1, 200);
    bugs.truncate(limit);

    let bug_ids: Vec<i64> = bugs.iter().map(|b| b.id).collect();
    let texts_map = db.get_bugs_texts_map(&bug_ids).unwrap_or_default();
    let categories_map = db.list_all_categories_map().unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();
    let projects_map: HashMap<i64, String> = db
        .list_visible_projects(&auth.user)
        .unwrap_or_default()
        .into_iter()
        .map(|(p, _)| (p.id, p.name))
        .collect();

    let base_url = db.base_url().trim_end_matches('/');
    let channel_link = format!("{}/view_all_bug_page", base_url);
    // Strip token/key credentials from self_link so RSS payloads never leak secret PATs
    let mut self_rel = build_issues_rss_url_for(&q, active_project_id);
    if let Some(lim) = extra.limit {
        let sep = if self_rel.contains('?') { '&' } else { '?' };
        self_rel.push_str(&format!("{}limit={}", sep, lim.clamp(1, 200)));
    }
    let self_link = format!("{}{}", base_url, self_rel);

    let channel_title = match active_project_name {
        Some(ref pname) => format!("MantisBT - Issues ({})", pname),
        None => "MantisBT - Issues".to_string(),
    };

    let entries: Vec<IssueRssEntry> = bugs
        .into_iter()
        .map(|b| {
            let desc = texts_map
                .get(&b.id)
                .map(|t| t.0.clone())
                .unwrap_or_default();
            let category = categories_map
                .get(&b.category_id)
                .cloned()
                .unwrap_or_else(|| "General".to_string());
            let project_name = projects_map
                .get(&b.project_id)
                .cloned()
                .unwrap_or_else(|| format!("Project #{}", b.project_id));
            let reporter = users_map
                .get(&b.reporter_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", b.reporter_id));
            let pub_date_ts = if b.last_updated > 0 {
                b.last_updated
            } else {
                b.date_submitted
            };
            IssueRssEntry {
                id: b.id,
                summary: b.summary,
                description: desc,
                category,
                project_name,
                reporter,
                status_label: views::bugs::status_label(b.status).to_string(),
                priority_label: views::bugs::priority_label(b.priority).to_string(),
                severity_label: views::bugs::severity_label(b.severity).to_string(),
                link: format!("{}/view_bug_page/{}", base_url, b.id),
                pub_date_ts,
            }
        })
        .collect();

    rss_xml_response(render_issues_rss(
        &channel_title,
        &channel_link,
        &self_link,
        &entries,
    ))
}

pub async fn news_rss(
    State(db): State<Db>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let pairs = parse_query_pairs(raw_query.as_deref().unwrap_or(""));
    let extra = RssExtraParams::from_pairs(&pairs);
    let auth = match authenticate_rss_request(&db, &headers, &extra) {
        Ok(a) => a,
        Err(res) => return *res,
    };

    let (active_project_id, active_project_name) =
        match resolve_rss_project_id(&db, &auth.user, extra.project_id, auth.cookie_project_id) {
            Ok(v) => v,
            Err(res) => return *res,
        };

    let mut items = match db.list_accessible_news(&auth.user, active_project_id) {
        Ok(list) => list,
        Err(_) => {
            return rss_text_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load news.",
                false,
            );
        }
    };
    let limit = extra.limit.unwrap_or(50).clamp(1, 200);
    items.truncate(limit);

    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();
    let projects_map: HashMap<i64, String> = db
        .list_visible_projects(&auth.user)
        .unwrap_or_default()
        .into_iter()
        .map(|(p, _)| (p.id, p.name))
        .collect();

    let base_url = db.base_url().trim_end_matches('/');
    let channel_link = format!("{}/news_page", base_url);
    let mut self_rel = "/news_rss".to_string();
    let mut self_params = Vec::new();
    if active_project_id > 0 {
        self_params.push(format!("project_id={}", active_project_id));
    }
    if let Some(lim) = extra.limit {
        self_params.push(format!("limit={}", lim.clamp(1, 200)));
    }
    if !self_params.is_empty() {
        self_rel.push('?');
        self_rel.push_str(&self_params.join("&"));
    }
    let self_link = format!("{}{}", base_url, self_rel);

    let channel_title = match active_project_name {
        Some(ref pname) => format!("MantisBT - News ({})", pname),
        None => "MantisBT - News".to_string(),
    };

    let entries: Vec<NewsRssEntry> = items
        .into_iter()
        .map(|item| {
            let project_name = if item.project_id == 0 {
                "All Projects".to_string()
            } else {
                projects_map
                    .get(&item.project_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Project #{}", item.project_id))
            };
            let author = users_map
                .get(&item.poster_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", item.poster_id));
            let pub_date_ts = if item.last_modified > 0 {
                item.last_modified
            } else {
                item.date_posted
            };
            NewsRssEntry {
                id: item.id,
                headline: item.headline,
                body: item.body,
                project_name,
                author,
                link: format!("{}/news_page", base_url),
                pub_date_ts,
                announcement: item.announcement,
            }
        })
        .collect();

    rss_xml_response(render_news_rss(
        &channel_title,
        &channel_link,
        &self_link,
        &entries,
    ))
}

#[cfg(test)]
#[path = "rss_test.rs"]
mod tests;
