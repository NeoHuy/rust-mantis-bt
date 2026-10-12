use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{
    BugPriority, BugReproducibility, BugResolution, BugSeverity, BugStatus, BugUpdate,
    RelationshipType, ViewState,
};
use crate::ui::{html_escape, render_layout};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct ListBugsQuery {
    pub page: Option<i64>,
    pub status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub handler_id: Option<i64>,
    pub msg: Option<String>,
    pub err: Option<String>,
}

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

fn not_found(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Not Found: {}</div>"#,
        html_escape(msg)
    );
    (
        StatusCode::NOT_FOUND,
        render_layout("Not Found", ctx, None, &body),
    )
        .into_response()
}

fn status_label(v: i32) -> &'static str {
    BugStatus::from_i32(v).map(|s| s.as_str()).unwrap_or("unknown")
}

fn priority_label(v: i32) -> &'static str {
    BugPriority::from_i32(v).map(|p| p.as_str()).unwrap_or("unknown")
}

fn severity_label(v: i32) -> &'static str {
    BugSeverity::from_i32(v).map(|s| s.as_str()).unwrap_or("unknown")
}

fn reproducibility_label(v: i32) -> &'static str {
    BugReproducibility::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("unknown")
}

fn resolution_label(v: i32) -> &'static str {
    BugResolution::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("unknown")
}

fn relationship_label(v: i32) -> &'static str {
    RelationshipType::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("related to")
}

fn max_file_size() -> usize {
    std::env::var("MANTIS_MAX_FILE_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5_242_880)
}

fn upload_dir() -> String {
    std::env::var("MANTIS_UPLOAD_DIR").unwrap_or_default()
}

pub async fn view_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<ListBugsQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 25;
    let offset = (page - 1) * per_page;

    let mut bugs = if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return forbidden(&ctx, "You do not have access to this project.");
        }
        db.list_bugs(ctx.active_project_id, eff, 0, 10_000)
            .unwrap_or_default()
    } else {
        let all = db.list_bugs(0, 90, 0, 10_000).unwrap_or_default();
        all.into_iter()
            .filter(|b| {
                let eff = db
                    .effective_access_level(&ctx.user, b.project_id)
                    .unwrap_or(0);
                eff >= 10 && (b.view_state == 10 || eff >= 55)
            })
            .collect()
    };

    if let Some(st) = q.status {
        if st > 0 {
            bugs.retain(|b| b.status == st);
        }
    }
    if let Some(pr) = q.priority {
        if pr > 0 {
            bugs.retain(|b| b.priority == pr);
        }
    }
    if let Some(sev) = q.severity {
        if sev > 0 {
            bugs.retain(|b| b.severity == sev);
        }
    }
    if let Some(hid) = q.handler_id {
        if hid > 0 {
            bugs.retain(|b| b.handler_id == hid);
        }
    }

    let total = bugs.len();
    let paged_bugs: Vec<_> = bugs
        .into_iter()
        .skip(offset as usize)
        .take(per_page as usize)
        .collect();

    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();

    let mut rows_html = String::new();
    if paged_bugs.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="9" style="text-align:center; padding:16px;">No issues found.</td></tr>"#,
        );
    } else {
        for (idx, b) in paged_bugs.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let proj_name = projects_map
                .get(&b.project_id)
                .cloned()
                .or_else(|| {
                    db.get_project_by_id(b.project_id)
                        .ok()
                        .flatten()
                        .map(|p| p.name)
                })
                .unwrap_or_else(|| format!("#{}", b.project_id));
            let cat_name = db
                .get_category_by_id(b.category_id)
                .ok()
                .flatten()
                .map(|c| c.name)
                .unwrap_or_else(|| "General".to_string());
            let handler_name = if b.handler_id > 0 {
                users_map
                    .get(&b.handler_id)
                    .cloned()
                    .unwrap_or_else(|| format!("user#{}", b.handler_id))
            } else {
                String::new()
            };
            let priv_badge = if b.view_state == 50 {
                r#" <span class="badge badge-warning">private</span>"#
            } else {
                ""
            };

            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><a href="/view_bug_page/{}">{:07}</a></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}{}{}</td>
                    <td><a href="/view_bug_page/{}">{}</a>{}</td>
                    <td>{}</td>
                </tr>"#,
                row_class,
                b.id,
                b.id,
                html_escape(&proj_name),
                html_escape(&cat_name),
                html_escape(severity_label(b.severity)),
                html_escape(priority_label(b.priority)),
                html_escape(status_label(b.status)),
                if !handler_name.is_empty() { " (" } else { "" },
                if !handler_name.is_empty() {
                    format!("{})", html_escape(&handler_name))
                } else {
                    String::new()
                },
                b.id,
                html_escape(&b.summary),
                priv_badge,
                b.last_updated
            ));
        }
    }

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let prev_link = if page > 1 {
        format!(
            r#"<a class="btn btn-secondary btn-sm" href="/view_all_bug_page?page={}">« Prev</a>"#,
            page - 1
        )
    } else {
        String::new()
    };
    let next_link = if (offset as usize) + paged_bugs.len() < total {
        format!(
            r#"<a class="btn btn-secondary btn-sm" href="/view_all_bug_page?page={}">Next »</a>"#,
            page + 1
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Viewing Issues ({})</span>
                <a class="btn btn-sm" href="/bug_report_page">Report Issue</a>
            </div>
            <div class="panel-body">
                <form method="GET" action="/view_all_bug_page" style="display:flex; gap:10px; align-items:center; margin-bottom:12px; flex-wrap:wrap;">
                    <label>Status:
                        <select name="status" style="width:auto;">
                            <option value="0">All</option>
                            <option value="10">new</option>
                            <option value="20">feedback</option>
                            <option value="30">acknowledged</option>
                            <option value="40">confirmed</option>
                            <option value="50">assigned</option>
                            <option value="80">resolved</option>
                            <option value="90">closed</option>
                        </select>
                    </label>
                    <label>Priority:
                        <select name="priority" style="width:auto;">
                            <option value="0">All</option>
                            <option value="10">none</option>
                            <option value="20">low</option>
                            <option value="30">normal</option>
                            <option value="40">high</option>
                            <option value="50">urgent</option>
                            <option value="60">immediate</option>
                        </select>
                    </label>
                    <label>Severity:
                        <select name="severity" style="width:auto;">
                            <option value="0">All</option>
                            <option value="10">feature</option>
                            <option value="20">trivial</option>
                            <option value="30">text</option>
                            <option value="40">tweak</option>
                            <option value="50">minor</option>
                            <option value="60">major</option>
                            <option value="70">crash</option>
                            <option value="80">block</option>
                        </select>
                    </label>
                    <button type="submit" class="btn btn-secondary btn-sm">Apply Filter</button>
                </form>
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Project</th>
                            <th>Category</th>
                            <th>Severity</th>
                            <th>Priority</th>
                            <th>Status</th>
                            <th>Summary</th>
                            <th>Updated</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
                <div style="display:flex; gap:8px; justify-content:flex-end;">
                    {} {}
                </div>
            </div>
        </div>"#,
        err_banner, total, rows_html, prev_link, next_link
    );

    render_layout("View Issues", &ctx, q.msg.as_deref(), &content).into_response()
}

pub async fn bug_report_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    let default_proj_id = if ctx.active_project_id > 0 {
        ctx.active_project_id
    } else {
        ctx.visible_projects
            .first()
            .map(|(p, _)| p.id)
            .unwrap_or(0)
    };

    if default_proj_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, default_proj_id)
            .unwrap_or(0);
        if eff < 25 {
            return forbidden(&ctx, "Reporter access required to report an issue.");
        }
    } else if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to report an issue.");
    }

    let cats = db
        .list_effective_categories(default_proj_id)
        .unwrap_or_default();
    let versions = if default_proj_id > 0 {
        db.list_versions(default_proj_id).unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut proj_opts = String::new();
    for (p, _) in &ctx.visible_projects {
        let eff = db.effective_access_level(&ctx.user, p.id).unwrap_or(0);
        if eff >= 25 {
            let sel = if p.id == default_proj_id {
                "selected"
            } else {
                ""
            };
            proj_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                p.id,
                sel,
                html_escape(&p.name)
            ));
        }
    }

    let mut cat_opts = String::new();
    for c in &cats {
        cat_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            c.id,
            html_escape(&c.name)
        ));
    }

    let mut ver_opts = String::from(r#"<option value="">(none)</option>"#);
    for v in &versions {
        ver_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            html_escape(&v.version),
            html_escape(&v.version)
        ));
    }

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Enter Issue Details</div>
            <div class="panel-body">
                <form method="POST" action="/bug_report">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Project</td>
                            <td class="input-col"><select name="project_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Category</td>
                            <td class="input-col"><select name="category_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Reproducibility</td>
                            <td class="input-col">
                                <select name="reproducibility">
                                    <option value="10">always</option>
                                    <option value="30">sometimes</option>
                                    <option value="50">random</option>
                                    <option value="70" selected>have not tried</option>
                                    <option value="90">unable to reproduce</option>
                                    <option value="100">N/A</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Severity</td>
                            <td class="input-col">
                                <select name="severity">
                                    <option value="10">feature</option>
                                    <option value="20">trivial</option>
                                    <option value="30">text</option>
                                    <option value="40">tweak</option>
                                    <option value="50" selected>minor</option>
                                    <option value="60">major</option>
                                    <option value="70">crash</option>
                                    <option value="80">block</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority</td>
                            <td class="input-col">
                                <select name="priority">
                                    <option value="10">none</option>
                                    <option value="20">low</option>
                                    <option value="30" selected>normal</option>
                                    <option value="40">high</option>
                                    <option value="50">urgent</option>
                                    <option value="60">immediate</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Product Version</td>
                            <td class="input-col"><select name="version">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Platform / OS / Build</td>
                            <td class="input-col">
                                <input type="text" name="platform" placeholder="Platform" style="max-width:120px;">
                                <input type="text" name="os" placeholder="OS" style="max-width:120px;">
                                <input type="text" name="os_build" placeholder="OS Build" style="max-width:120px;">
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary *</td>
                            <td class="input-col"><input type="text" name="summary" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description *</td>
                            <td class="input-col"><textarea name="description"></textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Steps To Reproduce</td>
                            <td class="input-col"><textarea name="steps_to_reproduce"></textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Additional Information</td>
                            <td class="input-col"><textarea name="additional_information"></textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" selected>public</option>
                                    <option value="50">private</option>
                                </select>
                            </td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Submit Issue</button>
                </form>
            </div>
        </div>"#,
        err_banner, proj_opts, cat_opts, ver_opts
    );

    render_layout("Report Issue", &ctx, q.msg.as_deref(), &content).into_response()
}

#[derive(Deserialize)]
pub struct BugReportForm {
    pub project_id: Option<i64>,
    pub category_id: Option<i64>,
    pub reproducibility: Option<i32>,
    pub severity: Option<i32>,
    pub priority: Option<i32>,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub os: Option<String>,
    pub os_build: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub steps_to_reproduce: Option<String>,
    pub additional_information: Option<String>,
    pub view_state: Option<i32>,
}

pub async fn bug_report(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<BugReportForm>,
) -> Response {
    let project_id = form.project_id.unwrap_or(ctx.active_project_id);
    if project_id <= 0 {
        return Redirect::to("/bug_report_page?err=Please+select+a+valid+project").into_response();
    }

    let eff = db
        .effective_access_level(&ctx.user, project_id)
        .unwrap_or(0);
    if eff < 25 {
        return forbidden(&ctx, "Reporter access required to create an issue.");
    }

    let category_id = form.category_id.unwrap_or(1);
    let priority = form.priority.unwrap_or(30);
    let severity = form.severity.unwrap_or(50);
    let reproducibility = form.reproducibility.unwrap_or(70);
    let view_state = if form.view_state == Some(50) { 50 } else { 10 };

    match db.create_bug(
        project_id,
        ctx.user.id,
        category_id,
        form.summary.as_deref().unwrap_or(""),
        form.description.as_deref().unwrap_or(""),
        form.steps_to_reproduce.as_deref().unwrap_or(""),
        form.additional_information.as_deref().unwrap_or(""),
        priority,
        severity,
        reproducibility,
        form.version.as_deref().unwrap_or(""),
        form.os.as_deref().unwrap_or(""),
        form.os_build.as_deref().unwrap_or(""),
        form.platform.as_deref().unwrap_or(""),
        view_state,
    ) {
        Ok(bug_id) => Redirect::to(&format!("/view_bug_page/{}", bug_id)).into_response(),
        Err(_) => Redirect::to("/bug_report_page?err=Invalid+bug+fields").into_response(),
    }
}

pub async fn view_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let bug = &full.bug;

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 10 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "You do not have permission to view this issue.");
    }

    let proj_name = db
        .get_project_by_id(bug.project_id)
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_else(|| format!("#{}", bug.project_id));
    let cat_name = db
        .get_category_by_id(bug.category_id)
        .ok()
        .flatten()
        .map(|c| c.name)
        .unwrap_or_else(|| "General".to_string());

    let users = db.list_users(None, false).unwrap_or_default();
    let users_map: HashMap<i64, String> = users
        .iter()
        .map(|u| (u.id, u.username.clone()))
        .collect();

    let reporter_name = users_map
        .get(&bug.reporter_id)
        .cloned()
        .unwrap_or_else(|| format!("user#{}", bug.reporter_id));
    let handler_name = if bug.handler_id > 0 {
        users_map
            .get(&bug.handler_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", bug.handler_id))
    } else {
        "Unassigned".to_string()
    };

    let notes = db.list_bugnotes(id, eff).unwrap_or_default();
    let history = db.list_bug_history(id).unwrap_or_default();
    let rels = db.list_bug_relationships(id).unwrap_or_default();
    let files = db.list_bug_files(id).unwrap_or_default();

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    // Quick Action Toolbar (separate forms, no nesting!)
    let mut actions_html = String::new();
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == ctx.user.id);
    if can_edit {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-sm" href="/update_bug_page/{}">Edit</a>"#,
            bug.id
        ));
    }

    if eff >= 40 {
        let mut status_opts = String::new();
        for (code, name) in [
            (10, "new"),
            (20, "feedback"),
            (30, "acknowledged"),
            (40, "confirmed"),
            (50, "assigned"),
            (80, "resolved"),
            (90, "closed"),
        ] {
            if code == 80 && eff < 55 {
                continue;
            }
            if code == 90 && eff < 70 {
                continue;
            }
            let sel = if bug.status == code { "selected" } else { "" };
            status_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                code, sel, name
            ));
        }

        let mut res_opts = String::new();
        for (code, name) in [
            (10, "open"),
            (20, "fixed"),
            (30, "reopened"),
            (40, "unable to reproduce"),
            (50, "not fixable"),
            (60, "duplicate"),
            (70, "not a bug"),
            (80, "suspended"),
            (90, "won't fix"),
        ] {
            let sel = if bug.resolution == code {
                "selected"
            } else {
                ""
            };
            res_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                code, sel, name
            ));
        }

        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_change_status/{}" style="display:inline-flex; gap:4px; align-items:center;">
                <select name="new_status" style="width:auto;">{}</select>
                <select name="resolution" style="width:auto;">{}</select>
                <input type="text" name="note" placeholder="Optional status note" style="width:150px;">
                <button type="submit" class="btn btn-secondary btn-sm">Change Status</button>
            </form>"#,
            bug.id, status_opts, res_opts
        ));
    }

    if eff >= 55 {
        let mut user_opts = String::from(r#"<option value="0">(Unassigned)</option>"#);
        for u in &users {
            let sel = if u.id == bug.handler_id {
                "selected"
            } else {
                ""
            };
            user_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                u.id,
                sel,
                html_escape(&u.username)
            ));
        }
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_assign/{}" style="display:inline-flex; gap:4px; align-items:center;">
                <select name="handler_id" style="width:auto;">{}</select>
                <button type="submit" class="btn btn-secondary btn-sm">Assign To</button>
            </form>"#,
            bug.id, user_opts
        ));
    }

    if eff >= 70 {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_delete/{}" style="display:inline;">
                <button type="submit" class="btn btn-danger btn-sm">Delete Issue</button>
            </form>"#,
            bug.id
        ));
    }

    // Attachments section
    let mut files_html = String::new();
    if files.is_empty() {
        files_html.push_str("<p>No attachments.</p>");
    } else {
        files_html.push_str(r#"<table class="data-table"><thead><tr><th>Filename</th><th>Size</th><th>Type</th><th>Actions</th></tr></thead><tbody>"#);
        for f in &files {
            let del_btn = if eff >= 70 {
                format!(
                    r#"<form method="POST" action="/bug_file_delete/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    f.id
                )
            } else {
                String::new()
            };
            files_html.push_str(&format!(
                r#"<tr>
                    <td><a href="/file/bug/{}/{}">{}</a></td>
                    <td>{} bytes</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                f.id,
                html_escape(&f.filename),
                html_escape(&f.filename),
                f.filesize,
                html_escape(&f.file_type),
                del_btn
            ));
        }
        files_html.push_str("</tbody></table>");
    }

    let upload_form_html = if eff >= 25 {
        format!(
            r#"<form method="POST" action="/bug_file_add/{}" enctype="multipart/form-data" style="margin-top:10px; display:flex; gap:8px; align-items:center;">
                <input type="file" name="file" required>
                <button type="submit" class="btn btn-secondary btn-sm">Upload File</button>
            </form>"#,
            bug.id
        )
    } else {
        String::new()
    };

    // Relationships section
    let mut rels_html = String::new();
    if rels.is_empty() {
        rels_html.push_str("<p>No relationships.</p>");
    } else {
        rels_html.push_str(r#"<table class="data-table"><thead><tr><th>Relationship</th><th>Target Issue</th><th>Actions</th></tr></thead><tbody>"#);
        for r in &rels {
            let del_btn = if eff >= 55 {
                format!(
                    r#"<form method="POST" action="/bug_relationship_delete/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    r.id
                )
            } else {
                String::new()
            };
            rels_html.push_str(&format!(
                r#"<tr>
                    <td>{}</td>
                    <td><a href="/view_bug_page/{}">{:07}</a></td>
                    <td>{}</td>
                </tr>"#,
                html_escape(relationship_label(r.relationship_type)),
                r.destination_bug_id,
                r.destination_bug_id,
                del_btn
            ));
        }
        rels_html.push_str("</tbody></table>");
    }

    let rel_form_html = if eff >= 55 {
        format!(
            r#"<form method="POST" action="/bug_relationship_add/{}" style="margin-top:10px; display:flex; gap:8px; align-items:center;">
                <select name="relationship_type" style="width:auto;">
                    <option value="0">related to</option>
                    <option value="1">duplicate of</option>
                    <option value="2">has duplicate</option>
                    <option value="3">blocks</option>
                    <option value="4">blocked by</option>
                    <option value="5">parent of</option>
                    <option value="6">child of</option>
                </select>
                <input type="text" name="dest_bug_id" placeholder="Target Bug ID" style="width:140px;" required>
                <button type="submit" class="btn btn-secondary btn-sm">Add Relationship</button>
            </form>"#,
            bug.id
        )
    } else {
        String::new()
    };

    // Bugnotes section
    let mut notes_html = String::new();
    if notes.is_empty() {
        notes_html.push_str("<p>No notes attached to this issue.</p>");
    } else {
        notes_html.push_str(r#"<table class="data-table"><thead><tr><th>Author</th><th>Note</th><th>Visibility</th><th>Actions</th></tr></thead><tbody>"#);
        for n in &notes {
            let author = users_map
                .get(&n.reporter_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", n.reporter_id));
            let vis = if n.view_state == 50 {
                "private"
            } else {
                "public"
            };
            let can_del_note = eff >= 70 || (eff >= 25 && n.reporter_id == ctx.user.id);
            let del_btn = if can_del_note {
                format!(
                    r#"<form method="POST" action="/bugnote_delete/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    n.id
                )
            } else {
                String::new()
            };
            notes_html.push_str(&format!(
                r#"<tr>
                    <td style="width:160px;"><strong>{}</strong></td>
                    <td style="white-space:pre-wrap;">{}</td>
                    <td style="width:90px;">{}</td>
                    <td style="width:90px;">{}</td>
                </tr>"#,
                html_escape(&author),
                html_escape(&n.note),
                vis,
                del_btn
            ));
        }
        notes_html.push_str("</tbody></table>");
    }

    let note_form_html = if eff >= 25 {
        let priv_opt = if eff >= 55 {
            r#"<option value="50">private</option>"#
        } else {
            ""
        };
        format!(
            r#"<form method="POST" action="/bugnote_add/{}" style="margin-top:12px;">
                <table class="form-table">
                    <tr>
                        <td class="label-col">Note</td>
                        <td class="input-col"><textarea name="note" required></textarea></td>
                    </tr>
                    <tr>
                        <td class="label-col">View Status</td>
                        <td class="input-col">
                            <select name="view_state" style="width:auto;">
                                <option value="10">public</option>
                                {}
                            </select>
                        </td>
                    </tr>
                </table>
                <button type="submit" class="btn btn-sm">Add Note</button>
            </form>"#,
            bug.id, priv_opt
        )
    } else {
        String::new()
    };

    // History section
    let mut history_html = String::new();
    if history.is_empty() {
        history_html.push_str("<p>No history entries.</p>");
    } else {
        history_html.push_str(r#"<table class="data-table"><thead><tr><th>User</th><th>Field</th><th>Change</th></tr></thead><tbody>"#);
        for h in &history {
            let uname = users_map
                .get(&h.user_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", h.user_id));
            let change_desc = if h.type_ == 1 && h.field_name.is_empty() {
                "New Issue".to_string()
            } else {
                format!(
                    "{} => {}",
                    html_escape(&h.old_value),
                    html_escape(&h.new_value)
                )
            };
            history_html.push_str(&format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                html_escape(&uname),
                html_escape(&h.field_name),
                change_desc
            ));
        }
        history_html.push_str("</tbody></table>");
    }

    let view_state_str = ViewState::from_i32(bug.view_state)
        .map(|v| v.as_str())
        .unwrap_or("public");

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">View Issue Details [#{:07}]</div>
            <div class="panel-body">
                <div style="display:flex; gap:10px; flex-wrap:wrap; align-items:center; margin-bottom:14px; padding-bottom:10px; border-bottom:1px solid #e0e6ed;">
                    {}
                </div>
                <table class="form-table">
                    <tr>
                        <td class="label-col">ID</td>
                        <td class="input-col">{:07}</td>
                        <td class="label-col">Project</td>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Category</td>
                        <td class="input-col">{}</td>
                        <td class="label-col">View Status</td>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Reporter</td>
                        <td class="input-col">{}</td>
                        <td class="label-col">Assigned To</td>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Priority</td>
                        <td class="input-col">{}</td>
                        <td class="label-col">Severity</td>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Reproducibility</td>
                        <td class="input-col">{}</td>
                        <td class="label-col">Status / Resolution</td>
                        <td class="input-col">{} / {}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Product Version</td>
                        <td class="input-col">{}</td>
                        <td class="label-col">Fixed in Version / Target</td>
                        <td class="input-col">{} / {}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Summary</td>
                        <td class="input-col" colspan="3"><strong>{:07}: {}</strong></td>
                    </tr>
                    <tr>
                        <td class="label-col">Description</td>
                        <td class="input-col" colspan="3" style="white-space:pre-wrap;">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Steps To Reproduce</td>
                        <td class="input-col" colspan="3" style="white-space:pre-wrap;">{}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Additional Information</td>
                        <td class="input-col" colspan="3" style="white-space:pre-wrap;">{}</td>
                    </tr>
                </table>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Relationships</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Attached Files</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Activities / Bugnotes</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Issue History</div>
            <div class="panel-body">
                {}
            </div>
        </div>"#,
        err_banner,
        bug.id,
        actions_html,
        bug.id,
        html_escape(&proj_name),
        html_escape(&cat_name),
        html_escape(view_state_str),
        html_escape(&reporter_name),
        html_escape(&handler_name),
        html_escape(priority_label(bug.priority)),
        html_escape(severity_label(bug.severity)),
        html_escape(reproducibility_label(bug.reproducibility)),
        html_escape(status_label(bug.status)),
        html_escape(resolution_label(bug.resolution)),
        html_escape(&bug.version),
        html_escape(&bug.fixed_in_version),
        html_escape(&bug.target_version),
        bug.id,
        html_escape(&bug.summary),
        html_escape(&full.description),
        html_escape(&full.steps_to_reproduce),
        html_escape(&full.additional_information),
        rels_html,
        rel_form_html,
        files_html,
        upload_form_html,
        notes_html,
        note_form_html,
        history_html
    );

    render_layout(
        &format!("Issue #{:07}", bug.id),
        &ctx,
        q.msg.as_deref(),
        &content,
    )
    .into_response()
}

pub async fn update_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let bug = &full.bug;

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if bug.view_state == 50 && eff < 55 {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == ctx.user.id);
    if !can_edit {
        return forbidden(&ctx, "Insufficient access to edit this issue.");
    }

    let cats = db
        .list_effective_categories(bug.project_id)
        .unwrap_or_default();
    let mut cat_opts = String::new();
    for c in &cats {
        let sel = if c.id == bug.category_id {
            "selected"
        } else {
            ""
        };
        cat_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            c.id,
            sel,
            html_escape(&c.name)
        ));
    }

    let content = format!(
        r#"<div class="panel">
            <div class="panel-header">Update Issue #{:07}</div>
            <div class="panel-body">
                <form method="POST" action="/update_bug/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Category</td>
                            <td class="input-col"><select name="category_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority</td>
                            <td class="input-col">
                                <select name="priority">
                                    <option value="10" {}>none</option>
                                    <option value="20" {}>low</option>
                                    <option value="30" {}>normal</option>
                                    <option value="40" {}>high</option>
                                    <option value="50" {}>urgent</option>
                                    <option value="60" {}>immediate</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Severity</td>
                            <td class="input-col">
                                <select name="severity">
                                    <option value="10" {}>feature</option>
                                    <option value="20" {}>trivial</option>
                                    <option value="30" {}>text</option>
                                    <option value="40" {}>tweak</option>
                                    <option value="50" {}>minor</option>
                                    <option value="60" {}>major</option>
                                    <option value="70" {}>crash</option>
                                    <option value="80" {}>block</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Reproducibility</td>
                            <td class="input-col">
                                <select name="reproducibility">
                                    <option value="10" {}>always</option>
                                    <option value="30" {}>sometimes</option>
                                    <option value="50" {}>random</option>
                                    <option value="70" {}>have not tried</option>
                                    <option value="90" {}>unable to reproduce</option>
                                    <option value="100" {}>N/A</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Product Version</td>
                            <td class="input-col"><input type="text" name="version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Fixed in Version</td>
                            <td class="input-col"><input type="text" name="fixed_in_version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Target Version</td>
                            <td class="input-col"><input type="text" name="target_version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" {}>public</option>
                                    <option value="50" {}>private</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary *</td>
                            <td class="input-col"><input type="text" name="summary" value="{}" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description">{}</textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Steps To Reproduce</td>
                            <td class="input-col"><textarea name="steps_to_reproduce">{}</textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Additional Information</td>
                            <td class="input-col"><textarea name="additional_information">{}</textarea></td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Update Information</button>
                </form>
            </div>
        </div>"#,
        bug.id,
        bug.id,
        cat_opts,
        if bug.priority == 10 { "selected" } else { "" },
        if bug.priority == 20 { "selected" } else { "" },
        if bug.priority == 30 { "selected" } else { "" },
        if bug.priority == 40 { "selected" } else { "" },
        if bug.priority == 50 { "selected" } else { "" },
        if bug.priority == 60 { "selected" } else { "" },
        if bug.severity == 10 { "selected" } else { "" },
        if bug.severity == 20 { "selected" } else { "" },
        if bug.severity == 30 { "selected" } else { "" },
        if bug.severity == 40 { "selected" } else { "" },
        if bug.severity == 50 { "selected" } else { "" },
        if bug.severity == 60 { "selected" } else { "" },
        if bug.severity == 70 { "selected" } else { "" },
        if bug.severity == 80 { "selected" } else { "" },
        if bug.reproducibility == 10 { "selected" } else { "" },
        if bug.reproducibility == 30 { "selected" } else { "" },
        if bug.reproducibility == 50 { "selected" } else { "" },
        if bug.reproducibility == 70 { "selected" } else { "" },
        if bug.reproducibility == 90 { "selected" } else { "" },
        if bug.reproducibility == 100 { "selected" } else { "" },
        html_escape(&bug.version),
        html_escape(&bug.fixed_in_version),
        html_escape(&bug.target_version),
        if bug.view_state == 10 { "selected" } else { "" },
        if bug.view_state == 50 { "selected" } else { "" },
        html_escape(&bug.summary),
        html_escape(&full.description),
        html_escape(&full.steps_to_reproduce),
        html_escape(&full.additional_information),
    );

    render_layout(&format!("Update Issue #{:07}", bug.id), &ctx, None, &content).into_response()
}

pub async fn update_bug(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(fields): axum::Form<BugUpdate>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if bug.view_state == 50 && eff < 55 {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == ctx.user.id);
    if !can_edit {
        return forbidden(&ctx, "Insufficient access to update this issue.");
    }

    if let Some(hid) = fields.handler_id {
        if hid != bug.handler_id && eff < 55 {
            return forbidden(&ctx, "Developer access required to assign issues.");
        }
    }

    match db.update_bug_fields(id, ctx.user.id, fields) {
        Ok(_) => Redirect::to(&format!("/view_bug_page/{}?msg=Issue+updated", id)).into_response(),
        Err(_) => Redirect::to(&format!("/view_bug_page/{}?err=Failed+to+update+issue", id))
            .into_response(),
    }
}

pub async fn bug_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 70 {
        return forbidden(&ctx, "Manager access required to delete issues.");
    }

    let _ = db.delete_bug(id);
    Redirect::to("/view_all_bug_page?msg=Issue+deleted").into_response()
}

#[derive(Deserialize)]
pub struct ChangeStatusForm {
    pub new_status: i32,
    pub resolution: Option<i32>,
    pub note: Option<String>,
}

pub async fn bug_change_status(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<ChangeStatusForm>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if bug.view_state == 50 && eff < 55 {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }

    let required_level = if form.new_status == 90 {
        70 // Close requires Manager
    } else if form.new_status == 80 || bug.status >= 80 {
        55 // Resolve or reopen requires Developer
    } else {
        40 // Other status transitions require Updater
    };

    if eff < required_level {
        return forbidden(
            &ctx,
            "Insufficient access level for this status transition.",
        );
    }

    let resolution = form.resolution.unwrap_or(bug.resolution);
    match db.change_bug_status(
        id,
        ctx.user.id,
        form.new_status,
        resolution,
        form.note.as_deref(),
    ) {
        Ok(()) => Redirect::to(&format!("/view_bug_page/{}?msg=Status+updated", id)).into_response(),
        Err(_) => Redirect::to(&format!(
            "/view_bug_page/{}?err=Invalid+status+transition",
            id
        ))
        .into_response(),
    }
}

#[derive(Deserialize)]
pub struct AssignBugForm {
    pub handler_id: i64,
}

pub async fn bug_assign(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<AssignBugForm>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 {
        return forbidden(&ctx, "Developer access required to assign issues.");
    }

    match db.assign_bug(id, ctx.user.id, form.handler_id) {
        Ok(()) => Redirect::to(&format!("/view_bug_page/{}?msg=Issue+assigned", id)).into_response(),
        Err(_) => Redirect::to(&format!("/view_bug_page/{}?err=Failed+to+assign+issue", id))
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct BugnoteAddForm {
    pub note: String,
    pub view_state: Option<i32>,
}

pub async fn bugnote_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    axum::Form(form): axum::Form<BugnoteAddForm>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 25 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "Reporter access required to add bugnotes.");
    }

    let vs = form.view_state.unwrap_or(10);
    if vs == 50 && eff < 55 {
        return forbidden(&ctx, "Developer access required to add private bugnotes.");
    }

    match db.add_bugnote(bug_id, ctx.user.id, &form.note, vs) {
        Ok(_) => Redirect::to(&format!("/view_bug_page/{}?msg=Note+added", bug_id)).into_response(),
        Err(_) => Redirect::to(&format!(
            "/view_bug_page/{}?err=Note+cannot+be+empty",
            bug_id
        ))
        .into_response(),
    }
}

pub async fn bugnote_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let note = match db.get_bugnote_by_id(id) {
        Ok(Some(n)) => n,
        _ => return not_found(&ctx, "Bugnote not found."),
    };
    let bug = match db.get_bug(note.bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    let can_delete = eff >= 70 || (eff >= 25 && note.reporter_id == ctx.user.id);
    if !can_delete {
        return forbidden(&ctx, "Manager access required to delete bugnotes.");
    }

    let _ = db.delete_bugnote(id);
    Redirect::to(&format!("/view_bug_page/{}?msg=Note+deleted", note.bug_id)).into_response()
}

#[derive(Deserialize)]
pub struct RelationshipAddForm {
    pub dest_bug_id: i64,
    pub relationship_type: i32,
}

pub async fn bug_relationship_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    axum::Form(form): axum::Form<RelationshipAddForm>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 {
        return forbidden(&ctx, "Developer access required to manage relationships.");
    }

    match db.add_bug_relationship(bug_id, form.dest_bug_id, form.relationship_type) {
        Ok(()) => Redirect::to(&format!(
            "/view_bug_page/{}?msg=Relationship+added",
            bug_id
        ))
        .into_response(),
        Err(_) => Redirect::to(&format!(
            "/view_bug_page/{}?err=Failed+to+add+relationship",
            bug_id
        ))
        .into_response(),
    }
}

pub async fn bug_relationship_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let rel = match db.get_bug_relationship_by_id(id) {
        Ok(Some(r)) => r,
        _ => return not_found(&ctx, "Relationship not found."),
    };
    let bug = match db.get_bug(rel.source_bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 {
        return forbidden(&ctx, "Developer access required to manage relationships.");
    }

    let _ = db.delete_bug_relationship(id);
    Redirect::to(&format!(
        "/view_bug_page/{}?msg=Relationship+deleted",
        rel.source_bug_id
    ))
    .into_response()
}

fn parse_multipart_file(headers: &HeaderMap, body: &[u8]) -> Option<(String, String, Vec<u8>)> {
    let ct = headers
        .get(header::CONTENT_TYPE)?
        .to_str()
        .ok()?;
    let boundary = ct
        .split(';')
        .find_map(|part| {
            let p = part.trim();
            p.strip_prefix("boundary=")
                .map(|b| b.trim_matches('"').to_string())
        })?;

    let delimiter = format!("--{}", boundary);
    let delim_bytes = delimiter.as_bytes();

    // Find first boundary
    let start_idx = body
        .windows(delim_bytes.len())
        .position(|w| w == delim_bytes)?;
    let after_boundary = &body[start_idx + delim_bytes.len()..];
    let after_boundary = after_boundary
        .strip_prefix(b"\r\n")
        .or_else(|| after_boundary.strip_prefix(b"\n"))?;

    // Find end of part headers (\r\n\r\n or \n\n)
    let (part_headers_bytes, rest) = if let Some(pos) = after_boundary
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
    {
        (&after_boundary[..pos], &after_boundary[pos + 4..])
    } else if let Some(pos) = after_boundary.windows(2).position(|w| w == b"\n\n") {
        (&after_boundary[..pos], &after_boundary[pos + 2..])
    } else {
        return None;
    };

    let part_headers = String::from_utf8_lossy(part_headers_bytes);
    let mut filename = "attachment.bin".to_string();
    let mut file_type = "application/octet-stream".to_string();

    for line in part_headers.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            for param in line.split(';') {
                let p = param.trim();
                if let Some(fname) = p.strip_prefix("filename=") {
                    filename = fname.trim_matches('"').to_string();
                }
            }
        } else if let Some(val) = line.split_once(':').map(|(_, v)| v.trim()) {
            if lower.starts_with("content-type:") {
                file_type = val.to_string();
            }
        }
    }

    // Find next boundary in rest
    let end_idx = rest
        .windows(delim_bytes.len())
        .position(|w| w == delim_bytes)?;
    let mut content = &rest[..end_idx];
    if let Some(stripped) = content.strip_suffix(b"\r\n") {
        content = stripped;
    } else if let Some(stripped) = content.strip_suffix(b"\n") {
        content = stripped;
    }

    Some((filename, file_type, content.to_vec()))
}

pub async fn bug_file_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 25 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "Reporter access required to upload attachments.");
    }

    let max_bytes = max_file_size();
    let dir = upload_dir();

    if let Some((filename, content_type, data)) = parse_multipart_file(&headers, &body) {
        if data.len() > max_bytes {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Html("File exceeds maximum allowed size".to_string()),
            )
                .into_response();
        }
        let _ = db.add_bug_file(bug_id, ctx.user.id, &filename, &content_type, data, &dir);
    } else if body.len() > max_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Html("File exceeds maximum allowed size".to_string()),
        )
            .into_response();
    }

    Redirect::to(&format!("/view_bug_page/{}?msg=Attachment+uploaded", bug_id)).into_response()
}

pub async fn bug_file_download(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((file_id, _filename)): Path<(i64, String)>,
) -> Response {
    let dir = upload_dir();
    let (meta, bytes) = match db.get_bug_file(file_id, &dir) {
        Ok(Some(v)) => v,
        _ => return not_found(&ctx, "Attachment not found."),
    };

    let bug = match db.get_bug(meta.bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 10 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "Insufficient access to download this attachment.");
    }

    let content_type = if meta.file_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        meta.file_type
    };
    let disp = format!(
        "attachment; filename=\"{}\"",
        meta.filename.replace('"', "_")
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disp),
        ],
        bytes,
    )
        .into_response()
}

pub async fn bug_file_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let meta = match db.get_bug_file_meta(id) {
        Ok(Some(m)) => m,
        _ => return not_found(&ctx, "Attachment not found."),
    };
    let bug = match db.get_bug(meta.bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 70 {
        return forbidden(&ctx, "Manager access required to delete attachments.");
    }

    let dir = upload_dir();
    let _ = db.delete_bug_file(id, &dir);
    Redirect::to(&format!(
        "/view_bug_page/{}?msg=Attachment+deleted",
        meta.bug_id
    ))
    .into_response()
}
