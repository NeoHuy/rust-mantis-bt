use crate::auth::AuthContext;
use crate::db::Db;
use crate::handlers::parse_bool;
use crate::models::{BugFilter, BugPriority, BugResolution, BugSeverity, BugStatus, ViewState};
use crate::ui::{html_escape, render_layout};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
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
        html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

fn status_label(v: i32) -> &'static str {
    BugStatus::from_i32(v).map(|s| s.as_str()).unwrap_or("new")
}

fn priority_label(v: i32) -> &'static str {
    BugPriority::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("normal")
}

fn severity_label(v: i32) -> &'static str {
    BugSeverity::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("minor")
}

fn resolution_label(v: i32) -> &'static str {
    BugResolution::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("open")
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

    let mut by_target: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for b in bugs {
        if !b.target_version.trim().is_empty() {
            by_target
                .entry(b.target_version.trim().to_string())
                .or_default()
                .push(b);
        }
    }

    let mut sections_html = String::new();
    if by_target.is_empty() {
        sections_html.push_str("<p>No roadmap items with a Target Version found.</p>");
    } else {
        for (ver, items) in by_target {
            let total = items.len();
            let resolved = items.iter().filter(|b| b.status >= 80).count();
            let pct = if total > 0 {
                (resolved * 100) / total
            } else {
                0
            };

            let mut list_html = String::new();
            for b in &items {
                let handler_str = if b.handler_id > 0 {
                    format!(
                        " ({})",
                        html_escape(
                            users_map
                                .get(&b.handler_id)
                                .map(|s| s.as_str())
                                .unwrap_or("assigned")
                        )
                    )
                } else {
                    String::new()
                };
                let strike_open = if b.status >= 80 { "<s>" } else { "" };
                let strike_close = if b.status >= 80 { "</s>" } else { "" };
                list_html.push_str(&format!(
                    r#"<li style="margin-bottom:4px;">{strike_open}<a href="/view_bug_page/{id}">{id:07}</a>: <strong>{summary}</strong>{strike_close} - [{status}]{handler}</li>"#,
                    strike_open = strike_open,
                    strike_close = strike_close,
                    id = b.id,
                    summary = html_escape(&b.summary),
                    status = html_escape(status_label(b.status)),
                    handler = handler_str
                ));
            }

            sections_html.push_str(&format!(
                r#"<div class="panel">
                    <div class="panel-header">Target Version: {} — {} / {} resolved ({}%)</div>
                    <div class="panel-body">
                        <div style="background:#e0e6ed; height:12px; border-radius:6px; overflow:hidden; margin-bottom:12px; max-width:400px;">
                            <div style="background:#5cb85c; width:{}%; height:100%;"></div>
                        </div>
                        <ul style="padding-left:20px;">
                            {}
                        </ul>
                    </div>
                </div>"#,
                html_escape(&ver),
                resolved,
                total,
                pct,
                pct,
                list_html
            ));
        }
    }

    let content = format!(
        r#"<div class="section-title">Roadmap</div>
        {}"#,
        sections_html
    );

    render_layout("Roadmap", &ctx, None, &content).into_response()
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

    let mut by_fixed: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for b in bugs {
        if b.status >= 80 && !b.fixed_in_version.trim().is_empty() {
            by_fixed
                .entry(b.fixed_in_version.trim().to_string())
                .or_default()
                .push(b);
        }
    }

    let mut sections_html = String::new();
    if by_fixed.is_empty() {
        sections_html.push_str("<p>No resolved issues with a Fixed in Version found.</p>");
    } else {
        for (ver, items) in by_fixed {
            let mut list_html = String::new();
            for b in &items {
                let handler_str = if b.handler_id > 0 {
                    format!(
                        " ({})",
                        html_escape(
                            users_map
                                .get(&b.handler_id)
                                .map(|s| s.as_str())
                                .unwrap_or("assigned")
                        )
                    )
                } else {
                    String::new()
                };
                list_html.push_str(&format!(
                    r#"<li style="margin-bottom:4px;"><a href="/view_bug_page/{id}">{id:07}</a>: <strong>{summary}</strong> - [{resolution}]{handler}</li>"#,
                    id = b.id,
                    summary = html_escape(&b.summary),
                    resolution = html_escape(resolution_label(b.resolution)),
                    handler = handler_str
                ));
            }

            sections_html.push_str(&format!(
                r#"<div class="panel">
                    <div class="panel-header">Version: {} ({} issues fixed)</div>
                    <div class="panel-body">
                        <ul style="padding-left:20px;">
                            {}
                        </ul>
                    </div>
                </div>"#,
                html_escape(&ver),
                items.len(),
                list_html
            ));
        }
    }

    let content = format!(
        r#"<div class="section-title">Changelog</div>
        {}"#,
        sections_html
    );

    render_layout("Changelog", &ctx, None, &content).into_response()
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

    let mut status_rows = String::new();
    for (st, cnt) in by_status {
        status_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(status_label(st)),
            cnt
        ));
    }

    let mut sev_rows = String::new();
    for (sev, cnt) in by_severity {
        sev_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(severity_label(sev)),
            cnt
        ));
    }

    let mut pri_rows = String::new();
    for (pri, cnt) in by_priority {
        pri_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(priority_label(pri)),
            cnt
        ));
    }

    let mut rep_rows = String::new();
    for (rep, cnt) in by_reporter {
        rep_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(&rep),
            cnt
        ));
    }

    let mut hnd_rows = String::new();
    for (hnd, cnt) in by_handler {
        hnd_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(&hnd),
            cnt
        ));
    }

    let content = format!(
        r#"<div class="section-title">Summary Statistics</div>
        <div class="panel">
            <div class="panel-header">Overview (Total Issues: {})</div>
            <div class="panel-body">
                <p><strong>Open:</strong> {} &nbsp;|&nbsp; <strong>Resolved:</strong> {} &nbsp;|&nbsp; <strong>Closed:</strong> {}</p>
            </div>
        </div>
        <div style="display:grid; grid-template-columns: 1fr 1fr; gap:16px;">
            <div class="panel">
                <div class="panel-header">By Status</div>
                <div class="panel-body">
                    <table class="data-table"><thead><tr><th>Status</th><th>Count</th></tr></thead><tbody>{}</tbody></table>
                </div>
            </div>
            <div class="panel">
                <div class="panel-header">By Severity</div>
                <div class="panel-body">
                    <table class="data-table"><thead><tr><th>Severity</th><th>Count</th></tr></thead><tbody>{}</tbody></table>
                </div>
            </div>
            <div class="panel">
                <div class="panel-header">By Priority</div>
                <div class="panel-body">
                    <table class="data-table"><thead><tr><th>Priority</th><th>Count</th></tr></thead><tbody>{}</tbody></table>
                </div>
            </div>
            <div class="panel">
                <div class="panel-header">By Reporter / Developer</div>
                <div class="panel-body">
                    <h4>By Reporter</h4>
                    <table class="data-table"><thead><tr><th>Reporter</th><th>Count</th></tr></thead><tbody>{}</tbody></table>
                    <h4>By Handler</h4>
                    <table class="data-table"><thead><tr><th>Handler</th><th>Count</th></tr></thead><tbody>{}</tbody></table>
                </div>
            </div>
        </div>"#,
        bugs.len(),
        open_count,
        resolved_count,
        closed_count,
        status_rows,
        sev_rows,
        pri_rows,
        rep_rows,
        hnd_rows
    );

    render_layout("Summary", &ctx, None, &content).into_response()
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
    let mut rows_html = String::new();

    for e in &entries {
        total_mins += e.minutes;
        *by_user.entry(e.username.clone()).or_default() += e.minutes;
        rows_html.push_str(&format!(
            r#"<tr>
                <td><a href="/view_bug_page/{id}">{id:07}</a>: {summary}</td>
                <td>{user}</td>
                <td>{hh:02}:{mm:02} ({mins} mins)</td>
                <td>{note}</td>
            </tr>"#,
            id = e.bug_id,
            summary = html_escape(&e.bug_summary),
            user = html_escape(&e.username),
            hh = e.minutes / 60,
            mm = e.minutes % 60,
            mins = e.minutes,
            note = html_escape(&e.note)
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="4">No time tracking entries recorded.</td></tr>"#);
    }

    let mut user_rows = String::new();
    for (u, m) in by_user {
        user_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{:02}:{:02} ({} mins)</td></tr>"#,
            html_escape(&u),
            m / 60,
            m % 60,
            m
        ));
    }

    let content = format!(
        r#"<div class="section-title">Time Tracking / Billing</div>
        <div class="panel">
            <div class="panel-header">Summary by User (Total: {:02}:{:02} / {} mins)</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead><tr><th>User</th><th>Time Spent</th></tr></thead>
                    <tbody>{}</tbody>
                </table>
            </div>
        </div>
        <div class="panel">
            <div class="panel-header">Detailed Time Entries</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead><tr><th>Issue</th><th>User</th><th>Duration</th><th>Note</th></tr></thead>
                    <tbody>{}</tbody>
                </table>
            </div>
        </div>"#,
        total_mins / 60,
        total_mins % 60,
        total_mins,
        user_rows,
        rows_html
    );

    render_layout("Time Tracking", &ctx, None, &content).into_response()
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

    let items = db
        .list_news(ctx.active_project_id, eff)
        .unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let mut list_html = String::new();
    if items.is_empty() {
        list_html.push_str("<p>No news announcements posted yet.</p>");
    } else {
        for n in &items {
            let poster = users_map
                .get(&n.poster_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", n.poster_id));
            let vs_str = ViewState::from_i32(n.view_state)
                .map(|v| v.as_str())
                .unwrap_or("public");
            let ann_badge = if n.announcement != 0 {
                r#" <span class="badge badge-warning">Announcement</span>"#
            } else {
                ""
            };
            let del_form = if eff >= 70 {
                format!(
                    r#"<form method="POST" action="/news_delete/{}" style="display:inline; float:right;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    n.id
                )
            } else {
                String::new()
            };
            list_html.push_str(&format!(
                r#"<div class="panel">
                    <div class="panel-header">
                        {}{} <span style="font-weight:normal; font-size:12px;">— posted by {} [{}]</span>
                        {}
                    </div>
                    <div class="panel-body" style="white-space:pre-wrap;">{}</div>
                </div>"#,
                html_escape(&n.headline),
                ann_badge,
                html_escape(&poster),
                vs_str,
                del_form,
                html_escape(&n.body)
            ));
        }
    }

    let add_form_html = if eff >= 70 {
        format!(
            r#"<div class="panel">
                <div class="panel-header">Post News Item</div>
                <div class="panel-body">
                    <form method="POST" action="/news_add">
                        <input type="hidden" name="project_id" value="{}">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Headline *</td>
                                <td class="input-col"><input type="text" name="headline" required></td>
                            </tr>
                            <tr>
                                <td class="label-col">Body *</td>
                                <td class="input-col"><textarea name="body" required></textarea></td>
                            </tr>
                            <tr>
                                <td class="label-col">Announcement</td>
                                <td class="input-col"><input type="checkbox" name="announcement" value="1"> Pin to top</td>
                            </tr>
                            <tr>
                                <td class="label-col">View Status</td>
                                <td class="input-col">
                                    <select name="view_state" style="width:auto;">
                                        <option value="10">public</option>
                                        <option value="50">private</option>
                                    </select>
                                </td>
                            </tr>
                        </table>
                        <button type="submit" class="btn">Post News</button>
                    </form>
                </div>
            </div>"#,
            ctx.active_project_id
        )
    } else {
        String::new()
    };

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <div class="section-title">Project News</div>
        {}
        {}"#,
        err_banner, list_html, add_form_html
    );

    render_layout("News", &ctx, q.msg.as_deref(), &content).into_response()
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

pub async fn news_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
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
    let mut rows_html = String::new();
    for e in &emails {
        rows_html.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td style="white-space:pre-wrap;">{}</td>
                <td>
                    <form method="POST" action="/manage_email_delete/{}" style="display:inline;">
                        <button type="submit" class="btn btn-danger btn-sm">Delete</button>
                    </form>
                </td>
            </tr>"#,
            e.email_id,
            html_escape(&e.email),
            html_escape(&e.subject),
            html_escape(&e.body),
            e.email_id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="5">Email queue is empty.</td></tr>"#);
    }

    let content = format!(
        r#"<div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Email Notification Queue ({})</span>
                <form method="POST" action="/manage_email_queue_flush" style="display:inline; margin:0;">
                    <button type="submit" class="btn btn-sm">Send / Flush Queue</button>
                </form>
            </div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Recipient</th>
                            <th>Subject</th>
                            <th>Body</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>"#,
        emails.len(),
        rows_html
    );

    render_layout("Email Queue", &ctx, q.msg.as_deref(), &content).into_response()
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
