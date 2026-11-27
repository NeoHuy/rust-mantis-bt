use crate::auth::AuthContext;
use crate::models::{JiraResolvedConfig, Project};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

fn render_mantis_status_options(selected: i32) -> String {
    let statuses = [
        (10, "10 - new"),
        (20, "20 - feedback"),
        (30, "30 - acknowledged"),
        (40, "40 - confirmed"),
        (50, "50 - assigned"),
        (80, "80 - resolved"),
        (90, "90 - closed"),
    ];
    let mut out = String::new();
    for (val, label) in statuses {
        let sel = if val == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{}"{}>{}</option>"#,
            val, sel, label
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn render_manage_jira_page(
    ctx: &AuthContext,
    config: &JiraResolvedConfig,
    _projects: &[Project],
    project_mappings: &[(i64, String, String)],
    cached_epics: &[(String, usize, i64)],
    status_mappings: &[(String, i32)],
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
    for (idx, (pid, pname, mapped_key)) in project_mappings.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        project_mapping_rows.push_str(&format!(
            r#"<tr class="{}">
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>
                    <input type="text" name="project_map_{}" value="{}" placeholder="e.g. PROJ" class="!w-40">
                </td>
            </tr>"#,
            row_cls,
            pid,
            html_escape(pname),
            pid,
            html_escape(mapped_key)
        ));
    }

    let mut status_mapping_rows = String::new();
    let mut row_idx = 0;
    for (j_status, m_status) in status_mappings {
        let row_cls = if row_idx % 2 == 0 { "row-1" } else { "row-2" };
        row_idx += 1;
        status_mapping_rows.push_str(&format!(
            r#"<tr class="{}">
                <td>
                    <input type="text" name="status_jira_{}" value="{}" placeholder="e.g. In Progress" class="!w-64">
                </td>
                <td>
                    <select name="status_mantis_{}" class="!w-52">
                        {}
                    </select>
                </td>
            </tr>"#,
            row_cls,
            row_idx,
            html_escape(j_status),
            row_idx,
            render_mantis_status_options(*m_status)
        ));
    }
    for _ in 0..2 {
        let row_cls = if row_idx % 2 == 0 { "row-1" } else { "row-2" };
        row_idx += 1;
        status_mapping_rows.push_str(&format!(
            r#"<tr class="{}">
                <td>
                    <input type="text" name="status_jira_{}" value="" placeholder="(New Jira status)" class="!w-64">
                </td>
                <td>
                    <select name="status_mantis_{}" class="!w-52">
                        {}
                    </select>
                </td>
            </tr>"#,
            row_cls,
            row_idx,
            row_idx,
            render_mantis_status_options(10)
        ));
    }

    let mut cached_epics_html = String::new();
    if !cached_epics.is_empty() {
        cached_epics_html.push_str(
            r#"<h3 class="mt-6 mb-2.5 text-sm font-semibold text-gray-800">Cached Jira Epics</h3>
            <div class="table-responsive">
                <table class="data-table max-w-[800px] mb-4">
                    <thead>
                        <tr>
                            <th class="w-44">Jira Project Key</th>
                            <th class="w-36">Cached Epics</th>
                            <th>Last Refreshed</th>
                        </tr>
                    </thead>
                    <tbody>"#,
        );
        for (idx, (pkey, count, updated_at)) in cached_epics.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            cached_epics_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><strong>{}</strong></td>
                    <td>{} epic(s)</td>
                    <td>{}</td>
                </tr>"#,
                row_cls,
                html_escape(pkey),
                count,
                crate::views::format_timestamp(*updated_at)
            ));
        }
        cached_epics_html.push_str("</tbody></table></div>");
    }

    let env_token_active = std::env::var("MANTIS_JIRA_API_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    let token_hint = if env_token_active {
        r#"<small class="block mt-1 text-xs text-blue-700"><span class="badge badge-info">ENV OVERRIDE</span> Supplied via <code>MANTIS_JIRA_API_TOKEN</code> environment variable — takes precedence over database value.</small>"#
    } else if !config.api_token.is_empty() {
        r#"<small class="block mt-1 text-xs text-gray-500">(API Token currently saved. Leave blank to keep existing token)</small>"#
    } else {
        ""
    };

    let body = format!(
        r#"{}
        <div class="widget-box widget-color-blue2 mb-5">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-exchange" aria-hidden="true"></i> Jira Cloud Integration Configuration</h4>
            </div>
            <div class="widget-body p-4">
                <form method="POST" action="/manage_jira_config" data-disable-on-submit>
                    <div class="table-responsive">
                        <table class="form-table form-table-left max-w-[800px]">
                            <tr>
                                <th class="category w-56">Enable Jira Integration</th>
                                <td class="input-col">
                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox" name="jira_enabled" value="1" {}>
                                        <span>Enable pushing Mantis issues to Atlassian Jira Cloud</span>
                                    </label>
                                </td>
                            </tr>
                            <tr>
                                <th class="category">Jira Base URL</th>
                                <td class="input-col">
                                    <input type="url" name="jira_base_url" value="{}" placeholder="https://yourcompany.atlassian.net" class="!w-full !max-w-[400px]" required>
                                    <small class="block mt-1 text-xs text-gray-500">Atlassian Jira Cloud URL (e.g. https://domain.atlassian.net). Internal/HTTP or on-prem hosts require <code>MANTIS_ALLOW_LOCAL_JIRA=1</code>.</small>
                                </td>
                            </tr>
                            <tr>
                                <th class="category">Jira Account Email</th>
                                <td class="input-col">
                                    <input type="email" name="jira_email" value="{}" placeholder="bot@yourcompany.com" class="!w-full !max-w-[400px]" required>
                                    <small class="block mt-1 text-xs text-gray-500">Atlassian account email address with issue creation permission</small>
                                </td>
                            </tr>
                            <tr>
                                <th class="category">Jira API Token</th>
                                <td class="input-col">
                                    <input type="password" name="jira_api_token" placeholder="{}" class="!w-full !max-w-[400px]">
                                    {}
                                </td>
                            </tr>
                            <tr>
                                <th class="category">Default Issue Type</th>
                                <td class="input-col">
                                    <select name="jira_default_issue_type" class="!w-40">
                                        <option value="Bug" {}>Bug</option>
                                        <option value="Task" {}>Task</option>
                                    </select>
                                </td>
                            </tr>
                            <tr>
                                <th class="category">Default Jira Project Key</th>
                                <td class="input-col">
                                    <input type="text" name="jira_default_project_key" value="{}" placeholder="e.g. MANTIS" class="!w-40">
                                    <small class="block mt-1 text-xs text-gray-500">Fallback key if project is not mapped below</small>
                                </td>
                            </tr>
                        </table>
                    </div>

                    <h3 class="mt-6 mb-3 text-[15px] font-semibold text-gray-800">Project Mapping (Mantis Project &rarr; Jira Project Key)</h3>
                    <div class="table-responsive">
                        <table class="data-table max-w-[800px] mb-5">
                            <thead>
                                <tr>
                                    <th class="w-16">ID</th>
                                    <th>Mantis Project Name</th>
                                    <th class="w-52">Jira Project Key</th>
                                </tr>
                            </thead>
                            <tbody>
                                {}
                            </tbody>
                        </table>
                    </div>

                    <h3 class="mt-6 mb-3 text-[15px] font-semibold text-gray-800">Status Mapping (Jira Status &rarr; Mantis Status)</h3>
                    <div class="table-responsive">
                        <table class="data-table max-w-[800px] mb-1.5">
                            <thead>
                                <tr>
                                    <th class="w-72">Jira Status (Case-insensitive)</th>
                                    <th>Mapped Mantis Status</th>
                                </tr>
                            </thead>
                            <tbody>
                                {}
                            </tbody>
                        </table>
                    </div>
                    <small class="block mb-5 text-xs text-gray-500">When issues sync from Jira, their Mantis status will automatically update to match this mapping. Leave Jira status blank to remove a mapping.</small>

                    <div class="flex items-center gap-2.5">
                        <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-check-circle" aria-hidden="true"></i> Save Jira Configuration</button>
                    </div>
                </form>

                <hr class="my-6 border-gray-200">

                <div class="flex flex-wrap items-center gap-4">
                    <form method="POST" action="/manage_jira_test" class="flex items-center gap-2 m-0" data-disable-on-submit>
                        <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-plug" aria-hidden="true"></i> Test Connection to Jira</button>
                        <small class="text-xs text-gray-600">Verifies current credentials against Atlassian Jira /rest/api/3/myself</small>
                    </form>

                    <form method="POST" action="/manage_jira_refresh_epics" class="flex items-center gap-2 m-0" data-disable-on-submit>
                        <button type="submit" class="btn btn-white btn-round btn-sm">
                            <i class="fa fa-refresh" aria-hidden="true"></i> Refresh Epics Cache
                        </button>
                        <small class="text-xs text-gray-600">Fetch and update cached Jira Epics immediately (cached for 24h by default)</small>
                    </form>
                </div>

                {}
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
        project_mapping_rows,
        status_mapping_rows,
        cached_epics_html
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
            let badge_cls = if lower.contains("done")
                || lower.contains("resolved")
                || lower.contains("closed")
            {
                "bg-emerald-700 text-white"
            } else if lower.contains("in progress") || lower.contains("dev") {
                "bg-blue-700 text-white"
            } else if lower.contains("review") || lower.contains("testing") || lower.contains("qa")
            {
                "bg-amber-600 text-white"
            } else {
                "bg-gray-600 text-white"
            };
            format!(
                r#"<span class="inline-block px-1.5 py-0.5 text-[11px] font-semibold rounded-sm {}">{}</span>"#,
                badge_cls,
                html_escape(&jl.jira_status)
            )
        } else {
            "-".to_string()
        };

        let unlink_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_unlink" class="inline-block ml-1 m-0" data-confirm="Are you sure you want to unlink this Jira ticket?">
                    <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-unlink" aria-hidden="true"></i> Unlink</button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        let sync_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/view_bug_page/{}/jira_sync" class="inline-block m-0" data-disable-on-submit>
                    <button type="submit" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-refresh" aria-hidden="true"></i> Sync with Jira</button>
                </form>"#,
                bug_id
            )
        } else {
            String::new()
        };

        format!(
            r#"<div class="widget-box widget-color-blue2" data-widget-id="jira_integration">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-external-link-square" aria-hidden="true"></i>
                        Jira Integration
                    </h4>
                    <div class="widget-toolbar">
                        <button type="button" data-action="collapse" aria-expanded="true" aria-label="Toggle Jira Integration">
                            <i class="ace-icon fa fa-chevron-up" aria-hidden="true"></i>
                        </button>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th class="w-36">Jira Ticket</th>
                                    <th class="w-24">Type</th>
                                    <th class="w-32">Jira Status</th>
                                    <th>Jira URL</th>
                                    <th class="w-52">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td>
                                        <a href="{}" target="_blank" rel="noopener noreferrer" class="font-bold text-blue-700 hover:underline">
                                            {} <i class="fa fa-external-link text-[11px]" aria-hidden="true"></i>
                                        </a>
                                    </td>
                                    <td>{}</td>
                                    <td>{}</td>
                                    <td>
                                        <a href="{}" target="_blank" rel="noopener noreferrer" class="text-gray-600 hover:underline">{}</a>
                                    </td>
                                    <td>
                                        <div class="flex flex-wrap items-center gap-1">{}{}{}</div>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
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
            r#"<div class="widget-box widget-color-blue2" data-widget-id="jira_integration">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-external-link-square" aria-hidden="true"></i>
                        Jira Integration
                    </h4>
                    <div class="widget-toolbar">
                        <button type="button" data-action="collapse" aria-expanded="true" aria-label="Toggle Jira Integration">
                            <i class="ace-icon fa fa-chevron-up" aria-hidden="true"></i>
                        </button>
                    </div>
                </div>
                <div class="widget-body">
                    <p class="mb-3 text-gray-600">No Jira ticket linked to this issue.</p>
                    <form method="POST" action="/view_bug_page/{}/jira_sync" data-disable-on-submit>
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category">Jira Ticket Number</th>
                                    <td class="input-col">
                                        <input type="text" name="jira_key" placeholder="e.g. 123 or JBC-123" class="!w-64">
                                        <small class="block mt-1 text-xs text-gray-500">Leave empty to create a new Jira ticket</small>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Epic</th>
                                    <td class="input-col">
                                        <select name="epic_key" class="!min-w-60 !w-auto">
                                            <option value="">None (No Epic)</option>
                                            {}
                                        </select>
                                        <small class="block mt-1 text-xs text-gray-500">Optional parent epic in Jira</small>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Issue Type</th>
                                    <td class="input-col">
                                        <select name="issue_type" class="!w-auto">
                                            <option value="Bug">Bug</option>
                                            <option value="Task">Task</option>
                                            <option value="Story">Story</option>
                                        </select>
                                        <small class="block mt-1 text-xs text-gray-500">Type if creating a new Jira ticket</small>
                                    </td>
                                </tr>
                            </table>
                        </div>
                        <button type="submit" class="btn btn-primary btn-white btn-round btn-sm mt-3"><i class="fa fa-refresh" aria-hidden="true"></i> Sync</button>
                    </form>
                </div>
            </div>"#,
            bug_id, epic_options
        )
    } else {
        r#"<div class="widget-box widget-color-blue2" data-widget-id="jira_integration">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="ace-icon fa fa-external-link-square" aria-hidden="true"></i>
                    Jira Integration
                </h4>
                <div class="widget-toolbar">
                    <button type="button" data-action="collapse" aria-expanded="true" aria-label="Toggle Jira Integration">
                        <i class="ace-icon fa fa-chevron-up" aria-hidden="true"></i>
                    </button>
                </div>
            </div>
            <div class="widget-body">
                <p class="text-gray-600 m-0">No Jira ticket linked to this issue.</p>
            </div>
        </div>"#
            .to_string()
    }
}

#[cfg(test)]
#[path = "jira_test.rs"]
mod tests;
