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

    views::jira::render_manage_jira_page(
        &ctx,
        &config,
        &projects,
        &project_mappings,
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
        return forbidden_html(
            &ctx,
            "Developer access required on this project to push issues to Jira.",
        );
    }

    // Check if already linked
    if let Ok(Some(existing)) = db.get_jira_link_by_bug_id(bug_id) {
        return Redirect::to(&format!(
            "/view_bug_page/{}?err=Issue+is+already+linked+to+Jira+({})",
            bug_id, existing.jira_key
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
    let jira_resp = match client
        .create_issue(&config, summary, &description, issue_type)
        .await
    {
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
        bug_id, jira_resp.key
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

pub fn router() -> Router<Db> {
    Router::new()
        .route("/manage_jira_page", get(manage_jira_page))
        .route("/manage_jira_config", post(manage_jira_config))
        .route("/manage_jira_test", post(manage_jira_test))
        .route("/view_bug_page/:id/jira_push", post(push_bug_to_jira))
}
