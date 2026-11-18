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
                r#"<span class="badge" style="background:{}; color:#fff; font-size:12px; padding:3px 8px; border-radius:3px;">{}</span>"#,
                bg,
                html_escape(&jl.jira_status)
            )
        } else {
            String::new()
        };

        let unlink_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_unlink" style="display:inline;" onsubmit="return confirm('Are you sure you want to unlink this Jira ticket?');">
                    <button type="submit" class="btn btn-danger btn-sm" style="padding:4px 10px;">
                        <i class="fa fa-chain-broken"></i> Unlink
                    </button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        let sync_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_sync" style="display:inline;">
                    <button type="submit" class="btn btn-primary btn-sm" style="background:#0052cc; color:#fff; border-color:#0747a6; padding:4px 10px;">
                        <i class="fa fa-refresh"></i> Sync with Jira
                    </button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        format!(
            r#"<div class="panel">
                <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                    <span><i class="fa fa-ticket"></i> Jira Integration</span>
                    {}
                </div>
                <div class="panel-body">
                    <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:12px;">
                        <div style="display:flex; align-items:center; gap:16px;">
                            <div>
                                <span style="color:#777; font-size:11px; display:block;">TICKET</span>
                                <a href="{}" target="_blank" rel="noopener noreferrer" style="font-weight:bold; font-size:14px; color:#0052cc; text-decoration:none;">
                                    {} <i class="fa fa-external-link" style="font-size:11px;"></i>
                                </a>
                            </div>
                            <div style="border-left:1px solid #ddd; height:24px;"></div>
                            <div>
                                <span style="color:#777; font-size:11px; display:block;">TYPE</span>
                                <span style="font-weight:600; font-size:13px;">{}</span>
                            </div>
                            <div style="border-left:1px solid #ddd; height:24px;"></div>
                            <div>
                                <span style="color:#777; font-size:11px; display:block;">JIRA STATUS</span>
                                <span style="font-weight:600; font-size:13px;">{}</span>
                            </div>
                        </div>
                        <div style="display:flex; gap:8px; align-items:center;">
                            {}
                            {}
                        </div>
                    </div>
                </div>
            </div>"#,
            status_badge,
            html_escape(&jl.jira_url),
            html_escape(&jl.jira_key),
            html_escape(&jl.issue_type),
            if jl.jira_status.is_empty() {
                "Unknown"
            } else {
                &jl.jira_status
            },
            sync_btn,
            unlink_btn
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
                <div class="panel-header">
                    <span><i class="fa fa-ticket"></i> Jira Integration</span>
                </div>
                <div class="panel-body">
                    <form method="POST" action="/view_bug_page/{}/jira_sync">
                        <div style="display:flex; flex-wrap:wrap; gap:14px; align-items:flex-end;">
                            <div>
                                <label style="display:block; font-size:12px; font-weight:bold; margin-bottom:4px; color:#333;">Jira Ticket Number</label>
                                <input type="text" name="jira_key" placeholder="e.g. 123 or JBC-123 (empty to create new)" style="width:260px; padding:6px 8px; border:1px solid #ccc; border-radius:3px;">
                                <small style="color:#777; display:block; font-size:11px; margin-top:2px;">Leave empty to create a new Jira ticket</small>
                            </div>
                            <div>
                                <label style="display:block; font-size:12px; font-weight:bold; margin-bottom:4px; color:#333;">Epic</label>
                                <select name="epic_key" style="min-width:200px; padding:6px 8px; border:1px solid #ccc; border-radius:3px;">
                                    <option value="">None (No Epic)</option>
                                    {}
                                </select>
                                <small style="color:#777; display:block; font-size:11px; margin-top:2px;">Optional parent epic in Jira</small>
                            </div>
                            <div>
                                <label style="display:block; font-size:12px; font-weight:bold; margin-bottom:4px; color:#333;">Issue Type</label>
                                <select name="issue_type" style="width:130px; padding:6px 8px; border:1px solid #ccc; border-radius:3px;">
                                    <option value="Bug">Bug</option>
                                    <option value="Task">Task</option>
                                    <option value="Story">Story</option>
                                </select>
                                <small style="color:#777; display:block; font-size:11px; margin-top:2px;">Type if creating new</small>
                            </div>
                            <div>
                                <button type="submit" class="btn btn-primary btn-sm" style="background:#0052cc; color:#fff; border-color:#0747a6; padding:7px 18px; font-weight:bold; border-radius:3px; cursor:pointer;">
                                    <i class="fa fa-refresh"></i> Sync
                                </button>
                            </div>
                        </div>
                    </form>
                </div>
            </div>"#,
            bug_id, epic_options
        )
    } else {
        r#"<div class="panel">
            <div class="panel-header">
                <span><i class="fa fa-ticket"></i> Jira Integration</span>
            </div>
            <div class="panel-body">
                <span style="color:#777;">No Jira ticket linked to this issue.</span>
            </div>
        </div>"#
            .to_string()
    }
}

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
