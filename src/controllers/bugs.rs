use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{BugFilter, BugUpdate};
use crate::views;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct ListBugsQuery {
    pub page: Option<i64>,
    pub filter_id: Option<i64>,
    pub q: Option<String>,
    pub search: Option<String>,
    pub status: Option<i32>,
    pub hide_status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub category_id: Option<i64>,
    pub reporter_id: Option<i64>,
    pub handler_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub monitored_by: Option<i64>,
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
        views::html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        views::layout::render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

fn not_found(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Not Found: {}</div>"#,
        views::html_escape(msg)
    );
    (
        StatusCode::NOT_FOUND,
        views::layout::render_layout("Not Found", ctx, None, &body),
    )
        .into_response()
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

fn resolve_bug_filter(db: &Db, ctx: &AuthContext, q: &ListBugsQuery) -> BugFilter {
    if let Some(fid) = q.filter_id {
        if let Ok(Some((sf, mut bf))) = db.get_filter(fid) {
            if sf.is_public != 0 || sf.user_id == ctx.user.id || ctx.user.access_level >= 70 {
                if ctx.active_project_id > 0 {
                    bf.project_id = Some(ctx.active_project_id);
                }
                return bf;
            }
        }
    }

    let kw =
        q.q.clone()
            .or_else(|| q.search.clone())
            .filter(|s| !s.trim().is_empty());

    BugFilter {
        project_id: if ctx.active_project_id > 0 {
            Some(ctx.active_project_id)
        } else {
            None
        },
        search: kw,
        status: q.status.filter(|&v| v > 0),
        hide_status: q.hide_status.filter(|&v| v > 0),
        priority: q.priority.filter(|&v| v > 0),
        severity: q.severity.filter(|&v| v > 0),
        category_id: q.category_id.filter(|&v| v > 0),
        reporter_id: q.reporter_id.filter(|&v| v > 0),
        handler_id: q.handler_id.filter(|&v| v != 0),
        tag_id: q.tag_id.filter(|&v| v > 0),
        monitored_by: q.monitored_by.filter(|&v| v > 0),
    }
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/view_all_bug_page", get(view_all_bug_page))
        .route("/view_bug_page/:id", get(view_bug_page))
        .route("/bug_report_page", get(bug_report_page))
        .route("/bug_report", post(bug_report))
        .route("/update_bug_page/:id", get(update_bug_page))
        .route("/update_bug/:id", post(update_bug))
        .route("/bug_delete/:id", post(bug_delete))
        .route("/bug_change_status/:id", post(bug_change_status))
        .route("/bug_assign/:id", post(bug_assign))
        .route("/bugnote_add/:bug_id", post(bugnote_add))
        .route("/bugnote_delete/:id", post(bugnote_delete))
        .route("/bug_relationship_add/:bug_id", post(bug_relationship_add))
        .route(
            "/bug_relationship_delete/:id",
            post(bug_relationship_delete),
        )
        .route(
            "/bug_file_add/:bug_id",
            post(bug_file_add).layer(DefaultBodyLimit::disable()),
        )
        .route("/file/bug/:id/:filename", get(bug_file_download))
        .route("/bug_file_delete/:id", post(bug_file_delete))
        .route("/my_view_page", get(my_view_page))
        .route("/csv_export", get(csv_export))
        .route("/bug_monitor_add/:bug_id", post(bug_monitor_add))
        .route("/bug_monitor_delete/:bug_id", post(bug_monitor_delete))
}

pub async fn my_view_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    let pid = if ctx.active_project_id > 0 {
        Some(ctx.active_project_id)
    } else {
        None
    };

    let assigned_to_me = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                handler_id: Some(ctx.user.id),
                hide_status: Some(80),
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    let unassigned = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                handler_id: Some(-1),
                hide_status: Some(80),
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    let reported_by_me = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                reporter_id: Some(ctx.user.id),
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    let resolved = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                status: Some(80),
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    let recently_modified = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    let monitored_by_me = db
        .filter_bugs(
            &BugFilter {
                project_id: pid,
                monitored_by: Some(ctx.user.id),
                ..Default::default()
            },
            &ctx.user,
        )
        .unwrap_or_default();

    views::bugs::render_my_view_page(
        &ctx,
        &assigned_to_me,
        &unassigned,
        &reported_by_me,
        &resolved,
        &recently_modified,
        &monitored_by_me,
    )
    .into_response()
}

fn sanitize_csv_cell(val: &str) -> String {
    let trimmed = val.trim_start();
    let needs_prefix = trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@');
    let safe = if needs_prefix {
        format!("'{}", val)
    } else {
        val.to_string()
    };
    if safe.contains(',') || safe.contains('"') || safe.contains('\n') || safe.contains('\r') {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

pub async fn csv_export(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<ListBugsQuery>,
) -> Response {
    if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return forbidden(&ctx, "You do not have access to this project.");
        }
    }

    let filter = resolve_bug_filter(&db, &ctx, &q);
    let bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();

    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let mut out = String::from(
        "Id,Project,Reporter,Assigned To,Priority,Severity,Reproducibility,Status,Resolution,Category,Summary,Updated\r\n",
    );

    for b in &bugs {
        let proj = db
            .get_project_by_id(b.project_id)
            .ok()
            .flatten()
            .map(|p| p.name)
            .unwrap_or_else(|| format!("#{}", b.project_id));
        let rep = users_map
            .get(&b.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", b.reporter_id));
        let handler = if b.handler_id > 0 {
            users_map
                .get(&b.handler_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", b.handler_id))
        } else {
            String::new()
        };
        let cat = db
            .get_category_by_id(b.category_id)
            .ok()
            .flatten()
            .map(|c| c.name)
            .unwrap_or_else(|| "General".to_string());

        let row = [
            format!("{:07}", b.id),
            sanitize_csv_cell(&proj),
            sanitize_csv_cell(&rep),
            sanitize_csv_cell(&handler),
            sanitize_csv_cell(views::bugs::priority_label(b.priority)),
            sanitize_csv_cell(views::bugs::severity_label(b.severity)),
            sanitize_csv_cell(views::bugs::reproducibility_label(b.reproducibility)),
            sanitize_csv_cell(views::bugs::status_label(b.status)),
            sanitize_csv_cell(views::bugs::resolution_label(b.resolution)),
            sanitize_csv_cell(&cat),
            sanitize_csv_cell(&b.summary),
            b.last_updated.to_string(),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"issues.csv\"".to_string(),
            ),
        ],
        out,
    )
        .into_response()
}

pub async fn view_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<ListBugsQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 25;
    let offset = (page - 1) * per_page;

    if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return forbidden(&ctx, "You do not have access to this project.");
        }
    }

    let filter = resolve_bug_filter(&db, &ctx, &q);
    let bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();

    let total = bugs.len();
    let paged_bugs: Vec<_> = bugs
        .into_iter()
        .skip(offset as usize)
        .take(per_page as usize)
        .collect();

    let all_users = db.list_users(None, false).unwrap_or_default();
    let users_map: HashMap<i64, String> = all_users
        .iter()
        .map(|u| (u.id, u.username.clone()))
        .collect();

    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();

    let mut categories_map: HashMap<i64, String> = HashMap::new();
    let cats = db
        .list_effective_categories(ctx.active_project_id)
        .unwrap_or_default();
    let mut all_cats = Vec::new();
    for c in &cats {
        categories_map.insert(c.id, c.name.clone());
        all_cats.push((c.id, c.name.clone()));
    }

    let all_tags = db.list_tags().unwrap_or_default();
    let saved_filters = db
        .list_visible_filters(ctx.user.id, ctx.active_project_id)
        .unwrap_or_default();

    let mut jira_links = HashMap::new();
    for b in &paged_bugs {
        if let Ok(Some(jl)) = db.get_jira_link_by_bug_id(b.id) {
            jira_links.insert(b.id, jl);
        }
    }

    views::bugs::render_view_all_bug_page(
        &ctx,
        &paged_bugs,
        total,
        page,
        per_page,
        &filter,
        &projects_map,
        &categories_map,
        &users_map,
        &jira_links,
        &all_users,
        &all_cats,
        &all_tags,
        &saved_filters,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn bug_report_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    let default_proj_id = if ctx.active_project_id > 0 {
        ctx.active_project_id
    } else {
        ctx.visible_projects.first().map(|(p, _)| p.id).unwrap_or(0)
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
    let custom_fields = if default_proj_id > 0 {
        db.list_custom_fields_for_project(default_proj_id)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut project_opts = Vec::new();
    for (p, _) in &ctx.visible_projects {
        let eff = db.effective_access_level(&ctx.user, p.id).unwrap_or(0);
        if eff >= 25 {
            project_opts.push((p.id, p.name.clone(), p.id == default_proj_id));
        }
    }

    let cat_pairs: Vec<_> = cats.into_iter().map(|c| (c.id, c.name)).collect();

    views::bugs::render_bug_report_page(
        &ctx,
        &project_opts,
        &cat_pairs,
        &versions,
        &custom_fields,
        q.err.as_deref(),
        q.msg.as_deref(),
    )
    .into_response()
}

pub async fn bug_report(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> Response {
    let project_id: i64 = match form.get("project_id").and_then(|v| v.parse().ok()) {
        Some(pid) => pid,
        None => return Redirect::to("/bug_report_page?err=Invalid+project").into_response(),
    };

    let eff = db
        .effective_access_level(&ctx.user, project_id)
        .unwrap_or(0);
    if eff < 25 {
        return forbidden(
            &ctx,
            "Reporter access required to report issues in this project.",
        );
    }

    let summary = form.get("summary").map(|s| s.trim()).unwrap_or("");
    if summary.is_empty() {
        return Redirect::to("/bug_report_page?err=Summary+is+required").into_response();
    }

    let category_id: i64 = form
        .get("category_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let reproducibility: i32 = form
        .get("reproducibility")
        .and_then(|v| v.parse().ok())
        .unwrap_or(70);
    let severity: i32 = form
        .get("severity")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let priority: i32 = form
        .get("priority")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let version = form.get("version").cloned().unwrap_or_default();
    let platform = form.get("platform").cloned().unwrap_or_default();
    let os = form.get("os").cloned().unwrap_or_default();
    let os_build = form.get("os_build").cloned().unwrap_or_default();
    let description = form.get("description").cloned().unwrap_or_default();
    let steps_to_reproduce = form.get("steps_to_reproduce").cloned().unwrap_or_default();
    let additional_info = form
        .get("additional_information")
        .cloned()
        .unwrap_or_default();
    let view_state: i32 = form
        .get("view_state")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let bug_id = match db.create_bug(
        project_id,
        ctx.user.id,
        category_id,
        summary,
        &description,
        &steps_to_reproduce,
        &additional_info,
        priority,
        severity,
        reproducibility,
        &version,
        &os,
        &os_build,
        &platform,
        view_state,
    ) {
        Ok(id) => id,
        Err(_) => {
            return Redirect::to("/bug_report_page?err=Failed+to+create+issue").into_response()
        }
    };

    let custom_fields = db
        .list_custom_fields_for_project(project_id)
        .unwrap_or_default();
    for cf in &custom_fields {
        let key = format!("custom_field_{}", cf.id);
        if let Some(val) = form.get(&key) {
            let _ = db.set_custom_field_value(cf.id, bug_id, ctx.user.id, val);
        }
    }

    Redirect::to(&format!("/view_bug_page/{}", bug_id)).into_response()
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
    let users_map: HashMap<i64, String> =
        users.iter().map(|u| (u.id, u.username.clone())).collect();

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
    let bug_tags = db.list_bug_tags(id).unwrap_or_default();
    let cf_values = db.get_custom_field_values_for_bug(id).unwrap_or_default();
    let monitors = db.list_bug_monitors(id).unwrap_or_default();
    let is_monitoring = monitors.iter().any(|u| u.id == ctx.user.id);

    // Quick Action Toolbar
    let mut actions_html = String::new();
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == ctx.user.id);
    if can_edit {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-sm" href="/update_bug_page/{}">Edit</a>"#,
            bug.id
        ));
    }

    if is_monitoring {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_monitor_delete/{}" style="display:inline;">
                <button type="submit" class="btn btn-secondary btn-sm">End Monitoring</button>
            </form>"#,
            bug.id
        ));
    } else {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_monitor_add/{}" style="display:inline;">
                <button type="submit" class="btn btn-secondary btn-sm">Monitor</button>
            </form>"#,
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
                views::html_escape(&u.username)
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

    let jira_link = db.get_jira_link_by_bug_id(id).unwrap_or(None);
    let jira_conf =
        db.get_jira_config(bug.project_id)
            .unwrap_or_else(|_| crate::models::JiraResolvedConfig {
                enabled: false,
                base_url: String::new(),
                email: String::new(),
                api_token: String::new(),
                default_issue_type: "Bug".to_string(),
                project_key: String::new(),
            });

    if jira_link.is_none() && jira_conf.enabled && eff >= 55 {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/view_bug_page/{}/jira_push" style="display:inline-flex; gap:4px; align-items:center;">
                <select name="issue_type" style="width:auto;">
                    <option value="Bug">Jira Bug</option>
                    <option value="Task">Jira Task</option>
                </select>
                <button type="submit" class="btn btn-primary btn-sm" style="background:#0052cc; color:#fff; border-color:#0747a6;">Push to Jira</button>
            </form>"#,
            bug.id
        ));
    }

    let mut tags_html = String::new();
    if bug_tags.is_empty() {
        tags_html.push_str("<span>No tags attached.</span>");
    } else {
        for bt in &bug_tags {
            let can_detach = eff >= 55 || (eff >= 25 && bt.user_id == ctx.user.id);
            let detach_form = if can_detach {
                format!(
                    r#" <form method="POST" action="/bug_tag_detach/{}/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">x</button></form>"#,
                    bug.id, bt.tag_id
                )
            } else {
                String::new()
            };
            tags_html.push_str(&format!(
                r#"<span class="badge badge-success" style="margin-right:6px;"><a href="/view_all_bug_page?tag_id={}">{}</a>{}</span>"#,
                bt.tag_id,
                views::html_escape(&bt.tag_name),
                detach_form
            ));
        }
    }
    if eff >= 25 {
        tags_html.push_str(&format!(
            r#"<form method="POST" action="/bug_tag_attach/{}" style="margin-top:8px; display:flex; gap:6px; align-items:center;">
                <input type="text" name="tag_string" placeholder="Attach tags (comma-separated)" style="width:240px;">
                <button type="submit" class="btn btn-secondary btn-sm">Attach Tags</button>
            </form>"#,
            bug.id
        ));
    }

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
                views::html_escape(&f.filename),
                views::html_escape(&f.filename),
                f.filesize,
                views::html_escape(&f.file_type),
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
                views::html_escape(views::bugs::relationship_label(r.relationship_type)),
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

    let total_time_minutes: i32 = notes.iter().map(|n| n.time_tracking.max(0)).sum();
    let total_time_html = if total_time_minutes > 0 {
        format!(
            r#"<p style="margin-bottom:8px;"><strong>Total Time Spent:</strong> {:02}:{:02} ({} mins)</p>"#,
            total_time_minutes / 60,
            total_time_minutes % 60,
            total_time_minutes
        )
    } else {
        String::new()
    };

    let mut notes_html = total_time_html;
    if notes.is_empty() {
        notes_html.push_str("<p>No notes attached to this issue.</p>");
    } else {
        notes_html.push_str(r#"<table class="data-table"><thead><tr><th>Author</th><th>Note</th><th>Time Spent</th><th>Visibility</th><th>Actions</th></tr></thead><tbody>"#);
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
            let time_str = if n.time_tracking > 0 {
                format!("{:02}:{:02}", n.time_tracking / 60, n.time_tracking % 60)
            } else {
                "-".to_string()
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
                    <td style="width:180px; background:#f2f5f8;">
                        <strong>{}</strong><br>
                        <span style="font-size:11px; color:#666;">{}</span>
                    </td>
                    <td style="white-space:pre-wrap;">{}</td>
                    <td style="width:90px;">{}</td>
                    <td style="width:90px;">{}</td>
                    <td style="width:90px;">{}</td>
                </tr>"#,
                views::html_escape(&author),
                views::format_timestamp(n.date_submitted),
                views::html_escape(&n.note),
                time_str,
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
        let time_row = if eff >= 55 {
            r#"<tr>
                <td class="label-col">Time Tracking (HH:MM or mins)</td>
                <td class="input-col"><input type="text" name="time_tracking" placeholder="00:00 or minutes" style="width:160px;"></td>
            </tr>"#
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
                    {}
                </table>
                <button type="submit" class="btn btn-sm">Add Note</button>
            </form>"#,
            bug.id, priv_opt, time_row
        )
    } else {
        String::new()
    };

    let mut history_html = String::new();
    if history.is_empty() {
        history_html.push_str("<p>No history entries.</p>");
    } else {
        history_html.push_str(r#"<table class="data-table"><thead><tr><th>Date Modified</th><th>Username</th><th>Field</th><th>Change</th></tr></thead><tbody>"#);
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
                    views::html_escape(&h.old_value),
                    views::html_escape(&h.new_value)
                )
            };
            history_html.push_str(&format!(
                r#"<tr>
                    <td style="width:150px; white-space:nowrap;">{}</td>
                    <td style="width:150px;">{}</td>
                    <td style="width:180px;">{}</td>
                    <td>{}</td>
                </tr>"#,
                views::format_timestamp(h.date_modified),
                views::html_escape(&uname),
                views::html_escape(&h.field_name),
                change_desc
            ));
        }
        history_html.push_str("</tbody></table>");
    }

    views::bugs::render_view_bug_page(
        &ctx,
        &full,
        &proj_name,
        &cat_name,
        &reporter_name,
        &handler_name,
        &cf_values,
        &actions_html,
        &tags_html,
        &rels_html,
        &rel_form_html,
        &files_html,
        &upload_form_html,
        &notes_html,
        &note_form_html,
        &history_html,
        jira_link.as_ref(),
        q.msg.as_deref(),
        q.err.as_deref(),
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
            views::html_escape(&c.name)
        ));
    }

    let cf_values = db.get_custom_field_values_for_bug(id).unwrap_or_default();
    let mut cf_rows_html = String::new();
    for cfv in &cf_values {
        cf_rows_html.push_str(&format!(
            r#"<tr>
                <td class="label-col">{}</td>
                <td class="input-col"><input type="text" name="custom_field_{}" value="{}"></td>
            </tr>"#,
            views::html_escape(&cfv.field_name),
            cfv.field_id,
            views::html_escape(&cfv.value)
        ));
    }

    views::bugs::render_update_bug_page(&ctx, &full, &cat_opts, &cf_rows_html).into_response()
}

pub async fn update_bug(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
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

    let handler_id = form.get("handler_id").and_then(|v| v.parse::<i64>().ok());
    if let Some(hid) = handler_id {
        if hid != bug.handler_id && eff < 55 {
            return forbidden(&ctx, "Developer access required to assign issues.");
        }
    }

    let fields = BugUpdate {
        summary: form.get("summary").cloned(),
        category_id: form.get("category_id").and_then(|v| v.parse::<i64>().ok()),
        priority: form.get("priority").and_then(|v| v.parse::<i32>().ok()),
        severity: form.get("severity").and_then(|v| v.parse::<i32>().ok()),
        reproducibility: form
            .get("reproducibility")
            .and_then(|v| v.parse::<i32>().ok()),
        version: form.get("version").cloned(),
        target_version: form.get("target_version").cloned(),
        fixed_in_version: form.get("fixed_in_version").cloned(),
        view_state: form.get("view_state").and_then(|v| v.parse::<i32>().ok()),
        handler_id,
        description: form.get("description").cloned(),
        steps_to_reproduce: form.get("steps_to_reproduce").cloned(),
        additional_information: form.get("additional_information").cloned(),
    };

    match db.update_bug_fields(id, ctx.user.id, fields) {
        Ok(_) => {
            let custom_fields = db
                .list_custom_fields_for_project(bug.project_id)
                .unwrap_or_default();
            for cf in &custom_fields {
                let key = format!("custom_field_{}", cf.id);
                if let Some(val) = form.get(&key) {
                    if db
                        .set_custom_field_value(cf.id, id, ctx.user.id, val)
                        .is_err()
                    {
                        return Redirect::to(&format!(
                            "/view_bug_page/{}?err=Invalid+custom+field",
                            id
                        ))
                        .into_response();
                    }
                }
            }
            Redirect::to(&format!("/view_bug_page/{}?msg=Issue+updated", id)).into_response()
        }
        Err(_) => Redirect::to(&format!("/view_bug_page/{}?err=Failed+to+update+issue", id))
            .into_response(),
    }
}

pub async fn bug_monitor_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 10 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "Insufficient access to monitor this issue.");
    }
    let _ = db.add_bug_monitor(bug_id, ctx.user.id);
    Redirect::to(&format!("/view_bug_page/{}?msg=Issue+monitored", bug_id)).into_response()
}

pub async fn bug_monitor_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
) -> Response {
    let _ = db.remove_bug_monitor(bug_id, ctx.user.id);
    Redirect::to(&format!(
        "/view_bug_page/{}?msg=Stopped+monitoring+issue",
        bug_id
    ))
    .into_response()
}

pub async fn bug_delete(State(db): State<Db>, ctx: AuthContext, Path(id): Path<i64>) -> Response {
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
        Ok(()) => {
            Redirect::to(&format!("/view_bug_page/{}?msg=Status+updated", id)).into_response()
        }
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
        Ok(()) => {
            Redirect::to(&format!("/view_bug_page/{}?msg=Issue+assigned", id)).into_response()
        }
        Err(_) => Redirect::to(&format!("/view_bug_page/{}?err=Failed+to+assign+issue", id))
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct BugnoteAddForm {
    pub note: String,
    pub view_state: Option<i32>,
    pub time_tracking: Option<String>,
}

fn parse_time_tracking_minutes(raw: Option<&str>) -> i32 {
    let s = match raw {
        Some(v) => v.trim(),
        None => return 0,
    };
    if s.is_empty() {
        return 0;
    }
    if let Some((hh, mm)) = s.split_once(':') {
        let h = hh.trim().parse::<i32>().unwrap_or(0).max(0);
        let m = mm.trim().parse::<i32>().unwrap_or(0).max(0);
        h * 60 + m
    } else {
        s.parse::<i32>().unwrap_or(0).max(0)
    }
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

    let time_mins = if eff >= 55 {
        parse_time_tracking_minutes(form.time_tracking.as_deref())
    } else {
        0
    };

    match db.add_bugnote_with_time(bug_id, ctx.user.id, &form.note, vs, time_mins) {
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
        Ok(()) => Redirect::to(&format!("/view_bug_page/{}?msg=Relationship+added", bug_id))
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
    let ct = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    let boundary = ct.split(';').find_map(|part| {
        let p = part.trim();
        p.strip_prefix("boundary=")
            .map(|b| b.trim_matches('"').to_string())
    })?;

    let delimiter = format!("--{}", boundary);
    let delim_bytes = delimiter.as_bytes();

    let start_idx = body
        .windows(delim_bytes.len())
        .position(|w| w == delim_bytes)?;
    let after_boundary = &body[start_idx + delim_bytes.len()..];
    let after_boundary = after_boundary
        .strip_prefix(b"\r\n")
        .or_else(|| after_boundary.strip_prefix(b"\n"))?;

    let (part_headers_bytes, rest) =
        if let Some(pos) = after_boundary.windows(4).position(|w| w == b"\r\n\r\n") {
            (&after_boundary[..pos], &after_boundary[pos + 4..])
        } else {
            let pos = after_boundary.windows(2).position(|w| w == b"\n\n")?;
            (&after_boundary[..pos], &after_boundary[pos + 2..])
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

    Redirect::to(&format!(
        "/view_bug_page/{}?msg=Attachment+uploaded",
        bug_id
    ))
    .into_response()
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
