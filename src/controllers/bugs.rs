use super::{sanitize_error_message, url_encode};
use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{
    clamp_per_page, parse_custom_field_column_id, sanitize_spreadsheet_cell, sort_bugs, Bug,
    BugFilter, BugUpdate, ColumnContext, CustomField, CustomFieldType, ListBugsQuery, User,
    FILTER_MATCH_ANY,
};
use crate::views;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::models::extract_mentions;

pub fn url_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        match b {
            b'+' => bytes.push(b' '),
            b'%' => {
                let h1 = chars.next();
                let h2 = chars.next();
                if let (Some(c1), Some(c2)) = (h1, h2) {
                    let hex = [c1, c2];
                    if let Ok(s_hex) = std::str::from_utf8(&hex) {
                        if let Ok(val) = u8::from_str_radix(s_hex, 16) {
                            bytes.push(val);
                            continue;
                        }
                    }
                    bytes.push(b'%');
                    bytes.push(c1);
                    bytes.push(c2);
                } else {
                    bytes.push(b'%');
                    if let Some(c) = h1 {
                        bytes.push(c);
                    }
                }
            }
            _ => bytes.push(b),
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn parse_urlencoded_form(body: &[u8]) -> Vec<(String, String)> {
    let s = String::from_utf8_lossy(body);
    let mut pairs = Vec::new();
    for part in s.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            pairs.push((url_decode(k), url_decode(v)));
        } else {
            pairs.push((url_decode(part), String::new()));
        }
    }
    pairs
}

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct BugReportQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
    pub created_bug_id: Option<i64>,
    pub project_id: Option<i64>,
    pub category_id: Option<i64>,
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

pub(crate) fn filter_from_query(
    db: &Db,
    user: &User,
    active_project_id: i64,
    q: &ListBugsQuery,
    strip_inaccessible_cfs: bool,
) -> BugFilter {
    let kw =
        q.q.clone()
            .or_else(|| q.search.clone())
            .filter(|s| !s.trim().is_empty());

    let effective_project_id = q.project_id.filter(|&v| v > 0).unwrap_or(active_project_id);

    let mut filtered_cfs = std::collections::BTreeMap::new();
    if strip_inaccessible_cfs {
        let accessible_cf_ids: HashSet<i64> = db
            .list_accessible_custom_fields_for_user(user, effective_project_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|cf| cf.filter_by != 0)
            .map(|cf| cf.id)
            .collect();
        for (fid, vals) in &q.custom_fields {
            if accessible_cf_ids.contains(fid) {
                let cleaned: Vec<String> = vals
                    .iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "[any]")
                    .collect();
                if !cleaned.is_empty() {
                    filtered_cfs.insert(*fid, cleaned);
                }
            }
        }
    } else {
        for (fid, vals) in &q.custom_fields {
            if *fid > 0 {
                let cleaned: Vec<String> = vals
                    .iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "[any]")
                    .collect();
                if !cleaned.is_empty() {
                    filtered_cfs.insert(*fid, cleaned);
                }
            }
        }
    }

    let statuses = q.effective_statuses();
    let priorities = q.effective_priorities();
    let severities = q.effective_severities();
    let category_ids = q.effective_category_ids();
    let reporter_ids = q.effective_reporter_ids();
    let handler_ids = q.effective_handler_ids();
    let tag_ids = q.effective_tag_ids();
    let monitored_by_ids = q.effective_monitored_by_ids();
    let target_versions = q.effective_target_versions();
    let fixed_in_versions = q.effective_fixed_in_versions();

    BugFilter {
        project_id: (effective_project_id > 0).then_some(effective_project_id),
        search: kw,
        status: (statuses.len() == 1).then(|| statuses[0]),
        hide_status: q.hide_status.filter(|&v| v > 0),
        priority: (priorities.len() == 1).then(|| priorities[0]),
        severity: (severities.len() == 1).then(|| severities[0]),
        category_id: (category_ids.len() == 1).then(|| category_ids[0]),
        reporter_id: (reporter_ids.len() == 1).then(|| reporter_ids[0]),
        handler_id: (handler_ids.len() == 1).then(|| handler_ids[0]),
        tag_id: (tag_ids.len() == 1).then(|| tag_ids[0]),
        monitored_by: (monitored_by_ids.len() == 1).then(|| monitored_by_ids[0]),
        target_version: None,
        fixed_in_version: None,
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
        match_type: if q.match_type == Some(FILTER_MATCH_ANY) {
            FILTER_MATCH_ANY
        } else {
            0
        },
        statuses: if statuses.len() > 1 {
            statuses
        } else {
            Vec::new()
        },
        hide_statuses: q.hide_statuses.iter().copied().filter(|&v| v > 0).collect(),
        priorities: if priorities.len() > 1 {
            priorities
        } else {
            Vec::new()
        },
        severities: if severities.len() > 1 {
            severities
        } else {
            Vec::new()
        },
        category_ids: if category_ids.len() > 1 {
            category_ids
        } else {
            Vec::new()
        },
        reporter_ids: if reporter_ids.len() > 1 {
            reporter_ids
        } else {
            Vec::new()
        },
        handler_ids: if handler_ids.len() > 1 {
            handler_ids
        } else {
            Vec::new()
        },
        tag_ids: if tag_ids.len() > 1 {
            tag_ids
        } else {
            Vec::new()
        },
        monitored_by_ids: if monitored_by_ids.len() > 1 {
            monitored_by_ids
        } else {
            Vec::new()
        },
        resolutions: q.resolutions.iter().copied().filter(|&v| v > 0).collect(),
        reproducibilities: q
            .reproducibilities
            .iter()
            .copied()
            .filter(|&v| v > 0)
            .collect(),
        view_state: q.view_state.filter(|&v| v == 10 || v == 50),
        sticky: q.sticky.filter(|&v| v == 0 || v == 1),
        versions: q
            .versions
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "[any]")
            .collect(),
        target_versions,
        fixed_in_versions,
        date_submitted_from: q
            .date_submitted_from
            .as_deref()
            .map(str::trim)
            .filter(|s| crate::models::parse_date_filter_bound(s, false).is_some())
            .map(String::from),
        date_submitted_to: q
            .date_submitted_to
            .as_deref()
            .map(str::trim)
            .filter(|s| crate::models::parse_date_filter_bound(s, true).is_some())
            .map(String::from),
        last_updated_from: q
            .last_updated_from
            .as_deref()
            .map(str::trim)
            .filter(|s| crate::models::parse_date_filter_bound(s, false).is_some())
            .map(String::from),
        last_updated_to: q
            .last_updated_to
            .as_deref()
            .map(str::trim)
            .filter(|s| crate::models::parse_date_filter_bound(s, true).is_some())
            .map(String::from),
        relationship_type: q.relationship_type.filter(|&rt| (-1..=6).contains(&rt)),
        relationship_bug_id: q.relationship_bug_id.filter(|&id| id > 0),
        note_user_ids: q.note_user_ids.iter().copied().filter(|&v| v > 0).collect(),
        custom_fields: filtered_cfs,
    }
}

#[derive(Debug)]
pub(crate) enum FilterResolveError {
    Forbidden(&'static str),
    NotFound(&'static str),
    Internal(&'static str),
}

pub(crate) fn resolve_bug_filter_for_user(
    db: &Db,
    user: &User,
    active_project_id: i64,
    q: &ListBugsQuery,
) -> Result<BugFilter, FilterResolveError> {
    if let Some(fid) = q.filter_id.filter(|&id| id > 0) {
        match db.get_filter(fid) {
            Ok(Some((sf, mut bf))) => {
                if sf.is_public == 0 && sf.user_id != user.id {
                    return Err(FilterResolveError::Forbidden(
                        "You do not have permission to view this private filter.",
                    ));
                }
                if sf.project_id > 0
                    && db.effective_access_level(user, sf.project_id).unwrap_or(0) < 10
                {
                    return Err(FilterResolveError::Forbidden(
                        "You do not have access to this filter's project.",
                    ));
                }

                if let Some(pid) = q.project_id.filter(|&v| v > 0) {
                    bf.project_id = Some(pid);
                } else if active_project_id > 0 {
                    bf.project_id = Some(active_project_id);
                } else if let Some(bf_pid) = bf.project_id.filter(|&p| p > 0) {
                    if db.effective_access_level(user, bf_pid).unwrap_or(0) < 10 {
                        return Err(FilterResolveError::Forbidden(
                            "You do not have access to this project.",
                        ));
                    }
                }
                let tv = q.effective_target_versions();
                if !tv.is_empty() {
                    bf.target_version = None;
                    bf.target_versions = tv;
                }
                let fv = q.effective_fixed_in_versions();
                if !fv.is_empty() {
                    bf.fixed_in_version = None;
                    bf.fixed_in_versions = fv;
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
                let eff_pid = bf.project_id.unwrap_or(0);
                let accessible_cf_ids: HashSet<i64> = db
                    .list_accessible_custom_fields_for_user(user, eff_pid)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|cf| cf.filter_by != 0)
                    .map(|cf| cf.id)
                    .collect();
                bf.retain_custom_fields(&accessible_cf_ids);
                return Ok(bf);
            }
            Ok(None) => return Err(FilterResolveError::NotFound("Saved filter not found.")),
            Err(_) => return Err(FilterResolveError::Internal("Failed to load saved filter.")),
        }
    }

    Ok(filter_from_query(db, user, active_project_id, q, false))
}

fn resolve_bug_filter(
    db: &Db,
    ctx: &AuthContext,
    q: &ListBugsQuery,
) -> Result<BugFilter, Box<Response>> {
    resolve_bug_filter_for_user(db, &ctx.user, ctx.active_project_id, q).map_err(|err| match err {
        FilterResolveError::Forbidden(msg) => Box::new(forbidden(ctx, msg)),
        FilterResolveError::NotFound(msg) => Box::new(not_found(ctx, msg)),
        FilterResolveError::Internal(msg) => Box::new(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(ctx, "Database Error", Some(msg), ""),
            )
                .into_response(),
        ),
    })
}

pub fn router() -> Router<Db> {
    let upload_limit = max_file_size()
        .saturating_mul(10)
        .saturating_add(2 * 1024 * 1024)
        .max(25 * 1024 * 1024);
    Router::new()
        .route("/view_all_bug_page", get(view_all_bug_page))
        .route("/view_bug_page/:id", get(view_bug_page))
        .route("/bug_report_page", get(bug_report_page))
        .route(
            "/bug_report",
            post(bug_report).layer(DefaultBodyLimit::max(upload_limit)),
        )
        .route("/bug_reminder_page/:id", get(bug_reminder_page))
        .route("/bug_reminder/:id", post(bug_reminder))
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
        .route("/print_all_bug_page", get(print_all_bug_page))
        .route("/excel_all_bug_page", get(excel_all_bug_page))
        .route("/word_all_bug_page", get(word_all_bug_page))
        .route("/bug_export_word", get(word_all_bug_page))
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

    let mut eff_cache: HashMap<i64, i32> = HashMap::new();
    let mut editable_bug_ids = HashSet::new();
    for list in [
        &assigned_to_me,
        &unassigned,
        &reported_by_me,
        &resolved,
        &recently_modified,
        &monitored_by_me,
    ] {
        for b in list.iter().take(10) {
            let eff = *eff_cache.entry(b.project_id).or_insert_with(|| {
                db.effective_access_level(&ctx.user, b.project_id)
                    .unwrap_or(0)
            });
            if eff >= 40 || (eff >= 25 && b.reporter_id == ctx.user.id) {
                editable_bug_ids.insert(b.id);
            }
        }
    }

    let categories_map = db.list_all_categories_map().unwrap_or_default();
    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();
    let timeline_events = db
        .list_timeline_events(&ctx.user, pid, 25)
        .unwrap_or_default();
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();

    let meta = views::bugs::MyViewMeta {
        categories_map,
        projects_map,
        editable_bug_ids,
        timeline_events,
        users_map,
        active_project_id: ctx.active_project_id,
    };

    views::bugs::render_my_view_page(
        &ctx,
        &assigned_to_me,
        &unassigned,
        &reported_by_me,
        &resolved,
        &recently_modified,
        &monitored_by_me,
        &meta,
    )
    .into_response()
}

fn sanitize_csv_cell(val: &str) -> String {
    let safe = sanitize_spreadsheet_cell(val);
    if safe.contains(',') || safe.contains('"') || safe.contains('\n') || safe.contains('\r') {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

fn check_project_filter_access(
    db: &Db,
    ctx: &AuthContext,
    override_project_id: Option<i64>,
) -> Option<Response> {
    if ctx.active_project_id > 0 {
        let eff = db
            .effective_access_level(&ctx.user, ctx.active_project_id)
            .unwrap_or(0);
        if eff < 10 {
            return Some(forbidden(ctx, "You do not have access to this project."));
        }
    }
    if let Some(pid) = override_project_id.filter(|&v| v > 0) {
        let eff = db.effective_access_level(&ctx.user, pid).unwrap_or(0);
        if eff < 10 {
            return Some(forbidden(ctx, "You do not have access to this project."));
        }
    }
    None
}

fn word_export_max_rows() -> usize {
    std::env::var("MANTIS_EXPORT_MAX_ROWS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(1000)
}

struct PreparedExportBugs {
    bugs: Vec<Bug>,
    bug_eff_levels: HashMap<i64, i32>,
    projects_map: HashMap<i64, String>,
    users_map: HashMap<i64, String>,
    categories_map: HashMap<i64, String>,
    accessible_cfs: Vec<CustomField>,
    project_title: String,
    effective_project_id: i64,
}

struct ExportTabularDataset {
    bugs: Vec<Bug>,
    col_keys: Vec<String>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    project_title: String,
}

fn load_filtered_export_bugs(
    db: &Db,
    ctx: &AuthContext,
    q: &ListBugsQuery,
) -> Result<PreparedExportBugs, Box<Response>> {
    if let Some(resp) = check_project_filter_access(db, ctx, q.project_id) {
        return Err(Box::new(resp));
    }

    let filter = resolve_bug_filter(db, ctx, q)?;
    if let Some(resp) = check_project_filter_access(db, ctx, filter.project_id) {
        return Err(Box::new(resp));
    }
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

    let mut eff_cache: HashMap<i64, i32> = HashMap::new();
    let mut bug_eff_levels: HashMap<i64, i32> = HashMap::with_capacity(bugs.len());
    for b in &bugs {
        let eff = *eff_cache.entry(b.project_id).or_insert_with(|| {
            db.effective_access_level(&ctx.user, b.project_id)
                .unwrap_or(0)
        });
        bug_eff_levels.insert(b.id, eff);
    }

    let effective_project_id = filter.project_id.unwrap_or(ctx.active_project_id);
    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();
    let project_title = if effective_project_id == 0 {
        "All Projects".to_string()
    } else {
        projects_map
            .get(&effective_project_id)
            .cloned()
            .unwrap_or_else(|| format!("Project #{}", effective_project_id))
    };
    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();
    let categories_map = db.list_all_categories_map().unwrap_or_default();
    let accessible_cfs = db
        .list_accessible_custom_fields_for_user(&ctx.user, effective_project_id)
        .unwrap_or_default();

    Ok(PreparedExportBugs {
        bugs,
        bug_eff_levels,
        projects_map,
        users_map,
        categories_map,
        accessible_cfs,
        project_title,
        effective_project_id,
    })
}

fn prepare_tabular_export_dataset(
    db: &Db,
    ctx: &AuthContext,
    q: &ListBugsQuery,
    col_ctx: ColumnContext,
    format_last_updated: bool,
) -> Result<ExportTabularDataset, Box<Response>> {
    let prepared = load_filtered_export_bugs(db, ctx, q)?;
    let accessible_cf_ids: HashSet<i64> = prepared.accessible_cfs.iter().map(|c| c.id).collect();
    let raw_cols = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            prepared.effective_project_id,
            col_ctx,
            &accessible_cf_ids,
        )
        .unwrap_or_else(|_| {
            crate::models::default_columns_for_context(col_ctx, prepared.effective_project_id)
        });
    let col_keys: Vec<String> = raw_cols
        .into_iter()
        .filter(|c| c != "selection" && c != "edit")
        .collect();

    let cf_names: HashMap<i64, String> = prepared
        .accessible_cfs
        .iter()
        .map(|cf| (cf.id, cf.name.clone()))
        .collect();
    let cf_access_r: HashMap<i64, i32> = prepared
        .accessible_cfs
        .iter()
        .map(|cf| (cf.id, cf.access_level_r))
        .collect();
    let cf_ids_in_cols: HashSet<i64> = col_keys
        .iter()
        .filter_map(|c| parse_custom_field_column_id(c))
        .collect();

    let bug_ids: Vec<i64> = prepared.bugs.iter().map(|b| b.id).collect();
    let mut cf_values_map = if !cf_ids_in_cols.is_empty() {
        db.get_bugs_custom_field_values_map(&ctx.user, &prepared.bugs, &cf_ids_in_cols)
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    cf_values_map.retain(|(bug_id, cf_id), _| {
        let eff = prepared.bug_eff_levels.get(bug_id).copied().unwrap_or(0);
        let req = cf_access_r.get(cf_id).copied().unwrap_or(i32::MAX);
        eff >= req
    });

    let tags_map = if col_keys.iter().any(|c| c == "tags") {
        db.get_bugs_tags_map(&bug_ids).unwrap_or_default()
    } else {
        HashMap::new()
    };
    let texts_map = if col_keys.iter().any(|c| {
        matches!(
            c.as_str(),
            "description" | "steps_to_reproduce" | "additional_information"
        )
    }) {
        db.get_bugs_texts_map(&bug_ids).unwrap_or_default()
    } else {
        HashMap::new()
    };
    let (note_counts, file_counts) = if col_keys
        .iter()
        .any(|c| c == "bugnotes_count" || c == "attachment_count")
    {
        db.get_bug_note_and_file_counts(&bug_ids, &prepared.bug_eff_levels)
            .unwrap_or_default()
    } else {
        (HashMap::new(), HashMap::new())
    };

    let headers: Vec<String> = col_keys
        .iter()
        .map(|col| {
            if let Some(cf_id) = parse_custom_field_column_id(col) {
                cf_names.get(&cf_id).cloned().unwrap_or_else(|| col.clone())
            } else {
                crate::models::standard_column_label(col)
                    .unwrap_or(col.as_str())
                    .to_string()
            }
        })
        .collect();

    let mut rows = Vec::with_capacity(prepared.bugs.len());
    for b in &prepared.bugs {
        let mut row = Vec::with_capacity(col_keys.len());
        for col in &col_keys {
            let val = match col.as_str() {
                "id" => format!("{:07}", b.id),
                "project_id" => prepared
                    .projects_map
                    .get(&b.project_id)
                    .cloned()
                    .unwrap_or_default(),
                "reporter_id" => prepared
                    .users_map
                    .get(&b.reporter_id)
                    .cloned()
                    .unwrap_or_default(),
                "handler_id" => {
                    if b.handler_id > 0 {
                        prepared
                            .users_map
                            .get(&b.handler_id)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    }
                }
                "priority" => views::bugs::priority_label(b.priority).to_string(),
                "severity" => views::bugs::severity_label(b.severity).to_string(),
                "reproducibility" => {
                    views::bugs::reproducibility_label(b.reproducibility).to_string()
                }
                "status" => views::bugs::status_label(b.status).to_string(),
                "resolution" => views::bugs::resolution_label(b.resolution).to_string(),
                "view_state" => if b.view_state == 50 {
                    "private"
                } else {
                    "public"
                }
                .to_string(),
                "category_id" => prepared
                    .categories_map
                    .get(&b.category_id)
                    .cloned()
                    .unwrap_or_else(|| "General".to_string()),
                "version" => b.version.clone(),
                "target_version" => b.target_version.clone(),
                "fixed_in_version" => b.fixed_in_version.clone(),
                "build" => b.build.clone(),
                "platform" => b.platform.clone(),
                "os" => b.os.clone(),
                "os_build" => b.os_build.clone(),
                "date_submitted" => views::format_timestamp(b.date_submitted),
                "last_updated" => {
                    if format_last_updated {
                        views::format_timestamp(b.last_updated)
                    } else {
                        b.last_updated.to_string()
                    }
                }
                "due_date" => views::format_date(b.due_date),
                "sticky" => (if b.sticky == 1 { "yes" } else { "no" }).to_string(),
                "bugnotes_count" => note_counts.get(&b.id).copied().unwrap_or(0).to_string(),
                "attachment_count" => file_counts.get(&b.id).copied().unwrap_or(0).to_string(),
                "tags" => tags_map
                    .get(&b.id)
                    .map(|v| v.join(", "))
                    .unwrap_or_default(),
                "summary" => b.summary.clone(),
                "description" => texts_map
                    .get(&b.id)
                    .map(|t| t.0.clone())
                    .unwrap_or_default(),
                "steps_to_reproduce" => texts_map
                    .get(&b.id)
                    .map(|t| t.1.clone())
                    .unwrap_or_default(),
                "additional_information" => texts_map
                    .get(&b.id)
                    .map(|t| t.2.clone())
                    .unwrap_or_default(),
                other => {
                    if let Some(cf_id) = parse_custom_field_column_id(other) {
                        cf_values_map
                            .get(&(b.id, cf_id))
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    }
                }
            };
            row.push(val);
        }
        rows.push(row);
    }

    Ok(ExportTabularDataset {
        bugs: prepared.bugs,
        col_keys,
        headers,
        rows,
        project_title: prepared.project_title,
    })
}

pub async fn csv_export(
    State(db): State<Db>,
    ctx: AuthContext,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let q = ListBugsQuery::from_raw_query(raw_query.as_deref());
    let dataset = match prepare_tabular_export_dataset(&db, &ctx, &q, ColumnContext::Csv, false) {
        Ok(d) => d,
        Err(res) => return *res,
    };

    let header_cells: Vec<String> = dataset
        .headers
        .iter()
        .map(|h| sanitize_csv_cell(h))
        .collect();
    let mut out = format!("{}\r\n", header_cells.join(","));

    for row in &dataset.rows {
        let csv_row: Vec<String> = row.iter().map(|cell| sanitize_csv_cell(cell)).collect();
        out.push_str(&csv_row.join(","));
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

pub async fn print_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let q = ListBugsQuery::from_raw_query(raw_query.as_deref());
    let dataset = match prepare_tabular_export_dataset(&db, &ctx, &q, ColumnContext::Print, true) {
        Ok(d) => d,
        Err(res) => return *res,
    };

    views::bugs::render_print_all_bug_page(
        &ctx,
        &dataset.project_title,
        &q,
        &dataset.headers,
        &dataset.col_keys,
        &dataset.bugs,
        &dataset.rows,
    )
    .into_response()
}

pub async fn excel_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let q = ListBugsQuery::from_raw_query(raw_query.as_deref());
    let dataset = match prepare_tabular_export_dataset(&db, &ctx, &q, ColumnContext::Excel, true) {
        Ok(d) => d,
        Err(res) => return *res,
    };

    let xml = views::bugs::render_excel_xml(
        &dataset.project_title,
        &dataset.headers,
        &dataset.col_keys,
        &dataset.rows,
    );

    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.ms-excel; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"issues.xml\"".to_string(),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        xml,
    )
        .into_response()
}

pub async fn word_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let q = ListBugsQuery::from_raw_query(raw_query.as_deref());
    let mut prepared = match load_filtered_export_bugs(&db, &ctx, &q) {
        Ok(p) => p,
        Err(res) => return *res,
    };

    let max_rows = word_export_max_rows();
    let truncated_from = if prepared.bugs.len() > max_rows {
        let total = prepared.bugs.len();
        prepared.bugs.truncate(max_rows);
        Some(total)
    } else {
        None
    };

    let bug_ids: Vec<i64> = prepared.bugs.iter().map(|b| b.id).collect();
    let texts_map = db.get_bugs_texts_map(&bug_ids).unwrap_or_default();
    let tags_map = db.get_bugs_tags_map(&bug_ids).unwrap_or_default();
    let cf_ids: HashSet<i64> = prepared.accessible_cfs.iter().map(|c| c.id).collect();
    let cf_values_map = if !cf_ids.is_empty() {
        db.get_bugs_custom_field_values_map(&ctx.user, &prepared.bugs, &cf_ids)
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let notes_map = db
        .get_bugs_bugnotes_map(&bug_ids, &prepared.bug_eff_levels)
        .unwrap_or_default();

    let mut entries = Vec::with_capacity(prepared.bugs.len());
    for b in prepared.bugs {
        let eff = prepared.bug_eff_levels.get(&b.id).copied().unwrap_or(0);
        let project_name = prepared
            .projects_map
            .get(&b.project_id)
            .cloned()
            .unwrap_or_else(|| format!("Project #{}", b.project_id));
        let category_name = prepared
            .categories_map
            .get(&b.category_id)
            .cloned()
            .unwrap_or_else(|| "General".to_string());
        let reporter_name = prepared
            .users_map
            .get(&b.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", b.reporter_id));
        let handler_name = if b.handler_id > 0 {
            prepared
                .users_map
                .get(&b.handler_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", b.handler_id))
        } else {
            String::new()
        };
        let (description, steps_to_reproduce, additional_information) =
            texts_map.get(&b.id).cloned().unwrap_or_default();
        let tags = tags_map.get(&b.id).cloned().unwrap_or_default();

        let mut custom_fields = Vec::new();
        for cf in &prepared.accessible_cfs {
            if eff >= cf.access_level_r {
                if let Some(val) = cf_values_map.get(&(b.id, cf.id)) {
                    custom_fields.push((cf.name.clone(), val.clone()));
                }
            }
        }

        let notes = notes_map
            .get(&b.id)
            .map(|list| {
                list.iter()
                    .map(|n| views::bugs::WordBugnoteEntry {
                        id: n.id,
                        reporter_name: prepared
                            .users_map
                            .get(&n.reporter_id)
                            .cloned()
                            .unwrap_or_else(|| format!("user#{}", n.reporter_id)),
                        date_submitted: n.date_submitted,
                        view_state: n.view_state,
                        note: n.note.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        entries.push(views::bugs::WordIssueEntry {
            bug: b,
            project_name,
            category_name,
            reporter_name,
            handler_name,
            description,
            steps_to_reproduce,
            additional_information,
            tags,
            custom_fields,
            notes,
        });
    }

    let doc =
        views::bugs::render_word_export_doc(&prepared.project_title, &entries, truncated_from);

    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/msword; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"issues.doc\"".to_string(),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        doc,
    )
        .into_response()
}

pub async fn view_all_bug_page(
    State(db): State<Db>,
    ctx: AuthContext,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let q = ListBugsQuery::from_raw_query(raw_query.as_deref());
    if let Some(resp) = check_project_filter_access(&db, &ctx, q.project_id) {
        return resp;
    }

    let filter = match resolve_bug_filter_for_user(&db, &ctx.user, ctx.active_project_id, &q) {
        Ok(f) => f,
        Err(FilterResolveError::Forbidden(msg)) => return forbidden(&ctx, msg),
        Err(FilterResolveError::NotFound(_)) => {
            return Redirect::to("/view_all_bug_page?err=Saved+filter+not+found").into_response();
        }
        Err(FilterResolveError::Internal(msg)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(&ctx, "Database Error", Some(msg), ""),
            )
                .into_response();
        }
    };
    if let Some(resp) = check_project_filter_access(&db, &ctx, filter.project_id) {
        return resp;
    }
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

    let effective_project_id = filter.project_id.unwrap_or(ctx.active_project_id);
    let mut categories_map = db.list_all_categories_map().unwrap_or_default();
    let cats = db
        .list_effective_categories(effective_project_id)
        .unwrap_or_default();
    let mut all_cats = Vec::new();
    for c in &cats {
        categories_map.insert(c.id, c.name.clone());
        all_cats.push((c.id, c.name.clone()));
    }

    let all_tags = db.list_tags().unwrap_or_default();
    let saved_filters = db
        .list_visible_filters_for_user(&ctx.user, ctx.active_project_id)
        .unwrap_or_default();

    let mut jira_links = HashMap::new();
    let mut eff_cache: HashMap<i64, i32> = HashMap::new();
    let mut bug_access_levels: HashMap<i64, i32> = HashMap::new();
    let mut editable_bug_ids = HashSet::new();
    let mut paged_bug_ids = Vec::with_capacity(paged_bugs.len());
    for b in &paged_bugs {
        paged_bug_ids.push(b.id);
        let eff = *eff_cache.entry(b.project_id).or_insert_with(|| {
            db.effective_access_level(&ctx.user, b.project_id)
                .unwrap_or(0)
        });
        bug_access_levels.insert(b.id, eff);
        if eff >= 40 || (eff >= 25 && b.reporter_id == ctx.user.id) {
            editable_bug_ids.insert(b.id);
        }
        if let Ok(Some(jl)) = db.get_jira_link_by_bug_id(b.id) {
            jira_links.insert(b.id, jl);
        }
    }

    let (note_counts, file_counts) = db
        .get_bug_note_and_file_counts(&paged_bug_ids, &bug_access_levels)
        .unwrap_or_default();

    let accessible_cfs = db
        .list_accessible_custom_fields_for_user(&ctx.user, effective_project_id)
        .unwrap_or_default();
    let accessible_cf_ids: HashSet<i64> = accessible_cfs.iter().map(|c| c.id).collect();
    let view_columns = db
        .get_user_columns_with_cfs(
            ctx.user.id,
            effective_project_id,
            ColumnContext::View,
            &accessible_cf_ids,
        )
        .unwrap_or_else(|_| {
            crate::models::default_columns_for_context(ColumnContext::View, effective_project_id)
        });
    let project_versions_list = if effective_project_id > 0 {
        db.list_versions(effective_project_id).unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut filter_custom_fields = Vec::new();
    let mut custom_field_labels = HashMap::new();
    for cf in &accessible_cfs {
        custom_field_labels.insert(
            crate::models::custom_field_column_id(cf.id),
            cf.name.clone(),
        );
        if cf.filter_by != 0 {
            let opts = crate::models::resolve_possible_values(
                &cf.possible_values,
                &project_versions_list,
                &cats,
            );
            filter_custom_fields.push((cf.clone(), opts));
        }
    }

    let cf_ids_in_cols: HashSet<i64> = view_columns
        .iter()
        .filter_map(|c| parse_custom_field_column_id(c))
        .collect();
    let custom_field_values = if !cf_ids_in_cols.is_empty() {
        db.get_bugs_custom_field_values_map(&ctx.user, &paged_bugs, &cf_ids_in_cols)
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let bug_tags_map = if view_columns.iter().any(|c| c == "tags") {
        db.get_bugs_tags_map(&paged_bug_ids).unwrap_or_default()
    } else {
        HashMap::new()
    };
    let bug_texts_map = if view_columns.iter().any(|c| {
        matches!(
            c.as_str(),
            "description" | "steps_to_reproduce" | "additional_information"
        )
    }) {
        db.get_bugs_texts_map(&paged_bug_ids).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let versions = db
        .list_distinct_bug_versions_for_user(&ctx.user, effective_project_id)
        .unwrap_or_default();

    let can_save_public = if effective_project_id > 0 {
        db.effective_access_level(&ctx.user, effective_project_id)
            .unwrap_or(0)
            >= 70
    } else {
        ctx.user.access_level >= 70
    };
    let can_save_filter = if effective_project_id > 0 {
        db.effective_access_level(&ctx.user, effective_project_id)
            .unwrap_or(0)
            >= 25
    } else if ctx.user.access_level >= 25 {
        true
    } else {
        ctx.visible_projects
            .iter()
            .filter(|(p, _)| p.enabled != 0)
            .any(|(p, _)| db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 25)
    };
    let mut deletable_filter_ids = HashSet::new();
    for sf in &saved_filters {
        let can_del = if sf.is_public == 0 {
            sf.user_id == ctx.user.id || ctx.user.access_level >= 90
        } else if sf.user_id == ctx.user.id {
            true
        } else if sf.project_id > 0 {
            *eff_cache.entry(sf.project_id).or_insert_with(|| {
                db.effective_access_level(&ctx.user, sf.project_id)
                    .unwrap_or(0)
            }) >= 70
        } else {
            ctx.user.access_level >= 70
        };
        if can_del {
            deletable_filter_ids.insert(sf.id);
        }
    }

    let meta = views::bugs::ViewIssuesMeta {
        note_counts,
        file_counts,
        editable_bug_ids,
        columns: view_columns,
        versions,
        filter_custom_fields,
        custom_field_labels,
        custom_field_values,
        bug_tags_map,
        bug_texts_map,
        can_save_public: Some(can_save_public),
        can_save_filter: Some(can_save_filter),
        deletable_filter_ids: Some(deletable_filter_ids),
    };

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
        &meta,
    )
    .into_response()
}

pub async fn bug_report_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<BugReportQuery>,
) -> Response {
    let default_proj_id = q
        .project_id
        .filter(|&pid| pid > 0 && db.effective_access_level(&ctx.user, pid).unwrap_or(0) >= 25)
        .unwrap_or_else(|| {
            if ctx.active_project_id > 0 {
                ctx.active_project_id
            } else {
                ctx.visible_projects.first().map(|(p, _)| p.id).unwrap_or(0)
            }
        });

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

    let assignable_users: Vec<User> = if default_eff >= 55 {
        db.list_assignable_users_for_project(default_proj_id, 55)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let verified_created_bug_id = q.created_bug_id.and_then(|cid| {
        if cid <= 0 {
            return None;
        }
        let b = db.get_bug(cid).ok().flatten()?;
        let eff = db
            .effective_access_level(&ctx.user, b.project_id)
            .unwrap_or(0);
        if Db::can_view_bug(ctx.user.id, eff, &b) {
            Some(b.id)
        } else {
            None
        }
    });

    let mut project_opts = Vec::new();
    for (p, _) in &ctx.visible_projects {
        let eff = db.effective_access_level(&ctx.user, p.id).unwrap_or(0);
        if eff >= 25 {
            project_opts.push((p.id, p.name.clone(), p.id == default_proj_id));
        }
    }

    let cat_pairs: Vec<_> = cats.into_iter().map(|c| (c.id, c.name)).collect();
    let selected_cat_id = q
        .category_id
        .filter(|cid| cat_pairs.iter().any(|(id, _)| id == cid));

    views::bugs::render_bug_report_page(
        &ctx,
        &project_opts,
        &cat_pairs,
        selected_cat_id,
        &versions,
        &assignable_users,
        &custom_fields,
        default_eff,
        q.err.as_deref(),
        q.msg.as_deref(),
        verified_created_bug_id,
        verified_created_bug_id.is_some(),
    )
    .into_response()
}

pub async fn bug_report(
    State(db): State<Db>,
    ctx: AuthContext,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let is_multipart = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("multipart/form-data"))
        .unwrap_or(false);

    let (form_data, uploaded_files) = if is_multipart {
        parse_multipart_data(&headers, &body)
    } else {
        (parse_urlencoded_form(&body), Vec::new())
    };

    if uploaded_files.len() > 10 {
        return Redirect::to(&format!(
            "/bug_report_page?err={}",
            url_encode("Maximum 10 files allowed per upload.")
        ))
        .into_response();
    }

    let max_bytes = max_file_size();

    for f in &uploaded_files {
        if f.data.len() > max_bytes {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Html("File exceeds maximum allowed size".to_string()),
            )
                .into_response();
        }
        if let Err(e) = validate_attachment_filename(&f.filename) {
            return Redirect::to(&format!(
                "/bug_report_page?err={}",
                url_encode(&sanitize_error_message(&e))
            ))
            .into_response();
        }
    }

    let mut form_map: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_data {
        form_map.entry(k).or_default().push(v);
    }

    let project_id: i64 = match get_single_val(&form_map, "project_id")
        .and_then(|v| v.parse().ok())
        .or((ctx.active_project_id > 0).then_some(ctx.active_project_id))
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
    let target_version = get_single_val(&form_map, "target_version")
        .map(|s| s.trim())
        .unwrap_or("");
    let versions = db.list_versions(project_id).unwrap_or_default();
    if !target_version.is_empty() {
        if eff < 55 {
            return forbidden(&ctx, "Developer access required to set target version.");
        }
        let valid_target = versions
            .iter()
            .any(|v| v.obsolete == 0 && v.released == 0 && v.version == target_version);
        if !valid_target {
            return Redirect::to(&format!(
                "/bug_report_page?project_id={}&err={}",
                project_id,
                url_encode(
                    "Invalid target version: select an unreleased version for this project."
                )
            ))
            .into_response();
        }
    }
    // handler_id=0 and an omitted field both yield None so category default assignee applies.
    let requested_handler_id = get_single_val(&form_map, "handler_id")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|&hid| hid > 0);
    if let Some(hid) = requested_handler_id {
        if eff < 55 {
            return forbidden(&ctx, "Developer access required to assign issues.");
        }
        match db.get_user_by_id(hid) {
            Ok(Some(target_user)) => {
                if target_user.enabled != 1 {
                    return Redirect::to(&format!(
                        "/bug_report_page?project_id={}&err={}",
                        project_id,
                        url_encode("Assigned user is disabled.")
                    ))
                    .into_response();
                }
                let target_eff = db
                    .effective_access_level(&target_user, project_id)
                    .unwrap_or(0);
                if target_eff < 55 {
                    return Redirect::to(&format!(
                        "/bug_report_page?project_id={}&err={}",
                        project_id,
                        url_encode("Assigned user does not have Developer access on this project.")
                    ))
                    .into_response();
                }
            }
            _ => {
                return Redirect::to(&format!(
                    "/bug_report_page?project_id={}&err={}",
                    project_id,
                    url_encode("Assigned user not found.")
                ))
                .into_response();
            }
        }
    }
    let report_stay = matches!(
        get_single_val(&form_map, "report_stay").map(str::trim),
        Some("1" | "true" | "on" | "yes")
    );
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
                    "/bug_report_page?project_id={}&err={}",
                    project_id,
                    url_encode(&sanitize_error_message(&e))
                ))
                .into_response();
            }
            custom_field_values_to_set.push((cf.id, val));
        }
    }

    let deduped =
        crate::models::collect_mentions(&[&description, &steps_to_reproduce, &additional_info]);

    let bug_id = match db.create_bug_full_with_mentions(
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
        &deduped,
        crate::db::BugCreateExtras {
            build,
            target_version,
            due_date,
            handler_id: requested_handler_id,
        },
    ) {
        Ok(id) => id,
        Err(_) => {
            return Redirect::to(&format!(
                "/bug_report_page?project_id={}&err=Failed+to+create+issue",
                project_id
            ))
            .into_response()
        }
    };

    for (cf_id, val) in custom_field_values_to_set {
        if let Err(e) = db.set_custom_field_value(cf_id, bug_id, ctx.user.id, &val) {
            return Redirect::to(&format!("/view_bug_page/{}?err={}", bug_id, url_encode(&e)))
                .into_response();
        }
    }

    if !deduped.is_empty() {
        let context_text = crate::models::combine_context(
            summary,
            &[&description, &steps_to_reproduce, &additional_info],
        );
        if let Err(e) = db.notify_bug_mentions(bug_id, ctx.user.id, &deduped, false, &context_text)
        {
            eprintln!(
                "[mentions] failed to send mention notifications on bug_report: {}",
                e
            );
        }
    }

    let dir = upload_dir();
    let mut attach_errors = Vec::new();
    for f in uploaded_files {
        if let Err(e) = db.add_bug_file(
            bug_id,
            ctx.user.id,
            &f.filename,
            &f.content_type,
            f.data,
            &dir,
        ) {
            attach_errors.push(format!("{}: {}", f.filename, e));
        }
    }

    if report_stay {
        if !attach_errors.is_empty() {
            let err_msg = format!(
                "Issue #{:07} created, but file errors: {}",
                bug_id,
                attach_errors.join("; ")
            );
            Redirect::to(&format!(
                "/bug_report_page?project_id={}&category_id={}&err={}&created_bug_id={}",
                project_id,
                category_id,
                url_encode(&sanitize_error_message(&err_msg)),
                bug_id
            ))
            .into_response()
        } else {
            Redirect::to(&format!(
                "/bug_report_page?project_id={}&category_id={}&created_bug_id={}",
                project_id, category_id, bug_id
            ))
            .into_response()
        }
    } else if !attach_errors.is_empty() {
        let err_msg = format!(
            "Issue created, but file errors: {}",
            attach_errors.join("; ")
        );
        Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode(&sanitize_error_message(&err_msg))
        ))
        .into_response()
    } else {
        Redirect::to(&format!("/view_bug_page/{}", bug_id)).into_response()
    }
}

fn build_format_context(
    db: &Db,
    viewer: &User,
    text_corpus: &[&str],
    users: &[User],
) -> views::formatting::FormatContext {
    let formatting_enabled = db.is_plugin_enabled(crate::models::PLUGIN_CORE_FORMATTING);
    let mut format_ctx = views::formatting::FormatContext {
        enabled: formatting_enabled,
        ..Default::default()
    };

    if !formatting_enabled {
        return format_ctx;
    }

    let mut issue_ids = std::collections::BTreeSet::new();
    let mut note_ids = std::collections::BTreeSet::new();

    for text in text_corpus {
        for id in crate::models::scan_issue_refs(text) {
            issue_ids.insert(id);
        }
        for id in crate::models::scan_note_refs(text) {
            note_ids.insert(id);
        }
    }

    // Cap batch DB lookups to 100 unique IDs per type in deterministic order to bound query size and prevent N+1/DoS amplification.
    let target_issue_ids: Vec<i64> = issue_ids.into_iter().take(100).collect();
    let target_note_ids: Vec<i64> = note_ids.into_iter().take(100).collect();

    let mut project_eff_cache: HashMap<i64, i32> = HashMap::new();

    match db.get_bugs_by_ids(&target_issue_ids) {
        Ok(target_bugs) => {
            for (&id, target_bug) in &target_bugs {
                let target_eff = *project_eff_cache
                    .entry(target_bug.project_id)
                    .or_insert_with(|| {
                        db.effective_access_level(viewer, target_bug.project_id)
                            .unwrap_or(0)
                    });
                if Db::can_view_bug(viewer.id, target_eff, target_bug) {
                    format_ctx.issue_refs.insert(
                        id,
                        views::formatting::IssueRefInfo {
                            id: target_bug.id,
                            status: target_bug.status,
                            summary: target_bug.summary.clone(),
                        },
                    );
                }
            }
        }
        Err(e) => eprintln!("[formatting] error batch fetching target bugs: {}", e),
    }

    match db.get_bugnotes_by_ids(&target_note_ids) {
        Ok(target_notes) => {
            let parent_bug_ids: Vec<i64> = target_notes.values().map(|n| n.bug_id).collect();
            let parent_bugs = db.get_bugs_by_ids(&parent_bug_ids).unwrap_or_default();

            for (&nid, target_note) in &target_notes {
                if let Some(target_bug) = parent_bugs.get(&target_note.bug_id) {
                    let target_eff = *project_eff_cache
                        .entry(target_bug.project_id)
                        .or_insert_with(|| {
                            db.effective_access_level(viewer, target_bug.project_id)
                                .unwrap_or(0)
                        });
                    if Db::can_view_bug(viewer.id, target_eff, target_bug)
                        && (target_note.view_state != crate::models::VIEW_STATE_PRIVATE
                            || target_eff >= crate::models::ACCESS_DEVELOPER)
                    {
                        format_ctx.note_refs.insert(nid, target_note.bug_id);
                    }
                }
            }
        }
        Err(e) => eprintln!("[formatting] error batch fetching target bugnotes: {}", e),
    }

    for u in users {
        if u.enabled != 0 {
            format_ctx.valid_usernames.insert(u.username.clone());
            format_ctx
                .valid_usernames
                .insert(u.username.to_ascii_lowercase());
        }
    }

    format_ctx
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
    let users_access_map = db
        .list_project_user_access_map(bug.project_id)
        .unwrap_or_else(|_| users.iter().map(|u| (u.id, u.access_level)).collect());

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
    let monitors = db.list_bug_monitor_users(id).unwrap_or_default();
    let is_monitoring = monitors.iter().any(|u| u.id == ctx.user.id);

    let can_clone = ctx.visible_projects.iter().any(|(p, _)| {
        p.enabled == 1 && db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 25
    });
    let can_move = eff >= 55
        && ctx.visible_projects.iter().any(|(p, _)| {
            p.id != bug.project_id
                && p.enabled == 1
                && db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 25
        });
    let assignable_users = if eff >= 55 {
        db.list_assignable_users_for_project(bug.project_id, 55)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let actions_html = views::bugs::render_bug_action_toolbar(
        bug,
        eff,
        ctx.user.id,
        &handler_name,
        &assignable_users,
        is_monitoring,
        can_clone,
        can_move,
        &custom_fields,
        &cf_values,
        &versions,
        &cats,
    );

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

    let tags_html = views::bugs::render_bug_tags_html(bug.id, &bug_tags, eff, ctx.user.id);

    let dir = upload_dir();
    let files_with_previews: Vec<(&crate::models::BugFile, String)> = files
        .iter()
        .map(|f| {
            let ext = std::path::Path::new(&f.filename)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();

            let is_safe_image =
                matches!(
                    f.file_type.to_ascii_lowercase().as_str(),
                    "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                ) && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp");

            let is_preview_text = matches!(
                ext.as_str(),
                "txt" | "log" | "diff" | "patch" | "json" | "md" | "csv" | "toml" | "yaml" | "yml"
            ) && f.filesize <= 65536;

            let text_bytes = if is_preview_text {
                db.get_bug_file(f.id, &dir)
                    .ok()
                    .flatten()
                    .map(|(_, bytes)| bytes)
            } else {
                None
            };
            let text_content = text_bytes
                .as_deref()
                .and_then(|bytes| std::str::from_utf8(bytes).ok());
            let preview_html = views::bugs::render_file_preview_html(
                f.id,
                &f.filename,
                is_safe_image,
                text_content,
            );
            (f, preview_html)
        })
        .collect();
    let files_html = views::bugs::render_bug_files_html(&files_with_previews, eff);

    let upload_form_html = if eff >= 25 {
        views::bugs::render_upload_form_html(bug.id)
    } else {
        String::new()
    };

    let rels_html = views::bugs::render_bug_relationships_html(&rels, eff);
    let rel_form_html = views::bugs::render_bug_relationship_add_form(bug.id, eff);

    let mut text_corpus = Vec::new();
    text_corpus.push(full.description.as_str());
    text_corpus.push(full.steps_to_reproduce.as_str());
    text_corpus.push(full.additional_information.as_str());
    for n in &notes {
        text_corpus.push(n.note.as_str());
    }
    let format_ctx = build_format_context(&db, &ctx.user, &text_corpus, &users);

    let notes_html = views::bugs::render_bugnotes_section_html(
        &notes,
        &users_map,
        &users_access_map,
        eff,
        ctx.user.id,
        &format_ctx,
    );

    let note_form_html = views::bugs::render_bugnote_add_form(bug.id, eff);

    let monitors_html = if eff >= 55 {
        views::bugs::render_monitors_html(&monitors, bug.id)
    } else {
        String::new()
    };

    let monitor_form_html = if eff >= 55 {
        views::bugs::render_monitor_form_html(bug.id)
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
        eff,
        &cf_values,
        &actions_html,
        &tags_html,
        &rels_html,
        &rel_form_html,
        &files_html,
        &upload_form_html,
        &notes_html,
        &note_form_html,
        &monitors_html,
        &monitor_form_html,
        &history_html,
        jira_link.as_ref(),
        &jira_section_html,
        q.msg.as_deref(),
        q.err.as_deref(),
        &format_ctx,
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

    let old_full = db.get_bug_full(id).ok().flatten();
    let mut old_mentions = HashSet::new();
    if let Some(ref f) = old_full {
        for m in extract_mentions(&f.description) {
            old_mentions.insert(m.to_ascii_lowercase());
        }
        for m in extract_mentions(&f.steps_to_reproduce) {
            old_mentions.insert(m.to_ascii_lowercase());
        }
        for m in extract_mentions(&f.additional_information) {
            old_mentions.insert(m.to_ascii_lowercase());
        }
    }

    let mut new_mentions = Vec::new();
    let mut context_parts = Vec::new();
    if let Some(ref d) = fields.description {
        let m = extract_mentions(d);
        if !m.is_empty() {
            context_parts.push(d.as_str());
        }
        new_mentions.extend(m);
    }
    if let Some(ref s) = fields.steps_to_reproduce {
        let m = extract_mentions(s);
        if !m.is_empty() {
            context_parts.push(s.as_str());
        }
        new_mentions.extend(m);
    }
    if let Some(ref a) = fields.additional_information {
        let m = extract_mentions(a);
        if !m.is_empty() {
            context_parts.push(a.as_str());
        }
        new_mentions.extend(m);
    }

    let fresh_mentions = crate::models::diff_new_mentions(&old_mentions, &new_mentions);
    let context_text = context_parts.join("\n\n");

    match db.update_bug_fields(id, ctx.user.id, fields) {
        Ok(_) => {
            // Trigger mention notifications immediately after update_bug_fields commits
            // so a custom field write failure cannot drop emails for already-saved text.
            if !fresh_mentions.is_empty() {
                if let Err(e) =
                    db.notify_bug_mentions(id, ctx.user.id, &fresh_mentions, false, &context_text)
                {
                    eprintln!(
                        "[mentions] failed to send mention notifications on update_bug: {}",
                        e
                    );
                }
            }
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

    // Intentionally do NOT re-notify historical @mentions copied from the source issue during cloning.
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
    const MAX_BULK: usize = 500;
    let mut bug_ids = Vec::new();
    let mut ignored_excess = 0;
    let mut form_map: HashMap<String, String> = HashMap::new();
    for (k, v) in form_data {
        if k == "bug_arr" {
            if bug_ids.len() >= MAX_BULK {
                ignored_excess += 1;
                continue;
            }
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
        return Redirect::to("/view_all_bug_page?err=No+issues+selected").into_response();
    }

    let action = form_map.get("action").map(|s| s.trim()).unwrap_or("");
    const KNOWN_ACTIONS: &[&str] = &[
        "MOVE",
        "COPY",
        "ASSIGN",
        "CLOSE",
        "DELETE",
        "RESOLVE",
        "UP_STATUS",
        "UP_PRIOR",
        "UP_SEV",
        "UP_CATEGORY",
        "UP_TARGET_VERSION",
        "UP_DUE_DATE",
        "VIEW_STATUS",
        "SET_STICKY",
        "ADD_NOTE",
        "ATTACH_TAGS",
    ];
    if !KNOWN_ACTIONS.contains(&action) {
        return Redirect::to("/view_all_bug_page?err=Please+select+a+valid+action").into_response();
    }

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
                    let resolution = if bug.resolution > 10 {
                        bug.resolution
                    } else {
                        20
                    };
                    db.change_bug_status(bug.id, ctx.user.id, 90, resolution, None)
                } else {
                    Err("Permission denied".to_string())
                }
            }
            "RESOLVE" => {
                if eff >= 55 {
                    let resolution = form_map
                        .get("resolution")
                        .and_then(|s| s.parse::<i32>().ok())
                        .filter(|r| [10, 20, 30, 40, 50, 60, 70, 80, 90].contains(r))
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
                    if handler_id > 0 {
                        match db.get_user_by_id(handler_id) {
                            Ok(Some(target_user)) => {
                                if target_user.enabled != 1 {
                                    Err("Assigned user is disabled".to_string())
                                } else {
                                    let target_eff = db
                                        .effective_access_level(&target_user, bug.project_id)
                                        .unwrap_or(0);
                                    if target_eff < 55 {
                                        Err("Assigned user does not have Developer access on this project".to_string())
                                    } else {
                                        db.assign_bug(bug.id, ctx.user.id, handler_id)
                                    }
                                }
                            }
                            Ok(None) => Err("Assigned user not found".to_string()),
                            Err(e) => Err(e.to_string()),
                        }
                    } else {
                        db.assign_bug(bug.id, ctx.user.id, 0)
                    }
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
                    // Intentionally do NOT re-notify historical @mentions copied from the source issue during bulk cloning.
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
                    if [10, 20, 30, 40, 50, 60].contains(&new_priority) {
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
                    if [10, 20, 30, 40, 50, 60, 70, 80].contains(&new_severity) {
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
                if ![10, 20, 30, 40, 50, 80, 90].contains(&target_status) {
                    Err("Invalid status".to_string())
                } else {
                    // Moving to Closed (90) requires eff >= 70
                    // Moving to Resolved (80) requires eff >= 55
                    // Reopening a bug that is currently resolved or closed (status >= 80) to < 80 requires eff >= 55
                    let allowed = if target_status == 90 {
                        eff >= 70
                    } else if target_status == 80 || (bug.status >= 80 && target_status < 80) {
                        eff >= 55
                    } else {
                        eff >= 40
                    };
                    if allowed {
                        let resolution = if target_status == 80 {
                            form_map
                                .get("resolution")
                                .and_then(|s| s.parse::<i32>().ok())
                                .filter(|r| [10, 20, 30, 40, 50, 60, 70, 80, 90].contains(r))
                                .unwrap_or(20)
                        } else if target_status == 90 {
                            if bug.resolution > 10 {
                                bug.resolution
                            } else {
                                20
                            }
                        } else if target_status < 80 && bug.status >= 80 {
                            10
                        } else {
                            bug.resolution
                        };
                        db.change_bug_status(bug.id, ctx.user.id, target_status, resolution, None)
                    } else {
                        Err("Permission denied".to_string())
                    }
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
                    if view_state == 10 || view_state == 50 {
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
                        Err("Invalid view state".to_string())
                    }
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
                    if sticky == 0 || sticky == 1 {
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
                        Err("Invalid sticky value".to_string())
                    }
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
                        if view_state == 50 && eff < 55 {
                            Err("Permission denied: private notes require developer access"
                                .to_string())
                        } else {
                            let valid_vs = if view_state == 50 { 50 } else { 10 };
                            let deduped = crate::models::collect_mentions(&[note_text]);
                            let res = db.add_bugnote_with_time_and_mentions(
                                bug.id,
                                ctx.user.id,
                                note_text,
                                valid_vs,
                                0,
                                &deduped,
                            );
                            if res.is_ok() && !deduped.is_empty() {
                                if let Err(e) = db.notify_bug_mentions(
                                    bug.id,
                                    ctx.user.id,
                                    &deduped,
                                    valid_vs == crate::models::VIEW_STATE_PRIVATE,
                                    note_text,
                                ) {
                                    eprintln!(
                                        "[mentions] failed to send mention notifications on bulk ADD_NOTE: {}",
                                        e
                                    );
                                }
                            }
                            res.map(|_| ())
                        }
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

    let msg = if skipped == 0 && ignored_excess == 0 {
        format!(
            "Operation '{}' succeeded on {} issue(s).",
            action, succeeded
        )
    } else if ignored_excess > 0 {
        format!(
            "Operation '{}': {} issue(s) updated, {} skipped due to permissions or errors ({} ignored, limit {}).",
            action, succeeded, skipped, ignored_excess, MAX_BULK
        )
    } else {
        format!(
            "Operation '{}': {} issue(s) updated, {} skipped due to permissions or errors.",
            action, succeeded, skipped
        )
    };

    let flash_key = if succeeded == 0 { "err" } else { "msg" };
    let return_query = form_map.get("return_query").map(|s| s.trim()).unwrap_or("");
    let safe_return_query: String = return_query
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '&' | '=' | '_' | '-' | '%' | '+' | '.' | '~')
        })
        .collect();

    let redirect_url = if safe_return_query.is_empty() {
        format!("/view_all_bug_page?{}={}", flash_key, url_encode(&msg))
    } else {
        format!(
            "/view_all_bug_page?{}&{}={}",
            safe_return_query,
            flash_key,
            url_encode(&msg)
        )
    };

    Redirect::to(&redirect_url).into_response()
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

enum ResolveMonitorTargetError {
    Forbidden(&'static str),
    RedirectError(String),
}

fn resolve_monitor_target(
    db: &Db,
    ctx: &AuthContext,
    eff: i32,
    body: &[u8],
) -> Result<User, ResolveMonitorTargetError> {
    let form: Vec<(String, String)> = parse_urlencoded_form(body);
    let target_username = form
        .iter()
        .find(|(k, _)| k == "username")
        .map(|(_, v)| v.trim())
        .filter(|s| !s.is_empty());
    let target_user_id = form
        .iter()
        .find(|(k, _)| k == "user_id")
        .and_then(|(_, v)| v.trim().parse::<i64>().ok());

    let is_targeting_other = match (target_username, target_user_id) {
        (Some(uname), _) if uname != ctx.user.username => true,
        (None, Some(uid)) if uid != ctx.user.id => true,
        _ => false,
    };

    if is_targeting_other && eff < 55 {
        return Err(ResolveMonitorTargetError::Forbidden(
            "Developer access required to manage monitors for other users.",
        ));
    }

    match (target_username, target_user_id) {
        (Some(uname), _) => match db.get_user_by_username(uname) {
            Ok(Some(u)) => Ok(u),
            _ => Err(ResolveMonitorTargetError::RedirectError(
                "User not found".to_string(),
            )),
        },
        (None, Some(uid)) => match db.get_user_by_id(uid) {
            Ok(Some(u)) => Ok(u),
            _ => Err(ResolveMonitorTargetError::RedirectError(
                "User not found".to_string(),
            )),
        },
        (None, None) => {
            if eff < 10 {
                return Err(ResolveMonitorTargetError::Forbidden(
                    "Insufficient access to monitor this issue.",
                ));
            }
            Ok(ctx.user.clone())
        }
    }
}

pub async fn bug_monitor_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    body: Bytes,
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

    let target = match resolve_monitor_target(&db, &ctx, eff, &body) {
        Ok(t) => t,
        Err(ResolveMonitorTargetError::Forbidden(msg)) => return forbidden(&ctx, msg),
        Err(ResolveMonitorTargetError::RedirectError(msg)) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err={}",
                bug_id,
                url_encode(&sanitize_error_message(&msg))
            ))
            .into_response()
        }
    };

    if target.id != ctx.user.id {
        if target.enabled == 0 {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err={}",
                bug_id,
                url_encode(&sanitize_error_message("User is disabled"))
            ))
            .into_response();
        }
        let target_eff = db
            .effective_access_level(&target, bug.project_id)
            .unwrap_or(0);
        if !Db::can_view_bug(target.id, target_eff, &bug) {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err={}",
                bug_id,
                url_encode(&sanitize_error_message(
                    "Target user does not have access to this issue"
                ))
            ))
            .into_response();
        }
    }

    if let Err(e) = db.add_bug_monitor_with_actor(bug_id, target.id, ctx.user.id) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response();
    }
    Redirect::to(&format!("/view_bug_page/{}?msg=Issue+monitored", bug_id)).into_response()
}

pub async fn bug_monitor_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    body: Bytes,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Insufficient access to view this issue.");
    }

    let target = match resolve_monitor_target(&db, &ctx, eff, &body) {
        Ok(t) => t,
        Err(ResolveMonitorTargetError::Forbidden(msg)) => return forbidden(&ctx, msg),
        Err(ResolveMonitorTargetError::RedirectError(msg)) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err={}",
                bug_id,
                url_encode(&sanitize_error_message(&msg))
            ))
            .into_response()
        }
    };

    if let Err(e) = db.remove_bug_monitor_with_actor(bug_id, target.id, ctx.user.id) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response();
    }
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
    if eff < crate::models::ACCESS_REPORTER || !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Reporter access required to add bugnotes.");
    }

    let vs = form.view_state.unwrap_or(crate::models::VIEW_STATE_PUBLIC);
    if vs == crate::models::VIEW_STATE_PRIVATE && eff < crate::models::ACCESS_DEVELOPER {
        return forbidden(&ctx, "Developer access required to add private bugnotes.");
    }

    let time_mins = if eff >= crate::models::ACCESS_DEVELOPER {
        parse_time_tracking_minutes(form.time_tracking.as_deref())
    } else {
        0
    };

    let trimmed_note = form.note.trim();
    let deduped = crate::models::collect_mentions(&[trimmed_note]);

    match db.add_bugnote_with_time_and_mentions(
        bug_id,
        ctx.user.id,
        trimmed_note,
        vs,
        time_mins,
        &deduped,
    ) {
        Ok(_) => {
            if !deduped.is_empty() {
                let is_private = vs == crate::models::VIEW_STATE_PRIVATE;
                if let Err(e) =
                    db.notify_bug_mentions(bug_id, ctx.user.id, &deduped, is_private, trimmed_note)
                {
                    eprintln!(
                        "[mentions] failed to send mention notifications on bugnote_add: {}",
                        e
                    );
                }
            }
            Redirect::to(&format!("/view_bug_page/{}?msg=Note+added", bug_id)).into_response()
        }
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
    let old_mentions: HashSet<String> = extract_mentions(&note.note)
        .into_iter()
        .map(|m| m.to_ascii_lowercase())
        .collect();
    let new_mentions = crate::models::collect_mentions(&[&note_text]);
    let fresh_mentions = crate::models::diff_new_mentions(&old_mentions, &new_mentions);

    if let Err(e) = db.update_bugnote(id, ctx.user.id, &note_text) {
        return Redirect::to(&format!(
            "/bugnote_edit_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    if !fresh_mentions.is_empty() {
        if let Err(e) = db.notify_bug_mentions(
            bug.id,
            ctx.user.id,
            &fresh_mentions,
            note.view_state == crate::models::VIEW_STATE_PRIVATE,
            &note_text,
        ) {
            eprintln!(
                "[mentions] failed to send mention notifications on bugnote_update: {}",
                e
            );
        }
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

pub fn validate_attachment_filename(filename: &str) -> Result<(), String> {
    let raw_name = filename.trim().trim_end_matches('.');
    if raw_name.is_empty() {
        return Err("Filename cannot be empty".to_string());
    }

    let ext = std::path::Path::new(raw_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if let Ok(allowed_env) = std::env::var("MANTIS_ALLOWED_FILES") {
        let allowed_list: Vec<String> = allowed_env
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if !allowed_list.is_empty() {
            if ext.is_empty() || !allowed_list.iter().any(|allowed| allowed == &ext) {
                return Err(format!("File extension '{}' is not allowed", ext));
            }
            return Ok(());
        }
    }

    let disallowed_list: Vec<String> = match std::env::var("MANTIS_DISALLOWED_FILES") {
        Ok(disallowed_env) => disallowed_env
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => vec![
            "svg", "svgz", "html", "htm", "xhtml", "js", "mjs", "exe", "bat", "cmd", "sh", "ps1",
            "php", "phtml", "xml", "xht", "shtml", "hta", "vbs", "jar", "msi", "dll", "scr", "com",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    };

    if !ext.is_empty() && disallowed_list.iter().any(|disallowed| disallowed == &ext) {
        return Err(format!(
            "File extension '{}' is not allowed for security reasons",
            ext
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

pub fn parse_multipart_data(
    headers: &HeaderMap,
    body: &[u8],
) -> (Vec<(String, String)>, Vec<UploadedFile>) {
    let mut fields = Vec::new();
    let mut files = Vec::new();

    let ct = match headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        Some(ct) => ct,
        None => return (fields, files),
    };

    let boundary = match ct.split(';').find_map(|part| {
        let p = part.trim();
        p.strip_prefix("boundary=")
            .map(|b| b.trim_matches('"').trim_matches('\'').to_string())
    }) {
        Some(b) => b,
        None => return (fields, files),
    };

    let delimiter = format!("--{}", boundary);
    let delim_bytes = delimiter.as_bytes();

    let mut cursor = 0;
    while let Some(start_offset) = body[cursor..]
        .windows(delim_bytes.len())
        .position(|w| w == delim_bytes)
    {
        let part_start = cursor + start_offset + delim_bytes.len();
        cursor = part_start;

        if body[cursor..].starts_with(b"--") {
            break;
        }

        if body[cursor..].starts_with(b"\r\n") {
            cursor += 2;
        } else if body[cursor..].starts_with(b"\n") {
            cursor += 1;
        }

        let next_boundary_offset = match body[cursor..]
            .windows(delim_bytes.len())
            .position(|w| w == delim_bytes)
        {
            Some(pos) => pos,
            None => break,
        };

        let part_data = &body[cursor..cursor + next_boundary_offset];
        cursor += next_boundary_offset;

        let (header_bytes, mut content_bytes) =
            if let Some(pos) = part_data.windows(4).position(|w| w == b"\r\n\r\n") {
                (&part_data[..pos], &part_data[pos + 4..])
            } else if let Some(pos) = part_data.windows(2).position(|w| w == b"\n\n") {
                (&part_data[..pos], &part_data[pos + 2..])
            } else {
                continue;
            };

        if let Some(stripped) = content_bytes.strip_suffix(b"\r\n") {
            content_bytes = stripped;
        } else if let Some(stripped) = content_bytes.strip_suffix(b"\n") {
            content_bytes = stripped;
        }

        let header_str = String::from_utf8_lossy(header_bytes);
        let mut name: Option<String> = None;
        let mut filename: Option<String> = None;
        let mut content_type = "application/octet-stream".to_string();

        for line in header_str.lines() {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-disposition:") {
                for param in line.split(';') {
                    let p = param.trim();
                    if let Some(n) = p.strip_prefix("name=") {
                        name = Some(n.trim_matches('"').trim_matches('\'').to_string());
                    } else if let Some(f) = p.strip_prefix("filename=") {
                        filename = Some(f.trim_matches('"').trim_matches('\'').to_string());
                    }
                }
            } else if lower.starts_with("content-type:") {
                if let Some((_, val)) = line.split_once(':') {
                    content_type = val.trim().to_string();
                }
            }
        }

        if let Some(fname) = filename {
            let trimmed_fname = fname.trim();
            if trimmed_fname.is_empty() && content_bytes.is_empty() {
                continue;
            }
            let safe_name = if trimmed_fname.is_empty() {
                "attachment".to_string()
            } else {
                trimmed_fname.to_string()
            };
            files.push(UploadedFile {
                filename: safe_name,
                content_type,
                data: content_bytes.to_vec(),
            });
        } else if let Some(n) = name {
            let val = String::from_utf8_lossy(content_bytes).to_string();
            fields.push((n, val));
        }
    }

    (fields, files)
}

#[cfg(test)]
pub(crate) fn parse_multipart_file(
    headers: &HeaderMap,
    body: &[u8],
) -> Option<(String, String, Vec<u8>)> {
    let (_, files) = parse_multipart_data(headers, body);
    files
        .into_iter()
        .next()
        .map(|f| (f.filename, f.content_type, f.data))
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

    let (_, files) = parse_multipart_data(&headers, &body);

    if files.is_empty() {
        if body.len() > max_bytes {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Html("File exceeds maximum allowed size".to_string()),
            )
                .into_response();
        }
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode(&sanitize_error_message("No valid files uploaded"))
        ))
        .into_response();
    }

    if files.len() > 10 {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode("Maximum 10 files allowed per upload.")
        ))
        .into_response();
    }

    for f in &files {
        if f.data.len() > max_bytes {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Html("File exceeds maximum allowed size".to_string()),
            )
                .into_response();
        }
    }

    let mut saved_count = 0;
    let mut errors = Vec::new();
    for f in files {
        if let Err(e) = validate_attachment_filename(&f.filename) {
            errors.push(format!("{}: {}", f.filename, e));
            continue;
        }
        match db.add_bug_file(
            bug_id,
            ctx.user.id,
            &f.filename,
            &f.content_type,
            f.data,
            &dir,
        ) {
            Ok(_) => saved_count += 1,
            Err(e) => errors.push(format!("{}: {}", f.filename, e)),
        }
    }

    if saved_count > 0 && errors.is_empty() {
        let msg = if saved_count == 1 {
            "Attachment uploaded".to_string()
        } else {
            format!("{} attachments uploaded", saved_count)
        };
        Redirect::to(&format!(
            "/view_bug_page/{}?msg={}",
            bug_id,
            url_encode(&msg)
        ))
        .into_response()
    } else if saved_count > 0 {
        let msg = format!("{} uploaded, errors: {}", saved_count, errors.join("; "));
        Redirect::to(&format!(
            "/view_bug_page/{}?msg={}",
            bug_id,
            url_encode(&sanitize_error_message(&msg))
        ))
        .into_response()
    } else {
        let err = if errors.is_empty() {
            "No files attached".to_string()
        } else {
            errors.join("; ")
        };
        Redirect::to(&format!(
            "/view_bug_page/{}?err={}",
            bug_id,
            url_encode(&sanitize_error_message(&err))
        ))
        .into_response()
    }
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
    Query(query): Query<HashMap<String, String>>,
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

    let ext = std::path::Path::new(&safe_filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let is_inline = query.get("inline").map(|v| v == "1").unwrap_or(false);
    let safe_raster_mime = matches!(
        content_type.to_ascii_lowercase().as_str(),
        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    ) && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp");

    let disp = if is_inline && safe_raster_mime {
        format!("inline; filename=\"{}\"", safe_filename)
    } else {
        format!("attachment; filename=\"{}\"", safe_filename)
    };

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disp),
            (
                header::HeaderName::from_static("x-content-type-options"),
                "nosniff".to_string(),
            ),
        ],
        bytes,
    )
        .into_response()
}

pub async fn bug_reminder_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(query): Query<FlashQuery>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 || !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Developer access required to send reminders.");
    }

    let all_users = db.list_users(None, true).unwrap_or_default();
    let eligible_users: Vec<crate::models::User> = all_users
        .into_iter()
        .filter(|u| {
            if u.enabled == 0 {
                return false;
            }
            let u_eff = db.effective_access_level(u, bug.project_id).unwrap_or(0);
            Db::can_view_bug(u.id, u_eff, &bug)
        })
        .collect();

    views::bugs::render_bug_reminder_page(
        &ctx,
        &bug,
        &eligible_users,
        query.err.as_deref(),
        query.msg.as_deref(),
    )
    .into_response()
}

pub async fn bug_reminder(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form_data): axum::Form<Vec<(String, String)>>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => return not_found(&ctx, "Issue not found."),
    };
    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 || !Db::can_view_bug(ctx.user.id, eff, &bug) {
        return forbidden(&ctx, "Developer access required to send reminders.");
    }

    let mut to_ids = Vec::new();
    let mut body = String::new();
    let mut store_as_note_opt = None;
    let mut view_state = 10;
    let mut add_monitors_opt = None;

    for (k, v) in form_data {
        match k.as_str() {
            "to" | "to[]" => {
                if let Ok(uid) = v.parse::<i64>() {
                    to_ids.push(uid);
                }
            }
            "body" | "bugnote_text" => {
                body = v;
            }
            "store_as_note" => {
                store_as_note_opt = Some(v == "1" || v == "true" || v == "on");
            }
            "view_state" => {
                if let Ok(vs) = v.parse::<i32>() {
                    view_state = if vs == 50 { 50 } else { 10 };
                }
            }
            "private" => {
                if v == "1" || v == "true" || v == "on" {
                    view_state = 50;
                }
            }
            "add_monitors" => {
                add_monitors_opt = Some(v == "1" || v == "true" || v == "on");
            }
            _ => {}
        }
    }

    let store_as_note = store_as_note_opt.unwrap_or(true);
    let add_monitors = add_monitors_opt.unwrap_or(true);

    to_ids.sort_unstable();
    to_ids.dedup();

    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Redirect::to(&format!(
            "/bug_reminder_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message("Reminder message cannot be empty"))
        ))
        .into_response();
    }
    if to_ids.is_empty() {
        return Redirect::to(&format!(
            "/bug_reminder_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(
                "At least one recipient must be selected"
            ))
        ))
        .into_response();
    }

    let mut valid_recipients = Vec::new();
    for uid in to_ids {
        if let Ok(Some(u)) = db.get_user_by_id(uid) {
            if u.enabled == 0 {
                continue;
            }
            let u_eff = db.effective_access_level(&u, bug.project_id).unwrap_or(0);
            if !Db::can_view_bug(u.id, u_eff, &bug) {
                continue;
            }
            if view_state == 50 && u_eff < 55 {
                continue;
            }
            valid_recipients.push(u);
        }
    }

    if valid_recipients.is_empty() {
        return Redirect::to(&format!(
            "/bug_reminder_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message("No eligible recipients selected"))
        ))
        .into_response();
    }

    let recipient_ids: Vec<i64> = valid_recipients.iter().map(|u| u.id).collect();

    let proj_name = db
        .get_project_by_id(bug.project_id)
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_default();
    let subject = format!("[{} {:07}] Reminder: {}", proj_name, bug.id, bug.summary);
    let recipients_str = valid_recipients
        .iter()
        .map(|u| u.username.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    {
        let conn = db.conn();
        let email_body = format!(
            "The following reminder has been sent by {} to {}:\n\n{}\n\n{}",
            ctx.user.username,
            recipients_str,
            trimmed_body,
            db.build_mantis_email_body(&conn, id, "REMINDER", None)
                .unwrap_or_default()
        );

        let min_access = if view_state == 50 { 55 } else { 10 };
        db.notify_users(&conn, id, &recipient_ids, min_access, &subject, &email_body);
    }

    if store_as_note {
        let note_text = format!("Reminder sent to {}:\n\n{}", recipients_str, trimmed_body);
        let _ = db.add_bugnote_with_type(id, ctx.user.id, &note_text, view_state, 0, 1);
    }

    if add_monitors {
        for u in &valid_recipients {
            let _ = db.add_bug_monitor_with_actor(id, u.id, ctx.user.id);
        }
    }

    Redirect::to(&format!("/view_bug_page/{}?msg=Reminder+sent", id)).into_response()
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
