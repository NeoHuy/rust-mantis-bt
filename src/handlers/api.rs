use crate::auth::{ApiAuth, AuthContext};
use crate::db::Db;
use crate::handlers::parse_bool;
use crate::models::{
    AccessLevel, BugFilter, BugPriority, BugReproducibility, BugResolution, BugSeverity, BugStatus,
    BugUpdate, ViewState,
};
use crate::ui::{html_escape, render_layout};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
    pub token: Option<String>,
}

fn forbidden_html(ctx: &AuthContext, msg: &str) -> Response {
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

// ─── Web UI: API Tokens & Plugin Management ─────────────────────────────────

pub async fn api_tokens_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    let tokens = db.list_api_tokens(ctx.user.id).unwrap_or_default();

    let token_banner = match q.token.as_deref() {
        Some(raw) if !raw.is_empty() => format!(
            r#"<div class="flash-message">
                <strong>New API Token Created!</strong> Copy it now — it will not be shown again:<br>
                <code style="display:inline-block; margin-top:6px; padding:6px 10px; background:#fff; border:1px solid #b2dba1; font-weight:bold; font-size:13px;">{}</code>
            </div>"#,
            html_escape(raw)
        ),
        _ => String::new(),
    };

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    for t in &tokens {
        let used_str = if t.date_used > 0 {
            format!("Used (ts: {})", t.date_used)
        } else {
            "Never used".to_string()
        };
        rows_html.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <form method="POST" action="/api_token_revoke/{}" style="display:inline;">
                        <button type="submit" class="btn btn-danger btn-sm">Revoke</button>
                    </form>
                </td>
            </tr>"#,
            t.id,
            html_escape(&t.name),
            t.date_created,
            used_str,
            t.id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="5">No API tokens created yet.</td></tr>"#);
    }

    let content = format!(
        r#"{}
        {}
        <div class="panel">
            <div class="panel-header">Create API Token</div>
            <div class="panel-body">
                <form method="POST" action="/api_token_create" style="display:flex; gap:8px; align-items:center;">
                    <input type="text" name="name" placeholder="Token Name (e.g. CI/CD, CLI)" required style="max-width:300px;">
                    <button type="submit" class="btn">Create API Token</button>
                </form>
            </div>
        </div>
        <div class="panel">
            <div class="panel-header">My API Tokens ({})</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Token Name</th>
                            <th>Created</th>
                            <th>Last Used</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>"#,
        err_banner,
        token_banner,
        tokens.len(),
        rows_html
    );

    render_layout("API Tokens", &ctx, q.msg.as_deref(), &content).into_response()
}

#[derive(Deserialize)]
pub struct ApiTokenCreateForm {
    pub name: String,
}

pub async fn api_token_create(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<ApiTokenCreateForm>,
) -> Response {
    match db.create_api_token(ctx.user.id, &form.name) {
        Ok((_id, raw_token)) => Redirect::to(&format!(
            "/api_tokens_page?msg=Token+created&token={}",
            raw_token
        ))
        .into_response(),
        Err(_) => Redirect::to("/api_tokens_page?err=Invalid+or+duplicate+token+name")
            .into_response(),
    }
}

pub async fn api_token_revoke(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let _ = db.revoke_api_token(id, ctx.user.id);
    Redirect::to("/api_tokens_page?msg=Token+revoked").into_response()
}

pub async fn manage_plugin_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required to manage plugins.");
    }

    let plugins = db.list_plugins().unwrap_or_default();
    let audit_count = db
        .get_config("plugin_event_audit_count", 0, 0)
        .ok()
        .flatten()
        .unwrap_or_else(|| "0".to_string());

    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    for p in &plugins {
        let status_badge = if p.enabled != 0 {
            r#"<span class="badge badge-success">Enabled</span>"#
        } else {
            r#"<span class="badge badge-warning">Disabled</span>"#
        };
        let prot_str = if p.protected != 0 { "Yes" } else { "No" };
        let action_html = if p.protected != 0 {
            "<em>Protected</em>".to_string()
        } else {
            let next_enabled = if p.enabled != 0 { "0" } else { "1" };
            let btn_label = if p.enabled != 0 { "Disable" } else { "Enable" };
            format!(
                r#"<form method="POST" action="/manage_plugin_toggle/{}" style="display:inline;">
                    <input type="hidden" name="enabled" value="{}">
                    <button type="submit" class="btn btn-secondary btn-sm">{}</button>
                </form>"#,
                html_escape(&p.basename),
                next_enabled,
                btn_label
            )
        };

        rows_html.push_str(&format!(
            r#"<tr>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            html_escape(&p.basename),
            status_badge,
            prot_str,
            p.priority,
            action_html
        ));
    }

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Installed Plugins ({}) — EventAuditLog Dispatched Events: {}</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Plugin Basename</th>
                            <th>Status</th>
                            <th>Protected</th>
                            <th>Priority</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>"#,
        err_banner,
        plugins.len(),
        html_escape(&audit_count),
        rows_html
    );

    render_layout("Manage Plugins", &ctx, q.msg.as_deref(), &content).into_response()
}

#[derive(Deserialize)]
pub struct PluginToggleForm {
    pub enabled: Option<String>,
}

pub async fn manage_plugin_toggle(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(basename): Path<String>,
    axum::Form(form): axum::Form<PluginToggleForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required.");
    }
    let enabled = parse_bool(form.enabled.as_deref());
    match db.set_plugin_enabled(&basename, enabled) {
        Ok(()) => Redirect::to("/manage_plugin_page?msg=Plugin+updated").into_response(),
        Err(_) => Redirect::to("/manage_plugin_page?err=Cannot+modify+protected+plugin")
            .into_response(),
    }
}

// ─── MantisBT-Compatible JSON REST API (`/api/rest/...`) ────────────────────

fn enum_obj(id: i32, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "label": name
    })
}

pub async fn rest_users_me(auth: ApiAuth) -> Response {
    let u = &auth.user;
    let role_name = AccessLevel::from_i32(u.access_level)
        .map(|a| a.as_str())
        .unwrap_or("viewer");
    Json(json!({
        "id": u.id,
        "name": u.username,
        "real_name": u.realname,
        "email": u.email,
        "enabled": u.enabled != 0,
        "protected": u.protected != 0,
        "access_level": enum_obj(u.access_level, role_name)
    }))
    .into_response()
}

pub async fn rest_list_projects(State(db): State<Db>, auth: ApiAuth) -> Response {
    let visible = db.list_visible_projects(&auth.user).unwrap_or_default();
    let mut projects_json = Vec::new();
    for (p, _depth) in visible {
        let cats: Vec<Value> = db
            .list_effective_categories(p.id)
            .unwrap_or_default()
            .into_iter()
            .map(|c| json!({ "id": c.id, "name": c.name, "project_id": c.project_id }))
            .collect();
        let versions: Vec<Value> = db
            .list_versions(p.id)
            .unwrap_or_default()
            .into_iter()
            .map(|v| {
                json!({
                    "id": v.id,
                    "name": v.version,
                    "description": v.description,
                    "released": v.released != 0,
                    "obsolete": v.obsolete != 0
                })
            })
            .collect();
        let vs_name = ViewState::from_i32(p.view_state)
            .map(|v| v.as_str())
            .unwrap_or("public");
        projects_json.push(json!({
            "id": p.id,
            "name": p.name,
            "status": p.status,
            "description": p.description,
            "enabled": p.enabled != 0,
            "view_state": enum_obj(p.view_state, vs_name),
            "categories": cats,
            "versions": versions
        }));
    }

    Json(json!({ "projects": projects_json })).into_response()
}

pub async fn rest_get_project(
    State(db): State<Db>,
    auth: ApiAuth,
    Path(id): Path<i64>,
) -> Response {
    let eff = db.effective_access_level(&auth.user, id).unwrap_or(0);
    if eff < 10 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": format!("Project {} not found", id) })),
        )
            .into_response();
    }
    let p = match db.get_project_by_id(id) {
        Ok(Some(proj)) => proj,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": format!("Project {} not found", id) })),
            )
                .into_response()
        }
    };
    let cats: Vec<Value> = db
        .list_effective_categories(p.id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| json!({ "id": c.id, "name": c.name, "project_id": c.project_id }))
        .collect();
    let versions: Vec<Value> = db
        .list_versions(p.id)
        .unwrap_or_default()
        .into_iter()
        .map(|v| {
            json!({
                "id": v.id,
                "name": v.version,
                "description": v.description,
                "released": v.released != 0,
                "obsolete": v.obsolete != 0
            })
        })
        .collect();
    let vs_name = ViewState::from_i32(p.view_state)
        .map(|v| v.as_str())
        .unwrap_or("public");

    Json(json!({
        "projects": [{
            "id": p.id,
            "name": p.name,
            "status": p.status,
            "description": p.description,
            "enabled": p.enabled != 0,
            "view_state": enum_obj(p.view_state, vs_name),
            "categories": cats,
            "versions": versions
        }]
    }))
    .into_response()
}

fn serialize_issue_json(db: &Db, bug_id: i64, user_access: i32) -> Option<Value> {
    let full = db.get_bug_full(bug_id).ok().flatten()?;
    let b = &full.bug;

    let proj_name = db
        .get_project_by_id(b.project_id)
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_default();
    let cat_name = db
        .get_category_by_id(b.category_id)
        .ok()
        .flatten()
        .map(|c| c.name)
        .unwrap_or_else(|| "General".to_string());
    let reporter_name = db
        .get_user_by_id(b.reporter_id)
        .ok()
        .flatten()
        .map(|u| u.username)
        .unwrap_or_default();
    let handler_val = if b.handler_id > 0 {
        let hname = db
            .get_user_by_id(b.handler_id)
            .ok()
            .flatten()
            .map(|u| u.username)
            .unwrap_or_default();
        json!({ "id": b.handler_id, "name": hname })
    } else {
        Value::Null
    };

    let status_str = BugStatus::from_i32(b.status)
        .map(|s| s.as_str())
        .unwrap_or("new");
    let res_str = BugResolution::from_i32(b.resolution)
        .map(|s| s.as_str())
        .unwrap_or("open");
    let pri_str = BugPriority::from_i32(b.priority)
        .map(|s| s.as_str())
        .unwrap_or("normal");
    let sev_str = BugSeverity::from_i32(b.severity)
        .map(|s| s.as_str())
        .unwrap_or("minor");
    let rep_str = BugReproducibility::from_i32(b.reproducibility)
        .map(|s| s.as_str())
        .unwrap_or("have not tried");
    let vs_str = ViewState::from_i32(b.view_state)
        .map(|s| s.as_str())
        .unwrap_or("public");

    let notes: Vec<Value> = db
        .list_bugnotes(b.id, user_access)
        .unwrap_or_default()
        .into_iter()
        .map(|n| {
            let n_vs = ViewState::from_i32(n.view_state)
                .map(|s| s.as_str())
                .unwrap_or("public");
            json!({
                "id": n.id,
                "reporter": { "id": n.reporter_id },
                "text": n.note,
                "view_state": enum_obj(n.view_state, n_vs),
                "time_tracking": n.time_tracking,
                "created_at": n.date_submitted
            })
        })
        .collect();

    let tags: Vec<Value> = db
        .list_bug_tags(b.id)
        .unwrap_or_default()
        .into_iter()
        .map(|t| json!({ "id": t.tag_id, "name": t.tag_name }))
        .collect();

    let custom_fields: Vec<Value> = db
        .get_custom_field_values_for_bug(b.id)
        .unwrap_or_default()
        .into_iter()
        .map(|cf| {
            json!({
                "field": { "id": cf.field_id, "name": cf.field_name },
                "value": cf.value
            })
        })
        .collect();

    let attachments: Vec<Value> = db
        .list_bug_files(b.id)
        .unwrap_or_default()
        .into_iter()
        .map(|f| {
            json!({
                "id": f.id,
                "filename": f.filename,
                "size": f.filesize,
                "content_type": f.file_type
            })
        })
        .collect();

    Some(json!({
        "id": b.id,
        "summary": b.summary,
        "description": full.description,
        "steps_to_reproduce": full.steps_to_reproduce,
        "additional_information": full.additional_information,
        "project": { "id": b.project_id, "name": proj_name },
        "category": { "id": b.category_id, "name": cat_name },
        "version": b.version,
        "fixed_in_version": b.fixed_in_version,
        "target_version": b.target_version,
        "reporter": { "id": b.reporter_id, "name": reporter_name },
        "handler": handler_val,
        "status": enum_obj(b.status, status_str),
        "resolution": enum_obj(b.resolution, res_str),
        "view_state": enum_obj(b.view_state, vs_str),
        "priority": enum_obj(b.priority, pri_str),
        "severity": enum_obj(b.severity, sev_str),
        "reproducibility": enum_obj(b.reproducibility, rep_str),
        "notes": notes,
        "tags": tags,
        "custom_fields": custom_fields,
        "attachments": attachments,
        "created_at": b.date_submitted,
        "updated_at": b.last_updated
    }))
}

#[derive(Deserialize, Default)]
pub struct IssuesListQuery {
    pub project_id: Option<i64>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

pub async fn rest_list_issues(
    State(db): State<Db>,
    auth: ApiAuth,
    Query(q): Query<IssuesListQuery>,
) -> Response {
    let filter = BugFilter {
        project_id: q.project_id,
        ..Default::default()
    };
    let bugs = db.filter_bugs(&filter, &auth.user).unwrap_or_default();
    let page_size = q.page_size.unwrap_or(50).clamp(1, 500);
    let page = q.page.unwrap_or(1).max(1);
    let start = (page - 1) * page_size;

    let mut issues_json = Vec::new();
    for b in bugs.into_iter().skip(start).take(page_size) {
        let eff = db
            .effective_access_level(&auth.user, b.project_id)
            .unwrap_or(0);
        if let Some(val) = serialize_issue_json(&db, b.id, eff) {
            issues_json.push(val);
        }
    }

    Json(json!({ "issues": issues_json })).into_response()
}

pub async fn rest_get_issue(
    State(db): State<Db>,
    auth: ApiAuth,
    Path(id): Path<i64>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": format!("Issue #{} not found", id) })),
            )
                .into_response()
        }
    };
    let eff = db
        .effective_access_level(&auth.user, bug.project_id)
        .unwrap_or(0);
    if eff < 10 || (bug.view_state == 50 && eff < 55) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Access denied to this issue" })),
        )
            .into_response();
    }

    match serialize_issue_json(&db, id, eff) {
        Some(issue) => Json(json!({ "issues": [issue] })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "Issue not found" })),
        )
            .into_response(),
    }
}

#[derive(Deserialize, Default, Serialize)]
pub struct IdNameRef {
    pub id: Option<i64>,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct RestCreateIssuePayload {
    pub summary: String,
    pub description: Option<String>,
    pub steps_to_reproduce: Option<String>,
    pub additional_information: Option<String>,
    pub project: IdNameRef,
    pub category: Option<IdNameRef>,
    pub priority: Option<IdNameRef>,
    pub severity: Option<IdNameRef>,
    pub reproducibility: Option<IdNameRef>,
    pub view_state: Option<IdNameRef>,
    pub version: Option<String>,
    pub target_version: Option<String>,
}

pub async fn rest_create_issue(
    State(db): State<Db>,
    auth: ApiAuth,
    Json(payload): Json<RestCreateIssuePayload>,
) -> Response {
    let project_id = payload.project.id.unwrap_or(0);
    if project_id <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "project.id is required" })),
        )
            .into_response();
    }

    let eff = db
        .effective_access_level(&auth.user, project_id)
        .unwrap_or(0);
    if eff < 25 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Reporter access required to create issues" })),
        )
            .into_response();
    }

    let category_id = payload
        .category
        .as_ref()
        .and_then(|c| c.id)
        .unwrap_or(1);
    let priority = payload
        .priority
        .as_ref()
        .and_then(|p| p.id)
        .unwrap_or(30) as i32;
    let severity = payload
        .severity
        .as_ref()
        .and_then(|s| s.id)
        .unwrap_or(50) as i32;
    let reproducibility = payload
        .reproducibility
        .as_ref()
        .and_then(|r| r.id)
        .unwrap_or(70) as i32;
    let view_state = payload
        .view_state
        .as_ref()
        .and_then(|v| v.id)
        .unwrap_or(10) as i32;

    match db.create_bug(
        project_id,
        auth.user.id,
        category_id,
        &payload.summary,
        payload.description.as_deref().unwrap_or(""),
        payload.steps_to_reproduce.as_deref().unwrap_or(""),
        payload.additional_information.as_deref().unwrap_or(""),
        priority,
        severity,
        reproducibility,
        payload.version.as_deref().unwrap_or(""),
        "",
        "",
        "",
        view_state,
    ) {
        Ok(bug_id) => {
            if let Some(ref tv) = payload.target_version {
                let _ = db.update_bug_fields(
                    bug_id,
                    auth.user.id,
                    BugUpdate {
                        target_version: Some(tv.clone()),
                        ..Default::default()
                    },
                );
            }
            let issue_json = serialize_issue_json(&db, bug_id, eff).unwrap_or(json!({ "id": bug_id }));
            (StatusCode::CREATED, Json(json!({ "issue": issue_json }))).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "message": e }))).into_response(),
    }
}

#[derive(Deserialize, Default)]
pub struct RestPatchIssuePayload {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub steps_to_reproduce: Option<String>,
    pub additional_information: Option<String>,
    pub status: Option<IdNameRef>,
    pub resolution: Option<IdNameRef>,
    pub handler: Option<IdNameRef>,
    pub priority: Option<IdNameRef>,
    pub severity: Option<IdNameRef>,
    pub target_version: Option<String>,
    pub fixed_in_version: Option<String>,
}

pub async fn rest_update_issue(
    State(db): State<Db>,
    auth: ApiAuth,
    Path(id): Path<i64>,
    Json(payload): Json<RestPatchIssuePayload>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": format!("Issue #{} not found", id) })),
            )
                .into_response()
        }
    };

    let eff = db
        .effective_access_level(&auth.user, bug.project_id)
        .unwrap_or(0);
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == auth.user.id);
    if !can_edit {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Insufficient access to update issue" })),
        )
            .into_response();
    }

    let update = BugUpdate {
        summary: payload.summary,
        description: payload.description,
        steps_to_reproduce: payload.steps_to_reproduce,
        additional_information: payload.additional_information,
        priority: payload.priority.and_then(|p| p.id).map(|v| v as i32),
        severity: payload.severity.and_then(|s| s.id).map(|v| v as i32),
        handler_id: payload.handler.and_then(|h| h.id),
        target_version: payload.target_version,
        fixed_in_version: payload.fixed_in_version,
        ..Default::default()
    };

    if let Err(e) = db.update_bug_fields(id, auth.user.id, update) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "message": e }))).into_response();
    }

    if let Some(st_ref) = payload.status {
        if let Some(new_st) = st_ref.id {
            let res_code = payload
                .resolution
                .and_then(|r| r.id)
                .map(|v| v as i32)
                .unwrap_or(bug.resolution);
            if let Err(e) = db.change_bug_status(id, auth.user.id, new_st as i32, res_code, None) {
                return (StatusCode::BAD_REQUEST, Json(json!({ "message": e }))).into_response();
            }
        }
    }

    let issue_json = serialize_issue_json(&db, id, eff).unwrap_or(json!({ "id": id }));
    Json(json!({ "issues": [issue_json] })).into_response()
}

pub async fn rest_delete_issue(
    State(db): State<Db>,
    auth: ApiAuth,
    Path(id): Path<i64>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": format!("Issue #{} not found", id) })),
            )
                .into_response()
        }
    };

    let eff = db
        .effective_access_level(&auth.user, bug.project_id)
        .unwrap_or(0);
    if eff < 70 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Manager access required to delete issues" })),
        )
            .into_response();
    }

    let _ = db.delete_bug(id);
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct RestAddNotePayload {
    pub text: String,
    pub view_state: Option<IdNameRef>,
    pub time_tracking: Option<i32>,
}

pub async fn rest_add_issue_note(
    State(db): State<Db>,
    auth: ApiAuth,
    Path(id): Path<i64>,
    Json(payload): Json<RestAddNotePayload>,
) -> Response {
    let bug = match db.get_bug(id) {
        Ok(Some(b)) => b,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": format!("Issue #{} not found", id) })),
            )
                .into_response()
        }
    };

    let eff = db
        .effective_access_level(&auth.user, bug.project_id)
        .unwrap_or(0);
    if eff < 25 || (bug.view_state == 50 && eff < 55) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Reporter access required to add bugnotes" })),
        )
            .into_response();
    }

    let vs = payload
        .view_state
        .and_then(|v| v.id)
        .unwrap_or(10) as i32;
    let mins = if eff >= 55 {
        payload.time_tracking.unwrap_or(0).max(0)
    } else {
        0
    };

    match db.add_bugnote_with_time(id, auth.user.id, &payload.text, vs, mins) {
        Ok(note_id) => {
            let note = db.get_bugnote_by_id(note_id).ok().flatten();
            (
                StatusCode::CREATED,
                Json(json!({
                    "note": {
                        "id": note_id,
                        "text": note.as_ref().map(|n| n.note.as_str()).unwrap_or(&payload.text),
                        "time_tracking": mins
                    }
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "message": e }))).into_response(),
    }
}
