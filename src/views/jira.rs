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

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
