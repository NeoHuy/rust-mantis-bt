use crate::auth::AuthContext;
use crate::models::{
    build_filter_permalink_url, BugFilter, BugPriority, BugReproducibility, BugResolution,
    BugSeverity, BugStatus, RelationshipType, SavedFilter, FILTER_MATCH_ANY,
};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;
use std::collections::{HashMap, HashSet};

fn summarize_filter_criteria(filter: &BugFilter) -> Vec<String> {
    let mut parts = Vec::new();
    if filter.match_type == FILTER_MATCH_ANY {
        parts.push("Match: ANY (OR)".to_string());
    }
    if let Some(ref s) = filter.search {
        if !s.trim().is_empty() {
            parts.push(format!("Search: \"{}\"", s.trim()));
        }
    }
    let statuses = filter.effective_statuses();
    if !statuses.is_empty() {
        let names: Vec<&str> = statuses
            .iter()
            .filter_map(|&v| BugStatus::from_i32(v).map(|s| s.as_str()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("Status: {}", names.join(", ")));
        }
    }
    if let Some(hs) = filter.hide_status.filter(|&v| v > 0) {
        if let Some(st) = BugStatus::from_i32(hs) {
            parts.push(format!("Hide Status: >= {}", st.as_str()));
        }
    }
    let priorities = filter.effective_priorities();
    if !priorities.is_empty() {
        let names: Vec<&str> = priorities
            .iter()
            .filter_map(|&v| BugPriority::from_i32(v).map(|p| p.as_str()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("Priority: {}", names.join(", ")));
        }
    }
    let severities = filter.effective_severities();
    if !severities.is_empty() {
        let names: Vec<&str> = severities
            .iter()
            .filter_map(|&v| BugSeverity::from_i32(v).map(|s| s.as_str()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("Severity: {}", names.join(", ")));
        }
    }
    let resolutions = filter.effective_resolutions();
    if !resolutions.is_empty() {
        let names: Vec<&str> = resolutions
            .iter()
            .filter_map(|&v| BugResolution::from_i32(v).map(|r| r.as_str()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("Resolution: {}", names.join(", ")));
        }
    }
    let reproducibilities = filter.effective_reproducibilities();
    if !reproducibilities.is_empty() {
        let names: Vec<&str> = reproducibilities
            .iter()
            .filter_map(|&v| BugReproducibility::from_i32(v).map(|r| r.as_str()))
            .collect();
        if !names.is_empty() {
            parts.push(format!("Reproducibility: {}", names.join(", ")));
        }
    }
    if let Some(vs) = filter.view_state {
        parts.push(format!(
            "View Status: {}",
            if vs == 50 { "private" } else { "public" }
        ));
    }
    if let Some(st) = filter.sticky {
        parts.push(format!("Sticky: {}", if st == 1 { "yes" } else { "no" }));
    }
    if !filter.versions.is_empty() {
        parts.push(format!("Version: {}", filter.versions.join(", ")));
    }
    if !filter.target_versions.is_empty() {
        parts.push(format!(
            "Target Version: {}",
            filter.target_versions.join(", ")
        ));
    }
    if !filter.fixed_in_versions.is_empty() {
        parts.push(format!(
            "Fixed in Version: {}",
            filter.fixed_in_versions.join(", ")
        ));
    }
    if filter.date_submitted_from.is_some() || filter.date_submitted_to.is_some() {
        parts.push(format!(
            "Submitted: {} .. {}",
            filter.date_submitted_from.as_deref().unwrap_or("*"),
            filter.date_submitted_to.as_deref().unwrap_or("*")
        ));
    }
    if filter.last_updated_from.is_some() || filter.last_updated_to.is_some() {
        parts.push(format!(
            "Updated: {} .. {}",
            filter.last_updated_from.as_deref().unwrap_or("*"),
            filter.last_updated_to.as_deref().unwrap_or("*")
        ));
    }
    if filter.relationship_type.is_some() || filter.relationship_bug_id.is_some() {
        let rt_label = filter
            .relationship_type
            .and_then(|rt| {
                if rt == -1 {
                    Some("any")
                } else {
                    RelationshipType::from_i32(rt).map(|r| r.as_str())
                }
            })
            .unwrap_or("any");
        let bug_suffix = filter
            .relationship_bug_id
            .map(|id| format!(" #{:07}", id))
            .unwrap_or_default();
        parts.push(format!("Relationship: {}{}", rt_label, bug_suffix));
    }
    if !filter.note_user_ids.is_empty() {
        parts.push(format!(
            "Note Authors: {} user(s)",
            filter.note_user_ids.len()
        ));
    }
    if !filter.custom_fields.is_empty() {
        parts.push(format!("Custom Fields: {}", filter.custom_fields.len()));
    }
    if parts.is_empty() {
        parts.push("All accessible issues (no constraints)".to_string());
    }
    parts
}

pub fn render_manage_filter_page(
    ctx: &AuthContext,
    filters: &[SavedFilter],
    manageable_ids: &HashSet<i64>,
    users_map: &HashMap<i64, String>,
    projects_map: &HashMap<i64, String>,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    if filters.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="6" class="text-center py-4 text-gray-500">No saved filters found for the selected project.</td></tr>"#,
        );
    } else {
        for (idx, sf) in filters.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let proj_name = if sf.project_id == 0 {
                "All Projects".to_string()
            } else {
                projects_map
                    .get(&sf.project_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Project #{}", sf.project_id))
            };
            let owner_name = users_map
                .get(&sf.user_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", sf.user_id));
            let vis_badge = if sf.is_public != 0 {
                r#"<span class="badge badge-success">Public</span>"#
            } else {
                r#"<span class="badge badge-info">Private</span>"#
            };
            let permalink = format!("/view_all_bug_page?filter_id={}", sf.id);
            let can_manage = manageable_ids.contains(&sf.id)
                || sf.user_id == ctx.user.id
                || (sf.is_public != 0 && ctx.user.access_level >= 70);
            let edit_btn = if can_manage {
                format!(
                    r#"<a href="/manage_filter_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>"#,
                    sf.id
                )
            } else {
                String::new()
            };
            let delete_form = if can_manage {
                format!(
                    r#"<form method="POST" action="/filter_delete/{}" class="inline-form">
                        <input type="hidden" name="return_to" value="manage">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm" data-confirm="Delete filter?"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                    </form>"#,
                    sf.id
                )
            } else {
                String::new()
            };

            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><a href="{}" class="font-semibold text-blue-700 hover:underline">{}</a></td>
                    <td class="whitespace-nowrap">{}</td>
                    <td class="text-center whitespace-nowrap">{}</td>
                    <td class="whitespace-nowrap">{}</td>
                    <td>
                        <div class="inline-flex items-center gap-1.5">
                            <input type="text" id="filter-permalink-{}" value="{}" readonly class="!w-52 text-xs bg-gray-50" aria-label="Filter permalink">
                            <button type="button" class="btn btn-white btn-round btn-sm" data-copy-target="filter-permalink-{}" title="Copy Permalink"><i class="fa fa-link" aria-hidden="true"></i> Copy</button>
                        </div>
                    </td>
                    <td class="whitespace-nowrap">
                        <div class="inline-flex items-center gap-1.5">
                            <a href="{}" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-check" aria-hidden="true"></i> Apply</a>
                            {}
                            {}
                        </div>
                    </td>
                </tr>"#,
                row_class,
                permalink,
                html_escape(&sf.name),
                html_escape(&proj_name),
                vis_badge,
                html_escape(&owner_name),
                sf.id,
                html_escape(&permalink),
                sf.id,
                permalink,
                edit_btn,
                delete_form
            ));
        }
    }

    let content = format!(
        r#"{}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa fa-filter" aria-hidden="true"></i> Manage Saved Filters ({})
                </h4>
                <div class="widget-toolbar">
                    <a href="/view_all_bug_page" class="btn btn-white btn-round btn-sm">
                        <i class="fa fa-arrow-left" aria-hidden="true"></i> Back to View Issues
                    </a>
                </div>
            </div>
            <div class="widget-body !p-0">
                <div class="widget-main !p-0">
                    <div class="table-responsive">
                        <table class="data-table !mb-0 !border-0">
                            <thead>
                                <tr>
                                    <th>Filter Name</th>
                                    <th>Project</th>
                                    <th class="text-center">Visibility</th>
                                    <th>Owner</th>
                                    <th>Shareable Permalink</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {}
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>"#,
        err_banner,
        filters.len(),
        rows_html
    );

    render_layout("Manage Filters", ctx, msg, &content)
}

pub fn render_manage_filter_edit_page(
    ctx: &AuthContext,
    sf: &SavedFilter,
    filter: &BugFilter,
    projects: &[(i64, String)],
    can_toggle_public: bool,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut project_opts = format!(
        r#"<option value="0" {}>All Projects</option>"#,
        if sf.project_id == 0 { "selected" } else { "" }
    );
    for (pid, pname) in projects {
        let sel = if sf.project_id == *pid {
            "selected"
        } else {
            ""
        };
        project_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            pid,
            sel,
            html_escape(pname)
        ));
    }

    let visibility_row = if can_toggle_public {
        let pub_checked = if sf.is_public != 0 { "checked" } else { "" };
        let priv_checked = if sf.is_public == 0 { "checked" } else { "" };
        format!(
            r#"<tr>
                <td class="label-col">Visibility</td>
                <td class="input-col">
                    <label class="inline-label"><input type="radio" name="is_public" value="1" {}> Public (visible to all users with project access)</label>
                    <label class="inline-label"><input type="radio" name="is_public" value="0" {}> Private (only visible to owner)</label>
                </td>
            </tr>"#,
            pub_checked, priv_checked
        )
    } else {
        let label = if sf.is_public != 0 {
            "Public"
        } else {
            "Private"
        };
        format!(
            r#"<tr>
                <td class="label-col">Visibility</td>
                <td class="input-col"><strong>{}</strong> <span class="text-xs text-gray-500">(Owner with Manager privileges required to change visibility)</span></td>
            </tr>"#,
            label
        )
    };

    let match_all_sel = if filter.match_type != FILTER_MATCH_ANY {
        "selected"
    } else {
        ""
    };
    let match_any_sel = if filter.match_type == FILTER_MATCH_ANY {
        "selected"
    } else {
        ""
    };

    let criteria_badges: String = summarize_filter_criteria(filter)
        .into_iter()
        .map(|item| {
            format!(
                r#"<span class="reminder-badge">{}</span>"#,
                html_escape(&item)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    let id_permalink = build_filter_permalink_url(filter, Some(sf.id));
    let criteria_permalink = build_filter_permalink_url(filter, None);

    let content = format!(
        r#"{}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa fa-pencil" aria-hidden="true"></i> Edit Saved Filter: {}
                </h4>
                <div class="widget-toolbar">
                    <a href="{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-check" aria-hidden="true"></i> Apply Filter</a>
                    <a href="/manage_filter_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back</a>
                </div>
            </div>
            <div class="widget-body">
                <form method="POST" action="/manage_filter_update/{}">
                    <table class="form-table form-table-left">
                        <tr>
                            <td class="label-col">Filter Name <span class="required">*</span></td>
                            <td class="input-col"><input type="text" name="name" value="{}" required maxlength="64"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Project</td>
                            <td class="input-col">
                                <select name="project_id" class="select-auto">{}</select>
                            </td>
                        </tr>
                        {}
                        <tr>
                            <td class="label-col">Match Type</td>
                            <td class="input-col">
                                <select name="match_type" class="select-auto">
                                    <option value="0" {}>Match All Criteria (AND)</option>
                                    <option value="1" {}>Match Any Criterion (OR)</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Stored Criteria</td>
                            <td class="input-col"><div class="flex flex-wrap gap-1.5">{}</div></td>
                        </tr>
                        <tr>
                            <td class="label-col">Saved Filter Permalink</td>
                            <td class="input-col">
                                <div class="inline-flex items-center gap-2 flex-wrap">
                                    <input type="text" id="edit-filter-permalink" value="{}" readonly class="!w-80 bg-gray-50 text-xs">
                                    <button type="button" class="btn btn-white btn-round btn-sm" data-copy-target="edit-filter-permalink"><i class="fa fa-link" aria-hidden="true"></i> Copy Link</button>
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Criteria Query Permalink</td>
                            <td class="input-col">
                                <div class="inline-flex items-center gap-2 flex-wrap">
                                    <input type="text" id="edit-criteria-permalink" value="{}" readonly class="!w-80 bg-gray-50 text-xs">
                                    <button type="button" class="btn btn-white btn-round btn-sm" data-copy-target="edit-criteria-permalink"><i class="fa fa-link" aria-hidden="true"></i> Copy Criteria URL</button>
                                </div>
                            </td>
                        </tr>
                    </table>
                    <div class="mt-3 flex items-center gap-2">
                        <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-save" aria-hidden="true"></i> Update Filter</button>
                        <a href="/manage_filter_page" class="btn btn-secondary btn-sm">Cancel</a>
                    </div>
                </form>
                <div class="mt-4 pt-3 border-t border-gray-200">
                    <form method="POST" action="/filter_delete/{}" class="inline-form">
                        <input type="hidden" name="return_to" value="manage">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm" data-confirm="Delete this saved filter?"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete Filter</button>
                    </form>
                </div>
            </div>
        </div>"#,
        err_banner,
        html_escape(&sf.name),
        id_permalink,
        sf.id,
        html_escape(&sf.name),
        project_opts,
        visibility_row,
        match_all_sel,
        match_any_sel,
        criteria_badges,
        html_escape(&id_permalink),
        html_escape(&criteria_permalink),
        sf.id
    );

    render_layout("Edit Filter", ctx, msg, &content)
}

#[cfg(test)]
#[path = "filters_test.rs"]
mod tests;
