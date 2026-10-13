use crate::auth::AuthContext;
use crate::db::Db;
use crate::jira::JiraClient;
use crate::ui::{html_escape, render_layout};
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
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

// ─── Manage Jira Page & Config ───────────────────────────────────────────────

pub async fn manage_jira_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required to manage Jira integration.");
    }

    let config = db.get_jira_config(0).unwrap_or_else(|_| crate::models::JiraResolvedConfig {
        enabled: false,
        base_url: String::new(),
        email: String::new(),
        api_token: String::new(),
        default_issue_type: "Bug".to_string(),
        project_key: String::new(),
    });

    let projects = db.list_all_projects().unwrap_or_default();

    let banner = if let Some(m) = &q.msg {
        format!(r#"<div class="flash-success">{}</div>"#, html_escape(m))
    } else if let Some(e) = &q.err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let checked = if config.enabled { "checked" } else { "" };
    let is_bug_selected = if config.default_issue_type == "Bug" { "selected" } else { "" };
    let is_task_selected = if config.default_issue_type == "Task" { "selected" } else { "" };

    let mut project_mapping_rows = String::new();
    for p in &projects {
        let mapped_key = db
            .get_config("jira_project_key", p.id, 0)
            .ok()
            .flatten()
            .unwrap_or_default();
        project_mapping_rows.push_str(&format!(
            r#"<tr>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <input type="text" name="project_map_{}" value="{}" placeholder="e.g. PROJ" style="width:160px;">
                </td>
            </tr>"#,
            p.id,
            html_escape(&p.name),
            p.id,
            html_escape(&mapped_key)
        ));
    }

    let token_hint = if !config.api_token.is_empty() {
        r#"<small style="color:#777; display:block; margin-top:2px;">(API Token currently saved. Leave blank to keep existing token)</small>"#
    } else {
        ""
    };

    let body = format!(
        r#"{}
        <div class="widget-box" style="margin-bottom:20px;">
            <div class="widget-header">
                <h2>Jira Cloud Integration Configuration</h2>
            </div>
            <div class="widget-body" style="padding:16px;">
                <form method="POST" action="/manage_jira_config">
                    <table class="table" style="max-width:800px;">
                        <tr>
                            <th style="width:220px;">Enable Jira Integration</th>
                            <td>
                                <label style="display:flex; align-items:center; gap:8px;">
                                    <input type="checkbox" name="jira_enabled" value="1" {}>
                                    <span>Enable pushing Mantis issues to Atlassian Jira Cloud</span>
                                </label>
                            </td>
                        </tr>
                        <tr>
                            <th>Jira Base URL</th>
                            <td>
                                <input type="url" name="jira_base_url" value="{}" placeholder="https://yourcompany.atlassian.net" style="width:100%; max-width:400px;" required>
                                <small style="color:#777; display:block; margin-top:2px;">Atlassian Jira Cloud URL (e.g. https://domain.atlassian.net)</small>
                            </td>
                        </tr>
                        <tr>
                            <th>Jira Account Email</th>
                            <td>
                                <input type="email" name="jira_email" value="{}" placeholder="bot@yourcompany.com" style="width:100%; max-width:400px;" required>
                                <small style="color:#777; display:block; margin-top:2px;">Atlassian account email address with issue creation permission</small>
                            </td>
                        </tr>
                        <tr>
                            <th>Jira API Token</th>
                            <td>
                                <input type="password" name="jira_api_token" placeholder="{}" style="width:100%; max-width:400px;">
                                {}
                            </td>
                        </tr>
                        <tr>
                            <th>Default Issue Type</th>
                            <td>
                                <select name="jira_default_issue_type" style="width:160px;">
                                    <option value="Bug" {}>Bug</option>
                                    <option value="Task" {}>Task</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <th>Default Jira Project Key</th>
                            <td>
                                <input type="text" name="jira_default_project_key" value="{}" placeholder="e.g. MANTIS" style="width:160px;">
                                <small style="color:#777; display:block; margin-top:2px;">Fallback key if project is not mapped below</small>
                            </td>
                        </tr>
                    </table>

                    <h3 style="margin-top:24px; margin-bottom:12px; font-size:15px;">Project Mapping (Mantis Project &rarr; Jira Project Key)</h3>
                    <table class="table" style="max-width:800px; margin-bottom:20px;">
                        <thead>
                            <tr>
                                <th style="width:60px;">ID</th>
                                <th>Mantis Project Name</th>
                                <th style="width:200px;">Jira Project Key</th>
                            </tr>
                        </thead>
                        <tbody>
                            {}
                        </tbody>
                    </table>

                    <div style="display:flex; gap:10px; align-items:center;">
                        <button type="submit" class="btn btn-primary">Save Jira Configuration</button>
                    </div>
                </form>

                <hr style="margin:24px 0;">

                <form method="POST" action="/manage_jira_test" style="display:flex; align-items:center; gap:12px;">
                    <button type="submit" class="btn btn-secondary">Test Connection to Jira</button>
                    <small style="color:#666;">Verifies current credentials against Atlassian Jira /rest/api/3/myself</small>
                </form>
            </div>
        </div>"#,
        banner,
        checked,
        html_escape(&config.base_url),
        html_escape(&config.email),
        if config.api_token.is_empty() { "Enter API token" } else { "••••••••" },
        token_hint,
        is_bug_selected,
        is_task_selected,
        html_escape(&config.project_key),
        project_mapping_rows
    );

    render_layout("Manage Jira", &ctx, None, &body).into_response()
}

pub async fn manage_jira_config(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required to update Jira configuration.");
    }

    let enabled = form.get("jira_enabled").map(|v| v == "1" || v == "on").unwrap_or(false);
    let base_url = form.get("jira_base_url").map(|s| s.trim()).unwrap_or("");
    let email = form.get("jira_email").map(|s| s.trim()).unwrap_or("");
    let raw_token = form.get("jira_api_token").map(|s| s.trim()).unwrap_or("");
    let default_issue_type = form.get("jira_default_issue_type").map(|s| s.trim()).unwrap_or("Bug");
    let default_project_key = form.get("jira_default_project_key").map(|s| s.trim()).unwrap_or("");

    let token_opt = if !raw_token.is_empty() {
        Some(raw_token)
    } else {
        None
    };

    if let Err(e) = db.save_jira_global_config(
        enabled,
        base_url,
        email,
        token_opt,
        default_issue_type,
        default_project_key,
    ) {
        return Redirect::to(&format!("/manage_jira_page?err=Failed+to+save+config:+{}", url_encode(&sanitize_error_message(&e.to_string())))).into_response();
    }

    // Process project mappings
    for (k, v) in &form {
        if let Some(rest) = k.strip_prefix("project_map_") {
            if let Ok(pid) = rest.parse::<i64>() {
                let _ = db.set_jira_project_mapping(pid, v.trim());
            }
        }
    }

    Redirect::to("/manage_jira_page?msg=Jira+configuration+saved+successfully").into_response()
}

pub async fn manage_jira_test(
    State(db): State<Db>,
    ctx: AuthContext,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required.");
    }

    let config = match db.get_jira_config(0) {
        Ok(c) => c,
        Err(e) => return Redirect::to(&format!("/manage_jira_page?err=Config+error:+{}", url_encode(&sanitize_error_message(&e.to_string())))).into_response(),
    };

    let client = JiraClient::new();
    match client.test_connection(&config.base_url, &config.email, &config.api_token).await {
        Ok(user) => {
            let msg = format!("Connection successful! Logged in as: {}", user.display_name);
            Redirect::to(&format!("/manage_jira_page?msg={}", url_encode(&sanitize_error_message(&msg)))).into_response()
        }
        Err(e) => {
            let err = format!("Connection failed: {}", e);
            Redirect::to(&format!("/manage_jira_page?err={}", url_encode(&sanitize_error_message(&err)))).into_response()
        }
    }
}

pub async fn push_bug_to_jira(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    form: Result<Form<HashMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let form_data = form.map(|Form(f)| f).unwrap_or_default();

    let bug_full = match db.get_bug_full(bug_id) {
        Ok(Some(bf)) => bf,
        _ => return Redirect::to("/view_all_bug_page?err=Issue+not+found").into_response(),
    };
    let bug = bug_full.bug;

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 || (bug.view_state == 50 && eff < 55) {
        return forbidden_html(&ctx, "Developer access required on this project to push issues to Jira.");
    }

    // Check if already linked
    if let Ok(Some(existing)) = db.get_jira_link_by_bug_id(bug_id) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err=Issue+is+already+linked+to+Jira+({})",
            bug_id,
            existing.jira_key
        ))
        .into_response();
    }

    // Resolve Jira config for this bug's project
    let config = match db.get_jira_config(bug.project_id) {
        Ok(c) => c,
        Err(e) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Config+error:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response()
        }
    };

    if !config.enabled {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err=Jira+integration+is+currently+disabled",
            bug_id
        ))
        .into_response();
    }

    let summary = form_data
        .get("summary")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&bug.summary);

    let issue_type = form_data
        .get("issue_type")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&config.default_issue_type);

    let description = format!(
        "{}\n\n---\nReported in MantisBT: Issue #{:07}\nSummary: {}\nReporter ID: {}\nPriority: {}",
        bug_full.description, bug.id, bug.summary, bug.reporter_id, bug.priority
    );

    let client = JiraClient::new();
    let jira_resp = match client.create_issue(&config, summary, &description, issue_type).await {
        Ok(resp) => resp,
        Err(e) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Jira+creation+failed:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response();
        }
    };

    // Save link, bugnote, and status update atomically in DB transaction
    if let Err(e) = db.record_jira_push_success(
        bug_id,
        &jira_resp.key,
        &jira_resp.browse_url,
        issue_type,
        ctx.user.id,
        &ctx.user.username,
    ) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err=Failed+to+save+Jira+link:+{}",
            bug_id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response();
    }

    Redirect::to(&format!(
        "/view_bug_page/{}?msg=Created+Jira+issue+{}",
        bug_id,
        jira_resp.key
    ))
    .into_response()
}

fn sanitize_error_message(s: &str) -> String {
    let single_line = s.replace(['\r', '\n', '\t'], " ");
    let trimmed = single_line.trim();
    if trimmed.chars().count() > 120 {
        format!("{}...", trimmed.chars().take(117).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
