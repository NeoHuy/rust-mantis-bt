use crate::auth::AuthContext;
use crate::controllers::parse_bool;
use crate::db::Db;
use crate::models::{
    compute_extended_summary, BillingFilterOptions, BillingIssueSummary, Bug, BugFilter,
    ExtendedSummaryData, TimeTrackingEntry,
};
use crate::views;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct RoadmapQuery {
    pub show_released: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct SummaryQuery {
    pub days: Option<String>,
}

fn parse_summary_days(raw: Option<&str>) -> i64 {
    raw.and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(30)
}

fn summary_access(db: &Db, ctx: &AuthContext) -> Result<bool, Box<Response>> {
    if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return Err(Box::new(forbidden(ctx, "Access denied to this project.")));
        }
        let descendants = db
            .list_project_descendant_ids(ctx.active_project_id)
            .unwrap_or_else(|_| vec![ctx.active_project_id]);
        let mut in_scope = Vec::new();
        for pid in descendants {
            let enabled = matches!(db.get_project_by_id(pid), Ok(Some(p)) if p.enabled != 0);
            if enabled {
                let peff = db.effective_access_level(&ctx.user, pid).unwrap_or(0);
                if peff >= 10 {
                    in_scope.push(peff);
                }
            }
        }
        if in_scope.is_empty() {
            Ok(eff >= 70)
        } else {
            Ok(in_scope.iter().all(|&lvl| lvl >= 70))
        }
    } else {
        let all_mgr = ctx.user.access_level >= 70
            && ctx
                .visible_projects
                .iter()
                .filter(|(p, _)| p.enabled != 0)
                .all(|(p, _)| db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 70);
        Ok(all_mgr)
    }
}

#[derive(Deserialize, Default)]
pub struct BillingQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub include_subprojects: Option<String>,
    pub cost: Option<String>,
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

pub async fn roadmap_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<RoadmapQuery>,
) -> Response {
    let show_released = parse_bool(q.show_released.as_deref());
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
    let categories_map: HashMap<i64, String> = db
        .list_all_categories()
        .unwrap_or_default()
        .into_iter()
        .collect();

    // Determine accessible projects and their names
    let (accessible_project_ids, project_names): (Vec<i64>, HashMap<i64, String>) =
        if ctx.active_project_id > 0 {
            let desc_ids = db
                .list_project_descendant_ids(ctx.active_project_id)
                .unwrap_or_default();
            let mut pids = Vec::new();
            let mut pnames = HashMap::new();
            for pid in desc_ids {
                if db.effective_access_level(&ctx.user, pid).unwrap_or(0)
                    >= crate::models::AccessLevel::Viewer as i32
                {
                    pids.push(pid);
                    if let Ok(Some(proj)) = db.get_project_by_id(pid) {
                        pnames.insert(pid, proj.name);
                    }
                }
            }
            (pids, pnames)
        } else {
            let visible = db.list_visible_projects(&ctx.user).unwrap_or_default();
            let mut pids = Vec::new();
            let mut pnames = HashMap::new();
            for (p, _) in visible {
                pids.push(p.id);
                pnames.insert(p.id, p.name);
            }
            (pids, pnames)
        };

    // Load project versions from DB across all accessible projects
    let mut all_project_versions = Vec::new();
    for pid in &accessible_project_ids {
        if let Ok(mut v_list) = db.list_versions(*pid) {
            all_project_versions.append(&mut v_list);
        }
    }

    let mut version_groups = crate::models::build_roadmap_groups(
        &accessible_project_ids,
        all_project_versions,
        bugs,
        &project_names,
    );

    if !show_released {
        version_groups.retain(|g| !g.released);
    }

    views::planning::render_roadmap_page_with_options(
        &ctx,
        &version_groups,
        &users_map,
        &categories_map,
        show_released,
    )
    .into_response()
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
    let categories_map: HashMap<i64, String> = db
        .list_all_categories()
        .unwrap_or_default()
        .into_iter()
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

    views::planning::render_changelog_page(&ctx, &by_fixed, &users_map, &categories_map)
        .into_response()
}

fn load_extended_summary(db: &Db, ctx: &AuthContext, trend_days: i64) -> ExtendedSummaryData {
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
    let categories_map: HashMap<i64, String> = db
        .list_all_categories()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();

    let mut eff_cache: HashMap<i64, i32> = HashMap::new();
    let bug_access_levels: HashMap<i64, i32> = bugs
        .iter()
        .map(|b| {
            let eff = *eff_cache.entry(b.project_id).or_insert_with(|| {
                db.effective_access_level(&ctx.user, b.project_id)
                    .unwrap_or(0)
            });
            (b.id, eff)
        })
        .collect();
    let bug_ids: Vec<i64> = bugs.iter().map(|b| b.id).collect();
    let (history_counts, resolved_at_map) = db
        .get_bug_history_summary_stats(&bug_ids, &bug_access_levels)
        .unwrap_or_default();
    let now_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    compute_extended_summary(
        &bugs,
        now_ts,
        &projects_map,
        &categories_map,
        &users_map,
        &history_counts,
        &resolved_at_map,
        trend_days,
    )
}

pub async fn summary_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<SummaryQuery>,
) -> Response {
    let show_user_metrics = match summary_access(&db, &ctx) {
        Ok(v) => v,
        Err(res) => return *res,
    };
    let summary = load_extended_summary(&db, &ctx, parse_summary_days(q.days.as_deref()));
    views::planning::render_summary_page_extended(&ctx, &summary, show_user_metrics).into_response()
}

pub async fn summary_graph_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<SummaryQuery>,
) -> Response {
    let show_user_metrics = match summary_access(&db, &ctx) {
        Ok(v) => v,
        Err(res) => return *res,
    };
    let summary = load_extended_summary(&db, &ctx, parse_summary_days(q.days.as_deref()));
    views::planning::render_summary_graph_page(&ctx, &summary, show_user_metrics).into_response()
}

struct BillingReportData {
    entries: Vec<TimeTrackingEntry>,
    by_user: BTreeMap<String, i64>,
    by_issue: Vec<BillingIssueSummary>,
    total_mins: i64,
    opts: BillingFilterOptions,
    warning: Option<String>,
}

fn load_billing_report(
    db: &Db,
    ctx: &AuthContext,
    q: &BillingQuery,
) -> Result<BillingReportData, Box<Response>> {
    let eff = if ctx.active_project_id > 0 {
        db.effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0)
    } else if ctx.user.access_level >= 55 {
        ctx.user.access_level
    } else {
        ctx.visible_projects
            .iter()
            .filter(|(p, _)| p.enabled != 0)
            .filter_map(|(p, _)| db.effective_access_level(&ctx.user, p.id).ok())
            .max()
            .unwrap_or(0)
    };
    if eff < 55 {
        return Err(Box::new(forbidden(
            ctx,
            "Developer access required to view Time Tracking.",
        )));
    }

    let from_raw = q
        .from
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let to_raw =
        q.to.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
    let from_ts = from_raw
        .as_deref()
        .map(views::parse_date_to_epoch)
        .filter(|&t| t > 0);
    let to_ts = to_raw
        .as_deref()
        .map(views::parse_date_to_epoch)
        .filter(|&t| t > 0)
        .map(|t| t + 86_399);
    let from_clean = if from_ts.is_some() {
        from_raw.clone()
    } else {
        None
    };
    let to_clean = if to_ts.is_some() {
        to_raw.clone()
    } else {
        None
    };
    let invalid_date =
        (from_raw.is_some() && from_ts.is_none()) || (to_raw.is_some() && to_ts.is_none());
    let reversed_range = matches!((from_ts, to_ts), (Some(f), Some(t)) if f > t);
    let warning = if invalid_date {
        Some("Invalid date filter ignored; expected YYYY-MM-DD.".to_string())
    } else if reversed_range {
        Some("Start date is after end date; no entries match this range.".to_string())
    } else {
        None
    };

    let include_subprojects = parse_bool(q.include_subprojects.as_deref());
    let hourly_rate = q
        .cost
        .as_deref()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|r| r.is_finite() && *r >= 0.0)
        .map(|r| r.min(1_000_000.0))
        .unwrap_or(0.0);

    let entries = if reversed_range {
        Vec::new()
    } else {
        db.list_time_tracking_filtered(
            &ctx.user,
            ctx.active_project_id,
            include_subprojects,
            from_ts,
            to_ts,
        )
        .unwrap_or_default()
    };

    let mut total_mins = 0i64;
    let mut by_user: BTreeMap<String, i64> = BTreeMap::new();
    let mut by_issue_map: HashMap<i64, BillingIssueSummary> = HashMap::new();

    for e in &entries {
        let mins = i64::from(e.minutes.max(0));
        total_mins = total_mins.saturating_add(mins);
        let u_entry = by_user.entry(e.username.clone()).or_default();
        *u_entry = u_entry.saturating_add(mins);
        by_issue_map
            .entry(e.bug_id)
            .and_modify(|item| item.minutes = item.minutes.saturating_add(mins))
            .or_insert_with(|| BillingIssueSummary {
                bug_id: e.bug_id,
                bug_summary: e.bug_summary.clone(),
                project_name: e.project_name.clone(),
                minutes: mins,
            });
    }

    let mut by_issue: Vec<BillingIssueSummary> = by_issue_map.into_values().collect();
    by_issue.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| a.bug_id.cmp(&b.bug_id))
    });

    Ok(BillingReportData {
        entries,
        by_user,
        by_issue,
        total_mins,
        opts: BillingFilterOptions {
            from: from_clean,
            to: to_clean,
            include_subprojects,
            hourly_rate,
        },
        warning,
    })
}

pub async fn billing_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<BillingQuery>,
) -> Response {
    let report = match load_billing_report(&db, &ctx, &q) {
        Ok(r) => r,
        Err(res) => return *res,
    };

    views::planning::render_billing_page_extended(
        &ctx,
        &report.entries,
        &report.by_user,
        &report.by_issue,
        report.total_mins,
        &report.opts,
        report.warning.as_deref(),
    )
    .into_response()
}

pub async fn billing_export_csv(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<BillingQuery>,
) -> Response {
    let report = match load_billing_report(&db, &ctx, &q) {
        Ok(r) => r,
        Err(res) => return *res,
    };
    let csv = views::planning::render_billing_csv(&report.entries, report.opts.hourly_rate);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"billing.csv\"".to_string(),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        csv,
    )
        .into_response()
}

pub async fn billing_export_excel(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<BillingQuery>,
) -> Response {
    let report = match load_billing_report(&db, &ctx, &q) {
        Ok(r) => r,
        Err(res) => return *res,
    };
    let xml = views::planning::render_billing_excel_xml(&report.entries, report.opts.hourly_rate);
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.ms-excel; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"billing.xml\"".to_string(),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        xml,
    )
        .into_response()
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

    let mut proj_access_cache: std::collections::HashMap<i64, i32> =
        std::collections::HashMap::new();
    let items: Vec<_> = db
        .list_news(ctx.active_project_id, eff)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| {
            if n.project_id == 0 {
                n.view_state == 10 || ctx.user.access_level >= 55
            } else {
                let p_eff = *proj_access_cache.entry(n.project_id).or_insert_with(|| {
                    db.effective_access_level(&ctx.user, n.project_id)
                        .unwrap_or(0)
                });
                p_eff >= 10 && (n.view_state == 10 || p_eff >= 55)
            }
        })
        .collect();
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

#[derive(Deserialize)]
pub struct NewsUpdateForm {
    pub headline: String,
    pub body: String,
    pub announcement: Option<String>,
    pub view_state: Option<i32>,
}

pub async fn news_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to edit news.");
    }
    let item = match db.get_news(id) {
        Ok(Some(item)) => item,
        Ok(None) => return views::render_not_found_page(&ctx, "News item not found"),
        Err(_) => return views::render_not_found_page(&ctx, "News item not found"),
    };
    views::planning::render_news_edit_page(&ctx, &item, q.msg.as_deref(), q.err.as_deref())
        .into_response()
}

pub async fn news_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<NewsUpdateForm>,
) -> Response {
    if ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to update news.");
    }
    match db.get_news(id) {
        Ok(Some(_)) => {}
        Ok(None) => return views::render_not_found_page(&ctx, "News item not found"),
        Err(_) => return views::render_not_found_page(&ctx, "News item not found"),
    }
    let ann = if parse_bool(form.announcement.as_deref()) {
        1
    } else {
        0
    };
    let vs = form.view_state.unwrap_or(10);
    match db.update_news(id, &form.headline, &form.body, vs, ann) {
        Ok(_) => Redirect::to("/news_page?msg=News+updated").into_response(),
        Err(_) => {
            Redirect::to(&format!("/news_edit_page/{}?err=Invalid+news+fields", id)).into_response()
        }
    }
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
    if let Some(mailer) = crate::mailer::SendGridMailer::from_env() {
        match mailer.dispatch_pending_queue(&db).await {
            Ok(count) => {
                let msg = format!("Email+queue+dispatched+({}+sent)", count);
                Redirect::to(&format!("/manage_email_queue_page?msg={}", msg)).into_response()
            }
            Err(e) => {
                let msg = format!("Failed+to+dispatch+emails:+{}", e);
                Redirect::to(&format!("/manage_email_queue_page?msg={}", msg)).into_response()
            }
        }
    } else {
        let _ = db.flush_email_queue();
        Redirect::to("/manage_email_queue_page?msg=Email+queue+flushed").into_response()
    }
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
        .route("/summary_graph_page", get(summary_graph_page))
        .route("/billing_page", get(billing_page))
        .route("/billing_export_csv", get(billing_export_csv))
        .route("/billing_export_excel", get(billing_export_excel))
        .route("/news_page", get(news_page))
        .route("/news_add", post(news_add))
        .route("/news_edit_page/:id", get(news_edit_page))
        .route("/news_update/:id", post(news_update))
        .route("/news_delete/:id", post(news_delete))
        .route("/manage_email_queue_page", get(manage_email_queue_page))
        .route("/manage_email_queue_flush", post(manage_email_queue_flush))
        .route("/manage_email_delete/:id", post(manage_email_delete))
}

#[cfg(test)]
#[path = "planning_test.rs"]
mod tests;
