use super::{sanitize_error_message, url_encode};
use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{
    clamp_per_page, sort_bugs, BugFilter, BugUpdate, CustomFieldType, ListBugsQuery,
};
use crate::views;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

fn get_single_val<'a>(form_map: &'a HashMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
    form_map
        .get(key)
        .and_then(|v| v.first())
        .map(|s| s.as_str())
}

fn get_custom_field_submitted_value(
    form_map: &HashMap<String, Vec<String>>,
    field_id: i64,
    field_type: i32,
) -> Option<String> {
    let key = format!("custom_field_{}", field_id);
    let present_key = format!("custom_field_{}_present", field_id);
    if let Some(values) = form_map.get(&key) {
        match CustomFieldType::from_i32(field_type) {
            Some(CustomFieldType::Checkbox) | Some(CustomFieldType::Multilist) => {
                let non_empty: Vec<&str> = values
                    .iter()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                Some(non_empty.join("|"))
            }
            _ => values.first().map(|s| s.trim().to_string()),
        }
    } else if form_map.contains_key(&present_key) {
        Some(String::new())
    } else {
        None
    }
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

fn conflict(ctx: &AuthContext, bug_id: i64, msg: &str) -> Response {
    (
        StatusCode::CONFLICT,
        views::bugs::render_conflict_page(ctx, bug_id, msg),
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
                if let Some(ref s) = q.sort {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        bf.sort = Some(trimmed.to_string());
                    }
                }
                if let Some(ref d) = q.dir {
                    let trimmed = d.trim();
                    if !trimmed.is_empty() {
                        bf.dir = Some(trimmed.to_string());
                    }
                }
                if let Some(pp) = q.per_page {
                    if pp > 0 {
                        bf.per_page = Some(clamp_per_page(Some(pp)));
                    }
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
        sort: q
            .sort
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        dir: q
            .dir
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        per_page: q
            .per_page
            .filter(|&v| v > 0)
            .map(|pp| clamp_per_page(Some(pp))),
    }
}

pub fn router() -> Router<Db> {
    let upload_limit = max_file_size()
        .saturating_add(2 * 1024 * 1024)
        .max(10 * 1024 * 1024);
    Router::new()
        .route("/view_all_bug_page", get(view_all_bug_page))
        .route("/view_bug_page/:id", get(view_bug_page))
        .route("/bug_report_page", get(bug_report_page))
        .route("/bug_report", post(bug_report))
        .route("/update_bug_page/:id", get(update_bug_page))
        .route("/update_bug/:id", post(update_bug))
        .route("/bug_clone_page/:id", get(bug_clone_page))
        .route("/bug_clone/:id", post(bug_clone))
        .route("/bug_move_page/:id", get(bug_move_page))
        .route("/bug_move/:id", post(bug_move))
        .route("/bug_actiongroup", post(bug_actiongroup))
        .route("/bug_stick/:id", post(bug_stick))
        .route("/bug_delete/:id", post(bug_delete))
        .route("/bug_change_status/:id", post(bug_change_status))
        .route("/bug_assign/:id", post(bug_assign))
        .route("/bugnote_add/:bug_id", post(bugnote_add))
        .route("/bugnote_delete/:id", post(bugnote_delete))
        .route("/bugnote_edit_page/:id", get(bugnote_edit_page))
        .route("/bugnote_update/:id", post(bugnote_update))
        .route("/bugnote_set_view_state/:id", post(bugnote_set_view_state))
        .route(
            "/bug_revision_view_page/:bug_id",
            get(bug_revision_view_page),
        )
        .route("/bug_revision_drop/:id", post(bug_revision_drop))
        .route("/bug_relationship_add/:bug_id", post(bug_relationship_add))
        .route(
            "/bug_relationship_delete/:id",
            post(bug_relationship_delete),
        )
        .route(
            "/bug_file_add/:bug_id",
            post(bug_file_add).layer(DefaultBodyLimit::max(upload_limit)),
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
    let mut bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();
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
    if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return forbidden(&ctx, "You do not have access to this project.");
        }
    }

    let filter = resolve_bug_filter(&db, &ctx, &q);
    let page = q.page.unwrap_or(1).max(1);

    let effective_sort = q
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(filter.sort.as_deref())
        .map(|s| s.to_string());

    let effective_dir = q
        .dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(filter.dir.as_deref())
        .map(|s| s.to_string());

    let effective_per_page = q
        .per_page
        .filter(|&v| v > 0)
        .or(filter.per_page)
        .map(|v| clamp_per_page(Some(v)));

    let per_page = clamp_per_page(effective_per_page) as i64;
    let offset = (page - 1) * per_page;

    let mut bugs = db.filter_bugs(&filter, &ctx.user).unwrap_or_default();
    sort_bugs(
        &mut bugs,
        effective_sort.as_deref(),
        effective_dir.as_deref(),
    );

    let mut effective_q = q.clone();
    effective_q.sort = effective_sort;
    effective_q.dir = effective_dir;
    effective_q.per_page = Some(per_page as usize);

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
        &effective_q,
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

    let default_eff = if default_proj_id > 0 {
        db.effective_access_level(&ctx.user, default_proj_id)
            .unwrap_or(0)
    } else {
        ctx.user.access_level
    };

    if default_eff < 25 {
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
        default_eff,
        q.err.as_deref(),
        q.msg.as_deref(),
    )
    .into_response()
}

pub async fn bug_report(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form_data): axum::Form<Vec<(String, String)>>,
) -> Response {
    let mut form_map: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_data {
        form_map.entry(k).or_default().push(v);
    }

    let project_id: i64 = match get_single_val(&form_map, "project_id").and_then(|v| v.parse().ok())
    {
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

    let summary = get_single_val(&form_map, "summary")
        .map(|s| s.trim())
        .unwrap_or("");
    if summary.is_empty() {
        return Redirect::to("/bug_report_page?err=Summary+is+required").into_response();
    }

    let category_id: i64 = get_single_val(&form_map, "category_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let reproducibility: i32 = get_single_val(&form_map, "reproducibility")
        .and_then(|v| v.parse().ok())
        .unwrap_or(70);
    let severity: i32 = get_single_val(&form_map, "severity")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let priority: i32 = get_single_val(&form_map, "priority")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let version = get_single_val(&form_map, "version")
        .unwrap_or_default()
        .to_string();
    let platform = get_single_val(&form_map, "platform")
        .unwrap_or_default()
        .to_string();
    let os = get_single_val(&form_map, "os")
        .unwrap_or_default()
        .to_string();
    let os_build = get_single_val(&form_map, "os_build")
        .unwrap_or_default()
        .to_string();
    let build = get_single_val(&form_map, "build")
        .map(|s| s.trim())
        .unwrap_or("");
    let due_date_str = get_single_val(&form_map, "due_date")
        .map(|s| s.trim())
        .unwrap_or("");
    let due_date = views::parse_date_to_epoch(due_date_str);
    let description = get_single_val(&form_map, "description")
        .unwrap_or_default()
        .to_string();
    let steps_to_reproduce = get_single_val(&form_map, "steps_to_reproduce")
        .unwrap_or_default()
        .to_string();
    let additional_info = get_single_val(&form_map, "additional_information")
        .unwrap_or_default()
        .to_string();
    let view_state: i32 = get_single_val(&form_map, "view_state")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let custom_fields = db
        .list_custom_fields_for_project(project_id)
        .unwrap_or_default();
    let versions = db.list_versions(project_id).unwrap_or_default();
    let cats = db.list_effective_categories(project_id).unwrap_or_default();

    // Validate custom fields before creating the bug
    let mut custom_field_values_to_set = Vec::new();
    for cf in &custom_fields {
        if eff < cf.access_level_r || cf.display_report == 0 {
            continue;
        }
        if eff >= cf.access_level_rw {
            let submitted = get_custom_field_submitted_value(&form_map, cf.id, cf.field_type);
            let val = submitted.unwrap_or_else(|| cf.default_value.clone());
            let resolved =
                crate::models::resolve_possible_values(&cf.possible_values, &versions, &cats);
            let is_req = cf.require_report != 0;
            if let Err(e) = crate::models::validate_custom_field_value(cf, &val, &resolved, is_req)
            {
                return Redirect::to(&format!(
                    "/bug_report_page?err={}",
                    url_encode(&sanitize_error_message(&e))
                ))
                .into_response();
            }
            custom_field_values_to_set.push((cf.id, val));
        }
    }

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

    if !build.is_empty() || due_date > 0 {
        let mut update = BugUpdate::default();
        if !build.is_empty() {
            update.build = Some(build.to_string());
        }
        if due_date > 0 {
            update.due_date = Some(due_date);
        }
        if let Err(e) = db.update_bug_fields(bug_id, ctx.user.id, update) {
            eprintln!(
                "[bug_report] Failed to update build/due_date for bug {}: {}",
                bug_id, e
            );
        }
    }

    for (cf_id, val) in custom_field_values_to_set {
        if let Err(e) = db.set_custom_field_value(cf_id, bug_id, ctx.user.id, &val) {
            return Redirect::to(&format!("/view_bug_page/{}?err={}", bug_id, url_encode(&e)))
                .into_response();
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
    if !Db::can_view_bug(ctx.user.id, eff, bug) {
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
    let mut history = db.list_bug_history(id).unwrap_or_default();
    let rels = db.list_bug_relationships(id).unwrap_or_default();
    let files = db.list_bug_files(id).unwrap_or_default();
    let bug_tags = db.list_bug_tags(id).unwrap_or_default();
    let mut cf_values = db.get_custom_field_values_for_bug(id).unwrap_or_default();
    cf_values.retain(|cfv| eff >= cfv.access_level_r);
    let custom_fields = db
        .list_custom_fields_for_project(bug.project_id)
        .unwrap_or_default();
    const CORE_HISTORY_FIELDS: &[&str] = &[
        "status",
        "resolution",
        "handler_id",
        "priority",
        "severity",
        "reproducibility",
        "summary",
        "description",
        "steps_to_reproduce",
        "additional_information",
        "category",
        "category_id",
        "project",
        "project_id",
        "version",
        "target_version",
        "fixed_in_version",
        "os",
        "os_build",
        "platform",
        "build",
        "due_date",
        "sticky",
        "view_state",
        "bugnote_added",
        "bugnote_deleted",
        "bugnote_updated",
        "relationship_added",
        "relationship_deleted",
        "file_added",
        "file_deleted",
        "monitor_added",
        "monitor_deleted",
        "tag_attached",
        "tag_detached",
    ];
    let hidden_cf_names: std::collections::HashSet<&str> = custom_fields
        .iter()
        .filter(|cf| {
            eff < cf.access_level_r
                && !CORE_HISTORY_FIELDS.contains(&cf.name.trim().to_lowercase().as_str())
        })
        .map(|cf| cf.name.as_str())
        .collect();
    history.retain(|h| !hidden_cf_names.contains(h.field_name.as_str()));
    let versions = db.list_versions(bug.project_id).unwrap_or_default();
    let cats = db
        .list_effective_categories(bug.project_id)
        .unwrap_or_default();
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
    actions_html.push_str(&format!(
        r#"<a class="btn btn-secondary btn-sm" href="/bug_revision_view_page/{}">Revisions</a>"#,
        bug.id
    ));

    let can_clone = ctx.visible_projects.iter().any(|(p, _)| {
        p.enabled == 1 && db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 25
    });
    if can_clone {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-secondary btn-sm" href="/bug_clone_page/{}">Clone</a>"#,
            bug.id
        ));
    }

    if eff >= 55 {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-secondary btn-sm" href="/bug_move_page/{}">Move</a>"#,
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
        let status_form = views::bugs::render_status_change_form(
            bug,
            eff,
            &custom_fields,
            &cf_values,
            &versions,
            &cats,
        );
        actions_html.push_str(&status_form);
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
        let (stick_label, stick_cls) = if bug.sticky == 1 {
            ("Unstick", "btn btn-warning btn-sm")
        } else {
            ("Stick", "btn btn-secondary btn-sm")
        };
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_stick/{}" style="display:inline;">
                <button type="submit" class="{}">{}</button>
            </form>"#,
            bug.id, stick_cls, stick_label
        ));

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

    let epics = if jira_conf.enabled && eff >= 55 && jira_link.is_none() {
        let pkey = jira_conf.project_key.trim();
        if pkey.is_empty() {
            vec![]
        } else if let Ok(Some(cached)) = db.get_cached_jira_epics(pkey, 86400) {
            cached
        } else {
            let client = crate::jira::JiraClient::with_allow_local(db.allow_local_jira());
            match client.list_epics(&jira_conf).await {
                Ok(fetched) => {
                    let _ = db.save_jira_epics_cache(pkey, &fetched);
                    fetched
                }
                Err(_) => {
                    // Fallback to expired cache if available during transient Jira issues
                    let fallback = db
                        .get_cached_jira_epics(pkey, i64::MAX)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    // Cache the fallback with a short 5-minute TTL
                    // so transient errors don't lock out epics for a full 24 hours.
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let _ = db.save_jira_epics_cache_with_ttl(pkey, &fallback, now, now + 300);
                    fallback
                }
            }
        }
    } else {
        vec![]
    };
    let jira_section_html = views::jira::render_jira_section(
        &ctx,
        bug.id,
        jira_link.as_ref(),
        &epics,
        jira_conf.enabled,
        eff,
    );

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
    notes_html.push_str(&views::bugs::render_bugnotes_table(
        &notes,
        &users_map,
        eff,
        ctx.user.id,
    ));

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

    let mut categories_map: HashMap<i64, String> = HashMap::new();
    if let Ok(cats) = db.list_all_categories() {
        for (cid, cname) in cats {
            categories_map.insert(cid, cname);
        }
    }
    let history_html = views::bugs::render_bug_history_html(&history, &users_map, &categories_map);

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
        &jira_section_html,
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
    if !Db::can_view_bug(ctx.user.id, eff, bug) {
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
    let custom_fields = db
        .list_custom_fields_for_project(bug.project_id)
        .unwrap_or_default();
    let versions = db.list_versions(bug.project_id).unwrap_or_default();

    views::bugs::render_update_bug_page(
        &ctx,
        &full,
        &cat_opts,
        &custom_fields,
        &cf_values,
        eff,
        &versions,
        &cats,
    )
    .into_response()
}

pub async fn update_bug(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form_data): axum::Form<Vec<(String, String)>>,
) -> Response {
    let mut form_map: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_data {
        form_map.entry(k).or_default().push(v);
    }

    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == ctx.user.id);
    if !can_edit {
        return forbidden(&ctx, "Insufficient access to update this issue.");
    }

    let expected_last_updated = match get_single_val(&form_map, "last_updated") {
        Some(raw) if !raw.trim().is_empty() => match raw.trim().parse::<i64>() {
            Ok(ts) => Some(ts),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    views::layout::render_layout(
                        "Bad Request",
                        &ctx,
                        Some("Invalid last_updated timestamp format for optimistic locking."),
                        "<p>Invalid last_updated timestamp format for optimistic locking.</p>",
                    ),
                )
                    .into_response();
            }
        },
        _ => None,
    };

    let handler_id = get_single_val(&form_map, "handler_id").and_then(|v| v.parse::<i64>().ok());
    if let Some(hid) = handler_id {
        if hid != bug.handler_id && eff < 55 {
            return forbidden(&ctx, "Developer access required to assign issues.");
        }
    }

    let due_date = get_single_val(&form_map, "due_date").map(views::parse_date_to_epoch);

    let fields = BugUpdate {
        summary: get_single_val(&form_map, "summary").map(|s| s.to_string()),
        category_id: get_single_val(&form_map, "category_id").and_then(|v| v.parse::<i64>().ok()),
        priority: get_single_val(&form_map, "priority").and_then(|v| v.parse::<i32>().ok()),
        severity: get_single_val(&form_map, "severity").and_then(|v| v.parse::<i32>().ok()),
        reproducibility: get_single_val(&form_map, "reproducibility")
            .and_then(|v| v.parse::<i32>().ok()),
        version: get_single_val(&form_map, "version").map(|s| s.to_string()),
        target_version: get_single_val(&form_map, "target_version").map(|s| s.to_string()),
        fixed_in_version: get_single_val(&form_map, "fixed_in_version").map(|s| s.to_string()),
        view_state: get_single_val(&form_map, "view_state").and_then(|v| v.parse::<i32>().ok()),
        handler_id,
        description: get_single_val(&form_map, "description").map(|s| s.to_string()),
        steps_to_reproduce: get_single_val(&form_map, "steps_to_reproduce").map(|s| s.to_string()),
        additional_information: get_single_val(&form_map, "additional_information")
            .map(|s| s.to_string()),
        os: get_single_val(&form_map, "os").map(|s| s.to_string()),
        os_build: get_single_val(&form_map, "os_build").map(|s| s.to_string()),
        platform: get_single_val(&form_map, "platform").map(|s| s.to_string()),
        build: get_single_val(&form_map, "build").map(|s| s.to_string()),
        due_date,
        sticky: None,
        expected_last_updated,
    };

    let custom_fields = db
        .list_custom_fields_for_project(bug.project_id)
        .unwrap_or_default();
    let versions = db.list_versions(bug.project_id).unwrap_or_default();
    let cats = db
        .list_effective_categories(bug.project_id)
        .unwrap_or_default();

    // Validate custom fields
    let mut custom_field_values_to_set = Vec::new();
    for cf in &custom_fields {
        if eff < cf.access_level_r || cf.display_update == 0 {
            continue;
        }
        // access_level_rw: ignore submitted mutations when effective access < cf.access_level_rw
        if eff < cf.access_level_rw {
            continue;
        }
        let is_present = form_map.contains_key(&format!("custom_field_{}", cf.id))
            || form_map.contains_key(&format!("custom_field_{}_present", cf.id));
        if is_present {
            let val = get_custom_field_submitted_value(&form_map, cf.id, cf.field_type)
                .unwrap_or_default();
            let resolved =
                crate::models::resolve_possible_values(&cf.possible_values, &versions, &cats);
            let is_req = cf.require_update != 0;
            if let Err(e) = crate::models::validate_custom_field_value(cf, &val, &resolved, is_req)
            {
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err={}",
                    id,
                    url_encode(&sanitize_error_message(&e))
                ))
                .into_response();
            }
            custom_field_values_to_set.push((cf.id, val));
        }
    }

    match db.update_bug_fields(id, ctx.user.id, fields) {
        Ok(_) => {
            for (cf_id, val) in custom_field_values_to_set {
                if let Err(e) = db.set_custom_field_value(cf_id, id, ctx.user.id, &val) {
                    return Redirect::to(&format!(
                        "/view_bug_page/{}?err={}",
                        id,
                        url_encode(&sanitize_error_message(&e))
                    ))
                    .into_response();
                }
            }
            Redirect::to(&format!("/view_bug_page/{}?msg=Issue+updated", id)).into_response()
        }
        Err(e) if e.starts_with("CONFLICT:") => conflict(
            &ctx,
            id,
            "Issue has been modified by another user or session.",
        ),
        Err(_) => Redirect::to(&format!("/view_bug_page/{}?err=Failed+to+update+issue", id))
            .into_response(),
    }
}

pub async fn bug_clone_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, full.bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &full.bug) {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }

    let mut target_projects = Vec::new();
    for (p, _) in &ctx.visible_projects {
        if p.enabled == 1 {
            let p_eff = db.effective_access_level(&ctx.user, p.id).unwrap_or(0);
            if p_eff >= 25 {
                let is_sel = p.id == full.bug.project_id;
                target_projects.push((p.id, p.name.clone(), is_sel));
            }
        }
    }

    if target_projects.is_empty() {
        return forbidden(
            &ctx,
            "Reporter access required on at least one project to clone an issue.",
        );
    }

    let proj_name = db
        .get_project_by_id(full.bug.project_id)
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_else(|| format!("Project #{}", full.bug.project_id));

    views::bugs::render_bug_clone_page(
        &ctx,
        &full,
        &proj_name,
        &target_projects,
        q.err.as_deref(),
        q.msg.as_deref(),
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct BugCloneForm {
    pub target_project_id: i64,
    pub copy_relationships: Option<String>,
    pub copy_files: Option<String>,
}

pub async fn bug_clone(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<BugCloneForm>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, full.bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &full.bug) {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }

    let target_eff = db
        .effective_access_level(&ctx.user, form.target_project_id)
        .unwrap_or(0);
    if target_eff < 25 {
        return forbidden(
            &ctx,
            "Reporter access required on target project to clone an issue.",
        );
    }

    let copy_rel = form.copy_relationships.as_deref() == Some("1")
        || form.copy_relationships.as_deref() == Some("on");
    let copy_files =
        form.copy_files.as_deref() == Some("1") || form.copy_files.as_deref() == Some("on");

    match db.clone_bug(
        id,
        ctx.user.id,
        form.target_project_id,
        copy_rel,
        copy_files,
        &upload_dir(),
    ) {
        Ok(new_id) => Redirect::to(&format!(
            "/view_bug_page/{}?msg={}",
            new_id,
            url_encode("Issue cloned successfully.")
        ))
        .into_response(),
        Err(e) => Redirect::to(&format!(
            "/bug_clone_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response(),
    }
}

pub async fn bug_move_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let source_eff = db
        .effective_access_level(&ctx.user, full.bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, source_eff, &full.bug) || source_eff < 55 {
        return forbidden(
            &ctx,
            "Developer access required on source project to move an issue.",
        );
    }

    let mut target_projects = Vec::new();
    for (p, _) in &ctx.visible_projects {
        if p.enabled == 1 && p.id != full.bug.project_id {
            let p_eff = db.effective_access_level(&ctx.user, p.id).unwrap_or(0);
            if p_eff >= 25 {
                target_projects.push((p.id, p.name.clone(), false));
            }
        }
    }

    if target_projects.is_empty() {
        return forbidden(
            &ctx,
            "No accessible target projects with Reporter access found to move the issue.",
        );
    }

    let proj_name = db
        .get_project_by_id(full.bug.project_id)
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_else(|| format!("Project #{}", full.bug.project_id));

    views::bugs::render_bug_move_page(
        &ctx,
        &full,
        &proj_name,
        &target_projects,
        q.err.as_deref(),
        q.msg.as_deref(),
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct BugMoveForm {
    pub target_project_id: i64,
}

pub async fn bug_move(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<BugMoveForm>,
) -> Response {
    let full = match db.get_bug_full(id) {
        Ok(Some(f)) => f,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let source_eff = db
        .effective_access_level(&ctx.user, full.bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, source_eff, &full.bug) || source_eff < 55 {
        return forbidden(
            &ctx,
            "Developer access required on source project to move an issue.",
        );
    }

    let target_eff = db
        .effective_access_level(&ctx.user, form.target_project_id)
        .unwrap_or(0);
    if target_eff < 25 {
        return forbidden(
            &ctx,
            "Reporter access required on target project to move an issue.",
        );
    }

    match db.move_bug(id, ctx.user.id, form.target_project_id) {
        Ok(_) => Redirect::to(&format!(
            "/view_bug_page/{}?msg={}",
            id,
            url_encode("Issue moved successfully.")
        ))
        .into_response(),
        Err(e) => Redirect::to(&format!(
            "/bug_move_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response(),
    }
}

pub async fn bug_actiongroup(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form_data): axum::Form<Vec<(String, String)>>,
) -> Response {
    let mut bug_ids = Vec::new();
    let mut form_map: HashMap<String, String> = HashMap::new();
    for (k, v) in form_data {
        if k == "bug_arr" {
            if let Ok(id) = v.trim().parse::<i64>() {
                if !bug_ids.contains(&id) {
                    bug_ids.push(id);
                }
            }
        } else {
            form_map.insert(k, v);
        }
    }

    if bug_ids.is_empty() {
        return Redirect::to("/view_all_bug_page?msg=No+issues+selected").into_response();
    }

    let action = form_map.get("action").map(|s| s.trim()).unwrap_or("");
    let mut succeeded = 0usize;
    let mut skipped = 0usize;

    for bug_id in bug_ids {
        let bug = match db.get_bug(bug_id) {
            Ok(Some(b)) => b,
            _ => {
                skipped += 1;
                continue;
            }
        };

        let eff = db
            .effective_access_level(&ctx.user, bug.project_id)
            .unwrap_or(0);
        if !Db::can_view_bug(ctx.user.id, eff, &bug) {
            skipped += 1;
            continue;
        }

        let res = match action {
            "CLOSE" => {
                if eff >= 70 {
                    db.change_bug_status(bug.id, ctx.user.id, 90, 20, None)
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "RESOLVE" => {
                if eff >= 55 {
                    let resolution = form_map
                        .get("resolution")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(20);
                    db.change_bug_status(bug.id, ctx.user.id, 80, resolution, None)
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "ASSIGN" => {
                if eff >= 55 {
                    let handler_id = form_map
                        .get("handler_id")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    db.assign_bug(bug.id, ctx.user.id, handler_id)
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "DELETE" => {
                if eff >= 70 {
                    db.delete_bug(bug.id).map_err(|e| e.to_string())
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "MOVE" => {
                if eff >= 55 {
                    let target_project_id = form_map
                        .get("target_project_id")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    let target_eff = db
                        .effective_access_level(&ctx.user, target_project_id)
                        .unwrap_or(0);
                    if target_eff >= 25 {
                        db.move_bug(bug.id, ctx.user.id, target_project_id)
                    } else {
                        Err("Permission denied on target project".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "COPY" => {
                let target_project_id = form_map
                    .get("target_project_id")
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(bug.project_id);
                let target_eff = db
                    .effective_access_level(&ctx.user, target_project_id)
                    .unwrap_or(0);
                if target_eff >= 25 {
                    db.clone_bug(
                        bug.id,
                        ctx.user.id,
                        target_project_id,
                        true,
                        true,
                        &upload_dir(),
                    )
                    .map(|_| ())
                } else {
                    Err("Permission denied on target project".to_string())
                }
            }
            "UP_PRIOR" => {
                if eff >= 40 {
                    let new_priority = form_map
                        .get("priority")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    if new_priority > 0 {
                        db.update_bug_fields(
                            bug.id,
                            ctx.user.id,
                            BugUpdate {
                                priority: Some(new_priority),
                                ..Default::default()
                            },
                        )
                        .map(|_| ())
                    } else {
                        Err("Invalid priority".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "UP_SEV" => {
                if eff >= 40 {
                    let new_severity = form_map
                        .get("severity")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    if new_severity > 0 {
                        db.update_bug_fields(
                            bug.id,
                            ctx.user.id,
                            BugUpdate {
                                severity: Some(new_severity),
                                ..Default::default()
                            },
                        )
                        .map(|_| ())
                    } else {
                        Err("Invalid severity".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "UP_STATUS" => {
                let target_status = form_map
                    .get("status")
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                let allowed = if target_status == 90 {
                    eff >= 70
                } else if target_status == 80 || (bug.status == 90 && target_status != 90) {
                    eff >= 55
                } else {
                    eff >= 40
                };
                if allowed && target_status > 0 {
                    let resolution = if target_status == 80 {
                        form_map
                            .get("resolution")
                            .and_then(|s| s.parse::<i32>().ok())
                            .unwrap_or(20)
                    } else {
                        bug.resolution
                    };
                    db.change_bug_status(bug.id, ctx.user.id, target_status, resolution, None)
                } else {
                    Err("Permission denied or invalid status".to_string())
                }
            }
            "UP_CATEGORY" => {
                if eff >= 40 {
                    let new_cat_id = form_map
                        .get("category_id")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    if new_cat_id > 0 {
                        let eff_cats = db
                            .list_effective_categories(bug.project_id)
                            .unwrap_or_default();
                        if eff_cats.iter().any(|c| c.id == new_cat_id) {
                            db.update_bug_fields(
                                bug.id,
                                ctx.user.id,
                                BugUpdate {
                                    category_id: Some(new_cat_id),
                                    ..Default::default()
                                },
                            )
                            .map(|_| ())
                        } else {
                            Err("Category not valid for project".to_string())
                        }
                    } else {
                        Err("Invalid category".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "UP_TARGET_VERSION" => {
                if eff >= 55 {
                    let target_ver = form_map
                        .get("target_version")
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    db.update_bug_fields(
                        bug.id,
                        ctx.user.id,
                        BugUpdate {
                            target_version: Some(target_ver),
                            ..Default::default()
                        },
                    )
                    .map(|_| ())
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "UP_DUE_DATE" => {
                if eff >= 55 {
                    let due_date_str = form_map.get("due_date").map(|s| s.trim()).unwrap_or("");
                    let due_date = views::parse_date_to_epoch(due_date_str);
                    db.update_bug_fields(
                        bug.id,
                        ctx.user.id,
                        BugUpdate {
                            due_date: Some(due_date),
                            ..Default::default()
                        },
                    )
                    .map(|_| ())
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "VIEW_STATUS" => {
                if eff >= 55 {
                    let view_state = form_map
                        .get("view_state")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(10);
                    db.update_bug_fields(
                        bug.id,
                        ctx.user.id,
                        BugUpdate {
                            view_state: Some(view_state),
                            ..Default::default()
                        },
                    )
                    .map(|_| ())
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "SET_STICKY" => {
                if eff >= 70 {
                    let sticky = form_map
                        .get("sticky")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    db.update_bug_fields(
                        bug.id,
                        ctx.user.id,
                        BugUpdate {
                            sticky: Some(sticky),
                            ..Default::default()
                        },
                    )
                    .map(|_| ())
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "ADD_NOTE" => {
                if eff >= 25 {
                    let note_text = form_map.get("bugnote_text").map(|s| s.trim()).unwrap_or("");
                    if !note_text.is_empty() {
                        let view_state = form_map
                            .get("note_view_state")
                            .and_then(|s| s.parse::<i32>().ok())
                            .unwrap_or(10);
                        db.add_bugnote(bug.id, ctx.user.id, note_text, view_state)
                            .map(|_| ())
                    } else {
                        Err("Empty note text".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "ATTACH_TAGS" => {
                if eff >= 25 {
                    let tag_string = form_map.get("tag_string").map(|s| s.trim()).unwrap_or("");
                    if !tag_string.is_empty() {
                        let mut any_attached = false;
                        for part in tag_string.split(',') {
                            let tname = part.trim();
                            if tname.is_empty() {
                                continue;
                            }
                            let tag_id = match db.get_tag_by_name(tname) {
                                Ok(Some(t)) => t.id,
                                _ => match db.create_tag(ctx.user.id, tname, "") {
                                    Ok(id) => id,
                                    Err(_) => continue,
                                },
                            };
                            if db.attach_tag_to_bug(bug.id, tag_id, ctx.user.id).is_ok() {
                                any_attached = true;
                            }
                        }
                        if any_attached {
                            Ok(())
                        } else {
                            Err("Failed to attach tags".to_string())
                        }
                    } else {
                        Err("Empty tag string".to_string())
                    }
                } else {
                    Err("Permission denied".to_string())
                }
            }
            _ => Err("Unknown action".to_string()),
        };

        if res.is_ok() {
            succeeded += 1;
        } else {
            skipped += 1;
        }
    }

    let msg = if skipped == 0 {
        format!(
            "Operation '{}' succeeded on {} issue(s).",
            action, succeeded
        )
    } else {
        format!(
            "Operation '{}': {} issue(s) updated, {} skipped due to permissions or errors.",
            action, succeeded, skipped
        )
    };

    Redirect::to(&format!("/view_all_bug_page?msg={}", url_encode(&msg))).into_response()
}

pub async fn bug_stick(State(db): State<Db>, ctx: AuthContext, Path(id): Path<i64>) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 70 {
        return forbidden(&ctx, "Manager access required to toggle sticky status.");
    }

    let new_sticky = if bug.sticky == 1 { 0 } else { 1 };
    let fields = BugUpdate {
        sticky: Some(new_sticky),
        ..Default::default()
    };

    if let Err(e) = db.update_bug_fields(id, ctx.user.id, fields) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    Redirect::to(&format!("/view_bug_page/{}", id)).into_response()
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
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
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

pub async fn bug_change_status(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form_data): axum::Form<Vec<(String, String)>>,
) -> Response {
    let mut form_map: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_data {
        form_map.entry(k).or_default().push(v);
    }

    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view private issue.");
    }

    let new_status: i32 = match get_single_val(&form_map, "new_status").and_then(|v| v.parse().ok())
    {
        Some(s) => s,
        None => {
            return Redirect::to(&format!("/view_bug_page/{}?err=Invalid+status", id))
                .into_response()
        }
    };

    let required_level = if new_status == 90 {
        70 // Close requires Manager
    } else if new_status == 80 || bug.status >= 80 {
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

    let resolution = get_single_val(&form_map, "resolution")
        .and_then(|v| v.parse().ok())
        .unwrap_or(bug.resolution);
    let note = get_single_val(&form_map, "note").map(|s| s.to_string());

    let custom_fields = db
        .list_custom_fields_for_project(bug.project_id)
        .unwrap_or_default();
    let current_values = db.get_custom_field_values_for_bug(id).unwrap_or_default();
    let versions = db.list_versions(bug.project_id).unwrap_or_default();
    let cats = db
        .list_effective_categories(bug.project_id)
        .unwrap_or_default();

    // Check require_resolved and require_closed & limit writes to fields displayed for target status
    let mut custom_field_values_to_set = Vec::new();
    let is_resolving = new_status == 80;
    let is_closing = new_status == 90;

    for cf in &custom_fields {
        if eff < cf.access_level_r {
            continue;
        }

        // Only process fields that are displayed for the target status
        let displays_for_status = if is_closing {
            cf.display_closed != 0
        } else if is_resolving {
            cf.display_resolved != 0
        } else {
            false
        };

        if !displays_for_status {
            continue;
        }

        let can_write = eff >= cf.access_level_rw;
        let is_req = if is_closing {
            can_write && cf.require_closed != 0
        } else if is_resolving {
            can_write && cf.require_resolved != 0
        } else {
            false
        };

        let is_present = form_map.contains_key(&format!("custom_field_{}", cf.id))
            || form_map.contains_key(&format!("custom_field_{}_present", cf.id));

        let current_val = current_values
            .iter()
            .find(|cv| cv.field_id == cf.id)
            .map(|cv| cv.value.as_str())
            .unwrap_or("");

        if can_write && is_present {
            let val = get_custom_field_submitted_value(&form_map, cf.id, cf.field_type)
                .unwrap_or_default();
            let resolved =
                crate::models::resolve_possible_values(&cf.possible_values, &versions, &cats);
            if let Err(e) = crate::models::validate_custom_field_value(cf, &val, &resolved, is_req)
            {
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err={}",
                    id,
                    url_encode(&sanitize_error_message(&e))
                ))
                .into_response();
            }
            custom_field_values_to_set.push((cf.id, val));
        } else if is_req && current_val.trim().is_empty() {
            let action_name = if is_closing { "close" } else { "resolve" };
            return Redirect::to(&format!(
                "/view_bug_page/{}?err={}",
                id,
                url_encode(&format!(
                    "Custom field '{}' is required to {} this issue",
                    cf.name, action_name
                ))
            ))
            .into_response();
        }
    }

    match db.change_bug_status(id, ctx.user.id, new_status, resolution, note.as_deref()) {
        Ok(()) => {
            for (cf_id, val) in custom_field_values_to_set {
                if let Err(e) = db.set_custom_field_value(cf_id, id, ctx.user.id, &val) {
                    return Redirect::to(&format!("/view_bug_page/{}?err={}", id, url_encode(&e)))
                        .into_response();
                }
            }
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
    if eff < 25 || !Db::can_view_bug(ctx.user.id, eff, &bug) {
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

#[derive(Deserialize, Default)]
pub struct BugnoteUpdateForm {
    pub note: Option<String>,
}

pub async fn bugnote_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
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
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view issue.");
    }

    if note.view_state == 50 && eff < 55 {
        return forbidden(&ctx, "Insufficient access to view private bugnote.");
    }

    let can_edit = eff >= 70 || (eff >= 25 && note.reporter_id == ctx.user.id);
    if !can_edit {
        return forbidden(&ctx, "Insufficient access to edit this bugnote.");
    }

    let flash_err = params.get("err").map(|s| s.as_str());
    views::bugs::render_bugnote_edit_page(&ctx, &bug, &note, flash_err).into_response()
}

pub async fn bugnote_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<BugnoteUpdateForm>,
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
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view issue.");
    }

    if note.view_state == 50 && eff < 55 {
        return forbidden(&ctx, "Insufficient access to view private bugnote.");
    }

    let can_edit = eff >= 70 || (eff >= 25 && note.reporter_id == ctx.user.id);
    if !can_edit {
        return forbidden(&ctx, "Insufficient access to edit this bugnote.");
    }

    let note_text = form.note.unwrap_or_default();
    if let Err(e) = db.update_bugnote(id, ctx.user.id, &note_text) {
        return Redirect::to(&format!(
            "/bugnote_edit_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    Redirect::to(&format!("/view_bug_page/{}", bug.id)).into_response()
}

#[derive(Deserialize, Default)]
pub struct BugnoteViewStateForm {
    pub view_state: Option<i32>,
}

pub async fn bugnote_set_view_state(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    form_res: Result<axum::Form<BugnoteViewStateForm>, axum::extract::rejection::FormRejection>,
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
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view issue.");
    }

    if eff < 55 {
        return forbidden(
            &ctx,
            "Developer access required to change bugnote visibility.",
        );
    }

    let target_vs = match form_res.ok().and_then(|f| f.0.view_state) {
        Some(vs) if vs == 10 || vs == 50 => vs,
        _ => {
            if note.view_state == 50 {
                10
            } else {
                50
            }
        }
    };

    if let Err(e) = db.set_bugnote_view_state(id, ctx.user.id, target_vs) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug.id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    Redirect::to(&format!("/view_bug_page/{}", bug.id)).into_response()
}

#[derive(Deserialize, Default)]
pub struct BugRevisionQuery {
    pub bugnote_id: Option<i64>,
    pub err: Option<String>,
}

pub async fn bug_revision_view_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    Query(query): Query<BugRevisionQuery>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view issue.");
    }

    let filter_note_id = query.bugnote_id.filter(|&n| n > 0);

    // If specific bugnote_id requested, check its visibility
    if let Some(nid) = filter_note_id {
        let note = match db.get_bugnote_by_id(nid) {
            Ok(Some(n)) => n,
            _ => return not_found(&ctx, "Bugnote not found."),
        };
        if note.bug_id != bug_id {
            return not_found(&ctx, "Bugnote does not belong to this issue.");
        }
        if note.view_state == 50 && eff < 55 {
            return forbidden(
                &ctx,
                "Insufficient access to view private bugnote revisions.",
            );
        }
    }

    let mut revisions = match db.list_bug_revisions(bug_id, filter_note_id) {
        Ok(r) => r,
        Err(_) => return forbidden(&ctx, "Unable to load revisions."),
    };

    // Never expose revisions belonging to private bugnotes (view_state == 50) to users with eff < 55
    // Use allow-list: only keep bug revisions (bugnote_id == 0) or revisions of notes visible to this user
    if eff < 55 {
        let visible_notes = match db.list_bugnotes(bug_id, eff) {
            Ok(notes) => notes,
            Err(_) => return forbidden(&ctx, "Unable to verify bugnote permissions."),
        };
        let allowed_note_ids: HashSet<i64> = visible_notes.iter().map(|n| n.id).collect();
        revisions.retain(|r| r.bugnote_id == 0 || allowed_note_ids.contains(&r.bugnote_id));
    }

    let users = db.list_users(None, false).unwrap_or_default();
    let mut users_map = HashMap::new();
    for u in users {
        users_map.insert(u.id, u.username);
    }

    let can_drop = eff >= 70;
    views::bugs::render_bug_revision_view_page(
        &ctx,
        &bug,
        &revisions,
        &users_map,
        can_drop,
        filter_note_id,
        query.err.as_deref(),
    )
    .into_response()
}

pub async fn bug_revision_drop(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(revision_id): Path<i64>,
) -> Response {
    let rev = match db.get_bug_revision_by_id(revision_id) {
        Ok(Some(r)) => r,
        _ => return not_found(&ctx, "Revision not found."),
    };

    let bug = match db.get_bug(rev.bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view issue.");
    }
    if eff < 70 {
        return forbidden(&ctx, "Manager access required to drop revisions.");
    }

    if let Err(e) = db.drop_bug_revision(revision_id) {
        return Redirect::to(&format!(
            "/bug_revision_view_page/{}?err={}",
            bug.id,
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response();
    }

    if rev.bugnote_id > 0 {
        Redirect::to(&format!(
            "/bug_revision_view_page/{}?bugnote_id={}",
            bug.id, rev.bugnote_id
        ))
        .into_response()
    } else {
        Redirect::to(&format!("/bug_revision_view_page/{}", bug.id)).into_response()
    }
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

    let dest_bug = match db.get_bug(form.dest_bug_id) {
        Ok(Some(b)) => b,
        _ => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Failed+to+add+relationship",
                bug_id
            ))
            .into_response()
        }
    };
    let dest_eff = db
        .effective_access_level(&ctx.user, dest_bug.project_id)
        .unwrap_or(0);
    if dest_eff < 10 || (dest_bug.view_state == 50 && dest_eff < 55) {
        return forbidden(&ctx, "Insufficient access to target issue.");
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
    if eff < 25 || !Db::can_view_bug(ctx.user.id, eff, &bug) {
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

pub(crate) fn sanitize_download_filename(raw: &str) -> String {
    let base = raw.rsplit(&['/', '\\'][..]).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c == '"' || c == ';' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.');
    if trimmed.is_empty() {
        "attachment.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn sanitize_content_type(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.chars().any(|c| c.is_control())
        || axum::http::HeaderValue::from_str(trimmed).is_err()
    {
        "application/octet-stream".to_string()
    } else {
        trimmed.to_string()
    }
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
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to download this attachment.");
    }

    let content_type = sanitize_content_type(&meta.file_type);
    let safe_filename = sanitize_download_filename(&meta.filename);
    let disp = format!("attachment; filename=\"{}\"", safe_filename);

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

#[cfg(test)]
#[path = "bugs_test.rs"]
mod tests;
