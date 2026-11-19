use crate::auth::AuthContext;
use crate::models::{
    Bug, BugPriority, BugResolution, BugSeverity, BugStatus, EmailMessage, NewsItem,
    TimeTrackingEntry, ViewState,
};
use crate::views::layout::render_layout;
use crate::views::{format_timestamp, html_escape, render_status_badge};
use axum::response::Html;
use std::collections::{BTreeMap, HashMap};

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

pub fn render_roadmap_page(
    ctx: &AuthContext,
    by_target: &BTreeMap<String, Vec<Bug>>,
    users_map: &HashMap<i64, String>,
) -> Html<String> {
    let mut sections_html = String::new();
    if by_target.is_empty() {
        sections_html.push_str("<p>No roadmap items with a Target Version found.</p>");
    } else {
        for (ver, items) in by_target {
            let total = items.len();
            let resolved = items.iter().filter(|b| b.status >= 80).count();
            let pct = (resolved * 100).checked_div(total).unwrap_or(0);

            let mut list_html = String::new();
            for b in items {
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
                html_escape(ver),
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

    render_layout("Roadmap", ctx, None, &content)
}

pub fn render_changelog_page(
    ctx: &AuthContext,
    by_fixed: &BTreeMap<String, Vec<Bug>>,
    users_map: &HashMap<i64, String>,
) -> Html<String> {
    let mut sections_html = String::new();
    if by_fixed.is_empty() {
        sections_html.push_str("<p>No resolved issues with a Fixed in Version found.</p>");
    } else {
        for (ver, items) in by_fixed {
            let mut list_html = String::new();
            for b in items {
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
                html_escape(ver),
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

    render_layout("Changelog", ctx, None, &content)
}

#[allow(clippy::too_many_arguments)]
pub fn render_summary_page(
    ctx: &AuthContext,
    total_bugs: usize,
    open_count: usize,
    resolved_count: usize,
    closed_count: usize,
    by_status: &BTreeMap<i32, usize>,
    by_severity: &BTreeMap<i32, usize>,
    by_priority: &BTreeMap<i32, usize>,
    by_reporter: &BTreeMap<String, usize>,
    by_handler: &BTreeMap<String, usize>,
) -> Html<String> {
    let mut status_rows = String::new();
    for (&st, &cnt) in by_status {
        status_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            render_status_badge(st, status_label(st)),
            cnt
        ));
    }

    let mut sev_rows = String::new();
    for (&sev, &cnt) in by_severity {
        sev_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(severity_label(sev)),
            cnt
        ));
    }

    let mut pri_rows = String::new();
    for (&pri, &cnt) in by_priority {
        pri_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(priority_label(pri)),
            cnt
        ));
    }

    let mut rep_rows = String::new();
    for (rep, &cnt) in by_reporter {
        rep_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(rep),
            cnt
        ));
    }

    let mut hnd_rows = String::new();
    for (hnd, &cnt) in by_handler {
        hnd_rows.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td></tr>"#,
            html_escape(hnd),
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
        total_bugs,
        open_count,
        resolved_count,
        closed_count,
        status_rows,
        sev_rows,
        pri_rows,
        rep_rows,
        hnd_rows
    );

    render_layout("Summary", ctx, None, &content)
}

pub fn render_billing_page(
    ctx: &AuthContext,
    entries: &[TimeTrackingEntry],
    by_user: &BTreeMap<String, i32>,
    total_mins: i32,
) -> Html<String> {
    let mut rows_html = String::new();
    for e in entries {
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
            html_escape(u),
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

    render_layout("Time Tracking", ctx, None, &content)
}

pub fn render_news_page(
    ctx: &AuthContext,
    items: &[NewsItem],
    users_map: &HashMap<i64, String>,
    eff: i32,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let mut list_html = String::new();
    if items.is_empty() {
        list_html.push_str("<p>No news announcements posted yet.</p>");
    } else {
        for n in items {
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
                        {}{} <span style="font-weight:normal; font-size:12px;">— posted by {} on {} [{}]</span>
                        {}
                    </div>
                    <div class="panel-body" style="white-space:pre-wrap;">{}</div>
                </div>"#,
                html_escape(&n.headline),
                ann_badge,
                html_escape(&poster),
                format_timestamp(n.date_posted),
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

    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <div class="section-title">Project News</div>
        {}
        {}"#,
        err_banner, list_html, add_form_html
    );

    render_layout("News", ctx, msg, &content)
}

pub fn render_manage_email_queue_page(
    ctx: &AuthContext,
    emails: &[EmailMessage],
    msg: Option<&str>,
) -> Html<String> {
    let mut rows_html = String::new();
    for e in emails {
        rows_html.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td style="white-space:pre-wrap;">{}</td>
                <td style="white-space:nowrap;">{}</td>
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
            format_timestamp(e.submitted),
            e.email_id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="6">Email queue is empty.</td></tr>"#);
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
                            <th>Submitted</th>
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

    render_layout("Email Queue", ctx, msg, &content)
}

#[cfg(test)]
#[path = "planning_test.rs"]
mod tests;
