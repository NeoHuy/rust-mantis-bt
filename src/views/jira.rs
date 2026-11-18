use crate::auth::AuthContext;
use crate::models::{JiraResolvedConfig, Project};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

pub fn render_manage_jira_page(
    ctx: &AuthContext,
    config: &JiraResolvedConfig,
    _projects: &[Project],
    project_mappings: &[(i64, String, String)],
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let banner = if let Some(m) = msg {
        format!(r#"<div class="flash-success">{}</div>"#, html_escape(m))
    } else if let Some(e) = err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let checked = if config.enabled { "checked" } else { "" };
    let is_bug_selected = if config.default_issue_type == "Bug" {
        "selected"
    } else {
        ""
    };
    let is_task_selected = if config.default_issue_type == "Task" {
        "selected"
    } else {
        ""
    };

    let mut project_mapping_rows = String::new();
    for (pid, pname, mapped_key) in project_mappings {
        project_mapping_rows.push_str(&format!(
            r#"<tr>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <input type="text" name="project_map_{}" value="{}" placeholder="e.g. PROJ" style="width:160px;">
                </td>
            </tr>"#,
            pid,
            html_escape(pname),
            pid,
            html_escape(mapped_key)
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
        if config.api_token.is_empty() {
            "Enter API token"
        } else {
            "••••••••"
        },
        token_hint,
        is_bug_selected,
        is_task_selected,
        html_escape(&config.project_key),
        project_mapping_rows
    );

    render_layout("Manage Jira", ctx, None, &body)
}

pub fn render_jira_section(
    _ctx: &AuthContext,
    bug_id: i64,
    jira_link: Option<&crate::models::JiraLink>,
    epics: &[crate::models::JiraEpic],
    config_enabled: bool,
    eff: i32,
) -> String {
    if !config_enabled {
        return String::new();
    }

    if let Some(jl) = jira_link {
        let status_badge = if !jl.jira_status.is_empty() {
            let lower = jl.jira_status.to_lowercase();
            let bg = if lower.contains("done")
                || lower.contains("resolved")
                || lower.contains("closed")
            {
                "#28a745"
            } else if lower.contains("in progress") || lower.contains("dev") {
                "#0052cc"
            } else if lower.contains("review") || lower.contains("testing") || lower.contains("qa")
            {
                "#ffab00"
            } else {
                "#6c757d"
            };
            format!(
                r#"<span class="badge" style="background:{}; color:#fff; font-size:11px; padding:2px 6px; border-radius:2px;">{}</span>"#,
                bg,
                html_escape(&jl.jira_status)
            )
        } else {
            "-".to_string()
        };

        let unlink_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_unlink" style="display:inline; margin-left:4px;" onsubmit="return confirm('Are you sure you want to unlink this Jira ticket?');">
                    <button type="submit" class="btn btn-sm btn-danger">Unlink</button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        let sync_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_sync" style="display:inline;">
                    <button type="submit" class="btn btn-sm">Sync with Jira</button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        format!(
            r#"<div class="panel">
                <div class="panel-header">Jira Integration</div>
                <div class="panel-body">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th style="width:140px;">Jira Ticket</th>
                                <th style="width:100px;">Type</th>
                                <th style="width:120px;">Jira Status</th>
                                <th>Jira URL</th>
                                <th style="width:180px;">Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td>
                                    <a href="{}" target="_blank" rel="noopener noreferrer" style="font-weight:bold; color:#0052cc;">
                                        {} <i class="fa fa-external-link" style="font-size:11px;"></i>
                                    </a>
                                </td>
                                <td>{}</td>
                                <td>{}</td>
                                <td>
                                    <a href="{}" target="_blank" rel="noopener noreferrer" style="color:#555;">{}</a>
                                </td>
                                <td>
                                    {}{}{}
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>"#,
            html_escape(&jl.jira_url),
            html_escape(&jl.jira_key),
            html_escape(&jl.issue_type),
            status_badge,
            html_escape(&jl.jira_url),
            html_escape(&jl.jira_url),
            sync_btn,
            unlink_btn,
            if eff < 55 { "—" } else { "" }
        )
    } else if eff >= 55 {
        let mut epic_options = String::new();
        for ep in epics {
            epic_options.push_str(&format!(
                r#"<option value="{}">{} - {}</option>"#,
                html_escape(&ep.key),
                html_escape(&ep.key),
                html_escape(&ep.summary)
            ));
        }

        format!(
            r#"<div class="panel">
                <div class="panel-header">Jira Integration</div>
                <div class="panel-body">
                    <p style="margin-bottom:12px; color:#666;">No Jira ticket linked to this issue.</p>
                    <form method="POST" action="/view_bug_page/{}/jira_sync">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Jira Ticket Number</td>
                                <td class="input-col">
                                    <input type="text" name="jira_key" placeholder="e.g. 123 or JBC-123" style="width:260px;">
                                    <small style="color:#777; display:block; margin-top:2px;">Leave empty to create a new Jira ticket</small>
                                </td>
                            </tr>
                            <tr>
                                <td class="label-col">Epic</td>
                                <td class="input-col">
                                    <select name="epic_key" style="min-width:240px; width:auto;">
                                        <option value="">None (No Epic)</option>
                                        {}
                                    </select>
                                    <small style="color:#777; display:block; margin-top:2px;">Optional parent epic in Jira</small>
                                </td>
                            </tr>
                            <tr>
                                <td class="label-col">Issue Type</td>
                                <td class="input-col">
                                    <select name="issue_type" style="width:auto;">
                                        <option value="Bug">Bug</option>
                                        <option value="Task">Task</option>
                                        <option value="Story">Story</option>
                                    </select>
                                    <small style="color:#777; display:block; margin-top:2px;">Type if creating a new Jira ticket</small>
                                </td>
                            </tr>
                        </table>
                        <button type="submit" class="btn btn-sm">Sync</button>
                    </form>
                </div>
            </div>"#,
            bug_id, epic_options
        )
    } else {
        r#"<div class="panel">
            <div class="panel-header">Jira Integration</div>
            <div class="panel-body">
                <p style="color:#666;">No Jira ticket linked to this issue.</p>
            </div>
        </div>"#
            .to_string()
    }
}

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
