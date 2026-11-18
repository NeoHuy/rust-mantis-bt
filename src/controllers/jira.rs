use crate::auth::AuthContext;
use crate::db::Db;
use crate::jira::JiraClient;
use crate::views;
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
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
        views::html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        views::layout::render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

pub async fn manage_jira_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(
            &ctx,
            "Administrator access required to manage Jira integration.",
        );
    }

    let config = db
        .get_jira_config(0)
        .unwrap_or_else(|_| crate::models::JiraResolvedConfig {
            enabled: false,
            base_url: String::new(),
            email: String::new(),
            api_token: String::new(),
            default_issue_type: "Bug".to_string(),
            project_key: String::new(),
        });

    let projects = db.list_all_projects().unwrap_or_default();

    let mut project_mappings = Vec::new();
    for p in &projects {
        let mapped_key = db
            .get_config("jira_project_key", p.id, 0)
            .ok()
            .flatten()
            .unwrap_or_default();
        project_mappings.push((p.id, p.name.clone(), mapped_key));
    }

    let cached_epics = db.list_cached_jira_epics_summary().unwrap_or_default();
    let status_mappings = db.list_jira_status_mappings().unwrap_or_default();

    views::jira::render_manage_jira_page(
        &ctx,
        &config,
        &projects,
        &project_mappings,
        &cached_epics,
        &status_mappings,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn manage_jira_config(
    State(db): State<Db>,
    ctx: AuthContext,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(
            &ctx,
            "Administrator access required to update Jira configuration.",
        );
    }

    let enabled = form
        .get("jira_enabled")
        .map(|v| v == "1" || v == "on")
        .unwrap_or(false);
    let base_url = form.get("jira_base_url").map(|s| s.trim()).unwrap_or("");
    let email = form.get("jira_email").map(|s| s.trim()).unwrap_or("");
    let raw_token = form.get("jira_api_token").map(|s| s.trim()).unwrap_or("");
    let default_issue_type = form
        .get("jira_default_issue_type")
        .map(|s| s.trim())
        .unwrap_or("Bug");
    let default_project_key = form
        .get("jira_default_project_key")
        .map(|s| s.trim())
        .unwrap_or("");

    let token_opt = if !raw_token.is_empty() {
        Some(raw_token)
    } else {
        None
    };

    if enabled || !base_url.is_empty() {
        if let Err(e) = crate::jira::validate_jira_url(base_url) {
            return Redirect::to(&format!(
                "/manage_jira_page?err=Failed+to+save+config:+{}",
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response();
        }
    }

    if let Err(e) = db.save_jira_global_config(
        enabled,
        base_url,
        email,
        token_opt,
        default_issue_type,
        default_project_key,
    ) {
        return Redirect::to(&format!(
            "/manage_jira_page?err=Failed+to+save+config:+{}",
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response();
    }

    // Process project mappings
    for (k, v) in &form {
        if let Some(rest) = k.strip_prefix("project_map_") {
            if let Ok(pid) = rest.parse::<i64>() {
                let _ = db.set_jira_project_mapping(pid, v.trim());
            }
        }
    }

    // Process status mappings
    let mut status_indices = std::collections::BTreeSet::new();
    for k in form.keys() {
        if let Some(rest) = k.strip_prefix("status_jira_") {
            if let Ok(idx) = rest.parse::<usize>() {
                status_indices.insert(idx);
            }
        }
    }

    if !status_indices.is_empty() {
        let mut new_mappings = Vec::new();
        for idx in status_indices {
            let j_key = format!("status_jira_{}", idx);
            let m_key = format!("status_mantis_{}", idx);
            if let Some(j_val) = form.get(&j_key).map(|s| s.trim()) {
                if !j_val.is_empty() {
                    let m_val = form
                        .get(&m_key)
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .unwrap_or(10);
                    new_mappings.push((j_val.to_string(), m_val));
                }
            }
        }
        if let Err(e) = db.save_jira_status_mappings(&new_mappings) {
            return Redirect::to(&format!(
                "/manage_jira_page?err=Failed+to+save+status+mappings:+{}",
                url_encode(&e.to_string())
            ))
            .into_response();
        }
    }

    Redirect::to("/manage_jira_page?msg=Jira+configuration+saved+successfully").into_response()
}

pub async fn manage_jira_test(State(db): State<Db>, ctx: AuthContext) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required.");
    }

    let config = match db.get_jira_config(0) {
        Ok(c) => c,
        Err(e) => {
            return Redirect::to(&format!(
                "/manage_jira_page?err=Config+error:+{}",
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response()
        }
    };

    let client = JiraClient::new();
    match client
        .test_connection(&config.base_url, &config.email, &config.api_token)
        .await
    {
        Ok(user) => {
            let msg = format!("Connection successful! Logged in as: {}", user.display_name);
            Redirect::to(&format!(
                "/manage_jira_page?msg={}",
                url_encode(&sanitize_error_message(&msg))
            ))
            .into_response()
        }
        Err(e) => {
            let err = format!("Connection failed: {}", e);
            Redirect::to(&format!(
                "/manage_jira_page?err={}",
                url_encode(&sanitize_error_message(&err))
            ))
            .into_response()
        }
    }
}

pub async fn manage_jira_refresh_epics(State(db): State<Db>, ctx: AuthContext) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden_html(&ctx, "Administrator access required to refresh Jira epics.");
    }

    let config = db
        .get_jira_config(0)
        .unwrap_or_else(|_| crate::models::JiraResolvedConfig {
            enabled: false,
            base_url: String::new(),
            email: String::new(),
            api_token: String::new(),
            default_issue_type: "Bug".to_string(),
            project_key: String::new(),
        });

    if !config.enabled {
        return Redirect::to("/manage_jira_page?err=Jira+integration+is+currently+disabled")
            .into_response();
    }

    let mut project_keys = std::collections::BTreeSet::new();
    let def_key = config.project_key.trim();
    if !def_key.is_empty() {
        project_keys.insert(def_key.to_string());
    }

    let projects = db.list_all_projects().unwrap_or_default();
    for p in &projects {
        if let Ok(Some(pk)) = db.get_config("jira_project_key", p.id, 0) {
            let trimmed = pk.trim();
            if !trimmed.is_empty() {
                project_keys.insert(trimmed.to_string());
            }
        }
    }

    if project_keys.is_empty() {
        return Redirect::to("/manage_jira_page?err=No+Jira+project+keys+configured+to+refresh")
            .into_response();
    }

    let client = JiraClient::new();
    let mut total_epics = 0;
    let mut errors = Vec::new();

    for pk in &project_keys {
        let mut key_conf = config.clone();
        key_conf.project_key = pk.clone();
        match client.list_epics(&key_conf).await {
            Ok(epics) => {
                total_epics += epics.len();
                let _ = db.save_jira_epics_cache(pk, &epics);
            }
            Err(e) => {
                errors.push(format!("{}: {}", pk, e));
            }
        }
    }

    if !errors.is_empty() && total_epics == 0 {
        let err_msg = url_encode(&format!("Failed to refresh epics: {}", errors.join(", ")));
        Redirect::to(&format!("/manage_jira_page?err={}", err_msg)).into_response()
    } else {
        let msg = url_encode(&format!(
            "Epics cache refreshed successfully: {} epics cached across {} Jira project(s).",
            total_epics,
            project_keys.len()
        ));
        Redirect::to(&format!("/manage_jira_page?msg={}", msg)).into_response()
    }
}

#[derive(Deserialize, Default)]
pub struct JiraSyncForm {
    pub jira_key: Option<String>,
    pub epic_key: Option<String>,
    pub issue_type: Option<String>,
}

pub async fn sync_bug_with_jira(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    form: Result<Form<JiraSyncForm>, axum::extract::rejection::FormRejection>,
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
        return forbidden_html(
            &ctx,
            "Developer access required on this project to sync issues with Jira.",
        );
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

    let client = JiraClient::new();
    let existing_link = match db.get_jira_link_by_bug_id(bug_id) {
        Ok(l) => l,
        Err(e) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Database+error:+{}",
                bug_id,
                url_encode(&e.to_string())
            ))
            .into_response();
        }
    };

    if let Some(existing) = existing_link {
        // Already linked -> Refresh status from Jira
        let details = match client.get_issue(&config, &existing.jira_key).await {
            Ok(d) => d,
            Err(e) => {
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err=Jira+sync+failed:+{}",
                    bug_id,
                    url_encode(&sanitize_error_message(&e.to_string()))
                ))
                .into_response();
            }
        };

        let updated_jira_url = if details.key != existing.jira_key && !config.base_url.is_empty() {
            format!(
                "{}/browse/{}",
                config.base_url.trim().trim_end_matches('/'),
                details.key
            )
        } else {
            existing.jira_url
        };

        if let Err(e) = db.record_jira_sync_success(
            bug_id,
            &details.key,
            &updated_jira_url,
            &details.issue_type,
            &details.status,
            ctx.user.id,
            &ctx.user.username,
            false,
        ) {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Failed+to+save+Jira+sync:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e))
            ))
            .into_response();
        }

        return Redirect::to(&format!(
            "/view_bug_page/{}?msg=Synced+with+Jira+issue+{}",
            bug_id,
            url_encode(&details.key)
        ))
        .into_response();
    }

    // Not linked yet
    let raw_key = form_data.jira_key.as_deref().unwrap_or("").trim();
    if !raw_key.is_empty() {
        let normalized_key = crate::jira::normalize_jira_key(raw_key, &config.project_key);
        if normalized_key.is_empty() {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Invalid+Jira+issue+key+format:+{}",
                bug_id,
                url_encode(raw_key)
            ))
            .into_response();
        }

        // Claim pending reservation to prevent concurrent creation or linking
        match db.claim_jira_link_pending(bug_id, ctx.user.id) {
            Ok(true) => {}
            Ok(false) => {
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err=A+Jira+operation+is+already+in+progress+for+this+issue",
                    bug_id
                ))
                .into_response();
            }
            Err(e) => {
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err=Database+error:+{}",
                    bug_id,
                    url_encode(&sanitize_error_message(&e.to_string()))
                ))
                .into_response();
            }
        }

        // Link with existing ticket
        let details = match client.get_issue(&config, &normalized_key).await {
            Ok(d) => d,
            Err(e) => {
                let _ = db.release_jira_link_pending(bug_id);
                return Redirect::to(&format!(
                    "/view_bug_page/{}?err=Jira+link+failed:+{}",
                    bug_id,
                    url_encode(&sanitize_error_message(&e.to_string()))
                ))
                .into_response();
            }
        };

        let trimmed_url = config.base_url.trim().trim_end_matches('/');
        let browse_url = format!("{}/browse/{}", trimmed_url, details.key);

        if let Err(e) = db.record_jira_sync_success(
            bug_id,
            &details.key,
            &browse_url,
            &details.issue_type,
            &details.status,
            ctx.user.id,
            &ctx.user.username,
            true,
        ) {
            let _ = db.release_jira_link_pending(bug_id);
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Failed+to+save+Jira+link:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e))
            ))
            .into_response();
        }

        return Redirect::to(&format!(
            "/view_bug_page/{}?msg=Linked+to+Jira+issue+{}",
            bug_id,
            url_encode(&details.key)
        ))
        .into_response();
    }

    // Create new ticket in Jira
    // Claim pending reservation to prevent concurrent creation (double-click)
    match db.claim_jira_link_pending(bug_id, ctx.user.id) {
        Ok(true) => {}
        Ok(false) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=A+Jira+operation+is+already+in+progress+for+this+issue",
                bug_id
            ))
            .into_response();
        }
        Err(e) => {
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Database+error:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response();
        }
    }

    let issue_type = form_data
        .issue_type
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&config.default_issue_type);

    let epic_key = form_data
        .epic_key
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let description = format!(
        "{}\n\n---\nReported in MantisBT: Issue #{:07}\nSummary: {}\nReporter ID: {}\nPriority: {}",
        bug_full.description, bug.id, bug.summary, bug.reporter_id, bug.priority
    );

    let jira_resp = match client
        .create_issue(&config, &bug.summary, &description, issue_type, epic_key)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            let _ = db.release_jira_link_pending(bug_id);
            return Redirect::to(&format!(
                "/view_bug_page/{}?err=Jira+creation+failed:+{}",
                bug_id,
                url_encode(&sanitize_error_message(&e.to_string()))
            ))
            .into_response();
        }
    };

    let jira_status = match client.get_issue(&config, &jira_resp.key).await {
        Ok(d) => d.status,
        Err(_) => String::new(),
    };

    if let Err(e) = db.record_jira_sync_success(
        bug_id,
        &jira_resp.key,
        &jira_resp.browse_url,
        issue_type,
        &jira_status,
        ctx.user.id,
        &ctx.user.username,
        true,
    ) {
        let _ = db.release_jira_link_pending(bug_id);
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
        url_encode(&jira_resp.key)
    ))
    .into_response()
}

pub async fn unlink_bug_from_jira(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
) -> Response {
    let bug_full = match db.get_bug_full(bug_id) {
        Ok(Some(bf)) => bf,
        _ => return Redirect::to("/view_all_bug_page?err=Issue+not+found").into_response(),
    };
    let bug = bug_full.bug;

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 55 || (bug.view_state == 50 && eff < 55) {
        return forbidden_html(
            &ctx,
            "Developer access required on this project to unlink issues from Jira.",
        );
    }

    match db.delete_jira_link_with_note(bug_id, ctx.user.id, &ctx.user.username) {
        Ok(true) => Redirect::to(&format!(
            "/view_bug_page/{}?msg=Jira+issue+unlinked+successfully",
            bug_id
        ))
        .into_response(),
        Ok(false) => Redirect::to(&format!(
            "/view_bug_page/{}?err=No+Jira+link+found+to+unlink",
            bug_id
        ))
        .into_response(),
        Err(e) => Redirect::to(&format!(
            "/view_bug_page/{}?err=Failed+to+unlink+Jira+issue:+{}",
            bug_id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response(),
    }
}

pub async fn push_bug_to_jira(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    form: Result<Form<HashMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let form_data = form.map(|Form(f)| f).unwrap_or_default();
    let sync_form = JiraSyncForm {
        jira_key: None,
        epic_key: None,
        issue_type: form_data.get("issue_type").cloned(),
    };
    sync_bug_with_jira(State(db), ctx, Path(bug_id), Ok(Form(sync_form))).await
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

pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_jira_page", get(manage_jira_page))
        .route("/manage_jira_config", post(manage_jira_config))
        .route("/manage_jira_test", post(manage_jira_test))
        .route(
            "/manage_jira_refresh_epics",
            post(manage_jira_refresh_epics),
        )
        .route("/view_bug_page/:id/jira_sync", post(sync_bug_with_jira))
        .route("/view_bug_page/:id/jira_unlink", post(unlink_bug_from_jira))
        .route("/view_bug_page/:id/jira_push", post(push_bug_to_jira))
}

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
