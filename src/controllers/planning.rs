use crate::auth::AuthContext;
use crate::controllers::parse_bool;
use crate::db::Db;
use crate::models::{Bug, BugFilter};
use crate::views;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

fn forbidden(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Access Denied: {}</div>"#,
        views::html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        views::layout::render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

pub async fn roadmap_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    let filter = BugFilter {
        project_id: if ctx.active_project_id > 0 {
            Some(ctx.active_project_id)
        } else {
            None
        },
        ..Default::default()
    };
    let bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let mut by_target: BTreeMap<String, Vec<Bug>> = BTreeMap::new();
    for b in bugs {
        if !b.target_version.trim().is_empty() {
            by_target
                .entry(b.target_version.trim().to_string())
                .or_default()
                .push(b);
        }
    }

    views::planning::render_roadmap_page(&ctx, &by_target, &users_map).into_response()
}

pub async fn changelog_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    let filter = BugFilter {
        project_id: if ctx.active_project_id > 0 {
            Some(ctx.active_project_id)
        } else {
            None
        },
        ..Default::default()
    };
    let bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let mut by_fixed: BTreeMap<String, Vec<Bug>> = BTreeMap::new();
    for b in bugs {
        if b.status >= 80 && !b.fixed_in_version.trim().is_empty() {
            by_fixed
                .entry(b.fixed_in_version.trim().to_string())
                .or_default()
                .push(b);
        }
    }

    views::planning::render_changelog_page(&ctx, &by_fixed, &users_map).into_response()
}

pub async fn summary_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    let filter = BugFilter {
        project_id: if ctx.active_project_id > 0 {
            Some(ctx.active_project_id)
        } else {
            None
        },
        ..Default::default()
    };
    let bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let mut by_status: BTreeMap<i32, usize> = BTreeMap::new();
    let mut by_severity: BTreeMap<i32, usize> = BTreeMap::new();
    let mut by_priority: BTreeMap<i32, usize> = BTreeMap::new();
    let mut by_reporter: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_handler: BTreeMap<String, usize> = BTreeMap::new();

    let mut open_count = 0usize;
    let mut resolved_count = 0usize;
    let mut closed_count = 0usize;

    for b in &bugs {
        *by_status.entry(b.status).or_default() += 1;
        *by_severity.entry(b.severity).or_default() += 1;
        *by_priority.entry(b.priority).or_default() += 1;

        let rep = users_map
            .get(&b.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", b.reporter_id));
        *by_reporter.entry(rep).or_default() += 1;

        if b.handler_id > 0 {
            let hnd = users_map
                .get(&b.handler_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", b.handler_id));
            *by_handler.entry(hnd).or_default() += 1;
        }

        if b.status == 90 {
            closed_count += 1;
        } else if b.status >= 80 {
            resolved_count += 1;
        } else {
            open_count += 1;
        }
    }

    views::planning::render_summary_page(
        &ctx,
        bugs.len(),
        open_count,
        resolved_count,
        closed_count,
        &by_status,
        &by_severity,
        &by_priority,
        &by_reporter,
        &by_handler,
    )
    .into_response()
}

pub async fn billing_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    let eff = if ctx.active_project_id > 0 {
        db.effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0)
    } else {
        ctx.user.access_level
    };
    if eff < 55 {
        return forbidden(&ctx, "Developer access required to view Time Tracking.");
    }

    let entries = db
        .list_time_tracking(ctx.active_project_id)
        .unwrap_or_default();

    let mut total_mins = 0i32;
    let mut by_user: BTreeMap<String, i32> = BTreeMap::new();

    for e in &entries {
        total_mins += e.minutes;
        *by_user.entry(e.username.clone()).or_default() += e.minutes;
    }

    views::planning::render_billing_page(&ctx, &entries, &by_user, total_mins).into_response()
}

pub async fn news_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    let eff = if ctx.active_project_id > 0 {
        db.effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0)
    } else {
        ctx.user.access_level
    };

    let items = db.list_news(ctx.active_project_id, eff).unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    views::planning::render_news_page(
        &ctx,
        &items,
        &users_map,
        eff,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct NewsAddForm {
    pub project_id: Option<i64>,
    pub headline: String,
    pub body: String,
    pub announcement: Option<String>,
    pub view_state: Option<i32>,
}

pub async fn news_add(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<NewsAddForm>,
) -> Response {
    let pid = form.project_id.unwrap_or(ctx.active_project_id);
    let eff = if pid > 0 {
        db.effective_access_level(&ctx.user, pid).unwrap_or(0)
    } else {
        ctx.user.access_level
    };
    if eff < 70 {
        return forbidden(&ctx, "Manager access required to post news.");
    }

    let ann = parse_bool(form.announcement.as_deref());
    let vs = form.view_state.unwrap_or(10);
    match db.create_news(pid, ctx.user.id, &form.headline, &form.body, ann, vs) {
        Ok(_) => Redirect::to("/news_page?msg=News+posted").into_response(),
        Err(_) => Redirect::to("/news_page?err=Invalid+news+fields").into_response(),
    }
}

pub async fn news_delete(State(db): State<Db>, ctx: AuthContext, Path(id): Path<i64>) -> Response {
    if ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to delete news.");
    }
    let _ = db.delete_news(id);
    Redirect::to("/news_page?msg=News+deleted").into_response()
}

pub async fn manage_email_queue_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required to manage email queue.");
    }

    let emails = db.list_queued_emails().unwrap_or_default();
    views::planning::render_manage_email_queue_page(&ctx, &emails, q.msg.as_deref()).into_response()
}

pub async fn manage_email_queue_flush(State(db): State<Db>, ctx: AuthContext) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.flush_email_queue();
    Redirect::to("/manage_email_queue_page?msg=Email+queue+flushed").into_response()
}

pub async fn manage_email_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.delete_queued_email(id);
    Redirect::to("/manage_email_queue_page?msg=Email+deleted").into_response()
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/roadmap_page", get(roadmap_page))
        .route("/changelog_page", get(changelog_page))
        .route("/summary_page", get(summary_page))
        .route("/billing_page", get(billing_page))
        .route("/news_page", get(news_page))
        .route("/news_add", post(news_add))
        .route("/news_delete/:id", post(news_delete))
        .route("/manage_email_queue_page", get(manage_email_queue_page))
        .route("/manage_email_queue_flush", post(manage_email_queue_flush))
        .route("/manage_email_delete/:id", post(manage_email_delete))
}

#[cfg(test)]
#[path = "planning_test.rs"]
mod tests;
