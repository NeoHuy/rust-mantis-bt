use crate::auth::AuthContext;
use crate::models::{
    Bug, BugFilter, BugFull, BugPriority, BugReproducibility, BugResolution, BugSeverity,
    BugStatus, CustomField, CustomFieldValue, JiraLink, ProjectVersion, RelationshipType,
    SavedFilter, Tag, User, ViewState,
};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use crate::views::{format_timestamp, render_status_badge, render_status_legend, status_color};
use axum::response::Html;
use std::collections::HashMap;

pub fn status_label(v: i32) -> &'static str {
    BugStatus::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("unknown")
}

pub fn priority_label(v: i32) -> &'static str {
    BugPriority::from_i32(v)
        .map(|p| p.as_str())
        .unwrap_or("unknown")
}

pub fn severity_label(v: i32) -> &'static str {
    BugSeverity::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("unknown")
}

pub fn reproducibility_label(v: i32) -> &'static str {
    BugReproducibility::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("unknown")
}

pub fn resolution_label(v: i32) -> &'static str {
    BugResolution::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("unknown")
}

pub fn relationship_label(v: i32) -> &'static str {
    RelationshipType::from_i32(v)
        .map(|r| r.as_str())
        .unwrap_or("related to")
}

pub fn render_my_view_box(title: &str, link: &str, bugs: &[Bug]) -> String {
    let mut rows = String::new();
    if bugs.is_empty() {
        rows.push_str(
            r#"<tr><td colspan="3" style="text-align:center; padding:10px; color:#777;">No issues found.</td></tr>"#,
        );
    } else {
        for b in bugs.iter().take(10) {
            let color = status_color(b.status);
            rows.push_str(&format!(
                r#"<tr>
                    <td style="width:95px; border-left:5px solid {};"><a href="/view_bug_page/{}" style="font-weight:bold;">{:07}</a><br><span style="font-size:11px; color:#555;">{}</span></td>
                    <td><a href="/view_bug_page/{}" style="font-weight:bold;">{}</a><br><span style="font-size:11px; color:#777;">Updated: {}</span></td>
                </tr>"#,
                color,
                b.id,
                b.id,
                html_escape(status_label(b.status)),
                b.id,
                html_escape(&b.summary),
                format_timestamp(b.last_updated)
            ));
        }
    }
    format!(
        r#"<div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>{} ({})</span>
                <a class="btn btn-secondary btn-sm" href="{}">View Issues</a>
            </div>
            <div class="panel-body" style="padding:0;">
                <table class="data-table" style="margin-bottom:0; border:none;">
                    <tbody>{}</tbody>
                </table>
            </div>
        </div>"#,
        html_escape(title),
        bugs.len(),
        link,
        rows
    )
}

pub fn render_my_view_page(
    ctx: &AuthContext,
    assigned_to_me: &[Bug],
    unassigned: &[Bug],
    reported_by_me: &[Bug],
    resolved: &[Bug],
    recently_modified: &[Bug],
    monitored_by_me: &[Bug],
) -> Html<String> {
    let content = format!(
        r#"<div style="display:grid; grid-template-columns: repeat(auto-fit, minmax(460px, 1fr)); gap:16px;">
            {}
            {}
            {}
            {}
            {}
            {}
        </div>
        {}"#,
        render_my_view_box(
            "Assigned to Me (Unresolved)",
            &format!(
                "/view_all_bug_page?handler_id={}&hide_status=80",
                ctx.user.id
            ),
            assigned_to_me
        ),
        render_my_view_box(
            "Unassigned",
            "/view_all_bug_page?handler_id=-1&hide_status=80",
            unassigned
        ),
        render_my_view_box(
            "Reported by Me",
            &format!("/view_all_bug_page?reporter_id={}", ctx.user.id),
            reported_by_me
        ),
        render_my_view_box("Resolved", "/view_all_bug_page?status=80", resolved),
        render_my_view_box("Recently Modified", "/view_all_bug_page", recently_modified),
        render_my_view_box(
            "Monitored by Me",
            &format!("/view_all_bug_page?monitored_by={}", ctx.user.id),
            monitored_by_me
        ),
        render_status_legend()
    );

    render_layout("My View", ctx, None, &content)
}

#[allow(clippy::too_many_arguments)]
pub fn render_view_all_bug_page(
    ctx: &AuthContext,
    bugs: &[Bug],
    total: usize,
    page: i64,
    per_page: i64,
    filter: &BugFilter,
    projects_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
    users_map: &HashMap<i64, String>,
    jira_links: &HashMap<i64, JiraLink>,
    all_users: &[User],
    all_categories: &[(i64, String)],
    all_tags: &[Tag],
    saved_filters: &[SavedFilter],
    flash_msg: Option<&str>,
    flash_err: Option<&str>,
) -> Html<String> {
    let offset = (page - 1) * per_page;
    let mut rows_html = String::new();
    if bugs.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="8" style="text-align:center; padding:16px;">No issues found.</td></tr>"#,
        );
    } else {
        for (idx, b) in bugs.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let proj_name = projects_map
                .get(&b.project_id)
                .cloned()
                .unwrap_or_else(|| format!("#{}", b.project_id));
            let cat_name = categories_map
                .get(&b.category_id)
                .cloned()
                .unwrap_or_else(|| "General".to_string());
            let handler_name = if b.handler_id > 0 {
                users_map
                    .get(&b.handler_id)
                    .cloned()
                    .unwrap_or_else(|| format!("user#{}", b.handler_id))
            } else {
                String::new()
            };
            let priv_badge = if b.view_state == 50 {
                r#" <span class="badge badge-warning">private</span>"#
            } else {
                ""
            };
            let jira_badge = if let Some(jl) = jira_links.get(&b.id) {
                format!(
                    r#" <a href="{}" target="_blank" class="badge" style="background:#0052cc; color:#fff; font-size:10px; text-decoration:none;">Jira: {}</a>"#,
                    html_escape(&jl.jira_url),
                    html_escape(&jl.jira_key)
                )
            } else {
                String::new()
            };
            let status_badge = render_status_badge(b.status, status_label(b.status));

            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td style="border-left:5px solid {};"><a href="/view_bug_page/{}" style="font-weight:bold;">{:07}</a></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}{}{}</td>
                    <td><a href="/view_bug_page/{}">{}</a>{}{}</td>
                    <td style="white-space:nowrap;">{}</td>
                </tr>"#,
                row_class,
                status_color(b.status),
                b.id,
                b.id,
                html_escape(&proj_name),
                html_escape(&cat_name),
                html_escape(severity_label(b.severity)),
                html_escape(priority_label(b.priority)),
                status_badge,
                if !handler_name.is_empty() { " (" } else { "" },
                if !handler_name.is_empty() {
                    format!("{})", html_escape(&handler_name))
                } else {
                    String::new()
                },
                b.id,
                html_escape(&b.summary),
                priv_badge,
                jira_badge,
                format_timestamp(b.last_updated)
            ));
        }
    }

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let prev_link = if page > 1 {
        format!(
            r#"<a class="btn btn-secondary btn-sm" href="/view_all_bug_page?page={}">« Prev</a>"#,
            page - 1
        )
    } else {
        String::new()
    };
    let next_link = if (offset as usize) + bugs.len() < total {
        format!(
            r#"<a class="btn btn-secondary btn-sm" href="/view_all_bug_page?page={}">Next »</a>"#,
            page + 1
        )
    } else {
        String::new()
    };

    let mut reporter_opts = String::from(r#"<option value="0">[any]</option>"#);
    let mut handler_opts = format!(
        r#"<option value="0">[any]</option><option value="-1" {}>[none]</option>"#,
        if filter.handler_id == Some(-1) {
            "selected"
        } else {
            ""
        }
    );
    let mut monitored_opts = String::from(r#"<option value="0">[any]</option>"#);
    for u in all_users {
        let rep_sel = if filter.reporter_id == Some(u.id) {
            "selected"
        } else {
            ""
        };
        reporter_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            u.id,
            rep_sel,
            html_escape(&u.username)
        ));
        let h_sel = if filter.handler_id == Some(u.id) {
            "selected"
        } else {
            ""
        };
        handler_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            u.id,
            h_sel,
            html_escape(&u.username)
        ));
        let m_sel = if filter.monitored_by == Some(u.id) {
            "selected"
        } else {
            ""
        };
        monitored_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            u.id,
            m_sel,
            html_escape(&u.username)
        ));
    }

    let mut cat_opts = String::from(r#"<option value="0">[any]</option>"#);
    for (cid, cname) in all_categories {
        let sel = if filter.category_id == Some(*cid) {
            "selected"
        } else {
            ""
        };
        cat_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            cid,
            sel,
            html_escape(cname)
        ));
    }

    let mut status_opts = String::from(r#"<option value="0">[any]</option>"#);
    let mut hide_status_opts = String::from(r#"<option value="0">[none]</option>"#);
    for (code, name) in [
        (10, "new"),
        (20, "feedback"),
        (30, "acknowledged"),
        (40, "confirmed"),
        (50, "assigned"),
        (80, "resolved"),
        (90, "closed"),
    ] {
        let s_sel = if filter.status == Some(code) {
            "selected"
        } else {
            ""
        };
        status_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            code, s_sel, name
        ));
        let hs_sel = if filter.hide_status == Some(code) {
            "selected"
        } else {
            ""
        };
        hide_status_opts.push_str(&format!(
            r#"<option value="{}" {}>{} (and above)</option>"#,
            code, hs_sel, name
        ));
    }

    let mut priority_opts = String::from(r#"<option value="0">[any]</option>"#);
    for (code, name) in [
        (10, "none"),
        (20, "low"),
        (30, "normal"),
        (40, "high"),
        (50, "urgent"),
        (60, "immediate"),
    ] {
        let sel = if filter.priority == Some(code) {
            "selected"
        } else {
            ""
        };
        priority_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            code, sel, name
        ));
    }

    let mut severity_opts = String::from(r#"<option value="0">[any]</option>"#);
    for (code, name) in [
        (10, "feature"),
        (20, "trivial"),
        (30, "text"),
        (40, "tweak"),
        (50, "minor"),
        (60, "major"),
        (70, "crash"),
        (80, "block"),
    ] {
        let sel = if filter.severity == Some(code) {
            "selected"
        } else {
            ""
        };
        severity_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            code, sel, name
        ));
    }

    let mut tag_opts = String::from(r#"<option value="0">[any]</option>"#);
    for t in all_tags {
        let sel = if filter.tag_id == Some(t.id) {
            "selected"
        } else {
            ""
        };
        tag_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            t.id,
            sel,
            html_escape(&t.name)
        ));
    }

    let mut saved_filters_html = String::new();
    for sf in saved_filters {
        saved_filters_html.push_str(&format!(
            r#"<a class="btn btn-secondary btn-sm" href="/view_all_bug_page?filter_id={}">{}</a> "#,
            sf.id,
            html_escape(&sf.name)
        ));
    }

    let search_val = filter.search.as_deref().unwrap_or("");
    let save_filter_form = if ctx.user.access_level >= 25 {
        format!(
            r#"<form method="POST" action="/filter_save" style="display:flex; gap:6px; align-items:center; margin-top:8px;">
                <input type="hidden" name="search" value="{}">
                <input type="hidden" name="status" value="{}">
                <input type="hidden" name="priority" value="{}">
                <input type="hidden" name="severity" value="{}">
                <input type="hidden" name="tag_id" value="{}">
                <input type="text" name="name" placeholder="Saved filter name" style="max-width:180px;" required>
                <label><input type="checkbox" name="is_public" value="1"> Public</label>
                <button type="submit" class="btn btn-secondary btn-sm">Save Current Filter</button>
            </form>"#,
            html_escape(search_val),
            filter.status.unwrap_or(0),
            filter.priority.unwrap_or(0),
            filter.severity.unwrap_or(0),
            filter.tag_id.unwrap_or(0)
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Filters</span>
                <a class="btn btn-secondary btn-sm" href="/view_all_bug_page">Reset Filter</a>
            </div>
            <div class="panel-body">
                <form method="GET" action="/view_all_bug_page">
                    <div class="filter-grid">
                        <div class="filter-cell">
                            <span class="filter-label">Reporter</span>
                            <select name="reporter_id">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Assigned To</span>
                            <select name="handler_id">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Monitored By</span>
                            <select name="monitored_by">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Category</span>
                            <select name="category_id">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Severity</span>
                            <select name="severity">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Priority</span>
                            <select name="priority">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Status</span>
                            <select name="status">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Hide Status</span>
                            <select name="hide_status">{}</select>
                        </div>
                        <div class="filter-cell">
                            <span class="filter-label">Tag</span>
                            <select name="tag_id">{}</select>
                        </div>
                    </div>
                    <div style="display:flex; gap:10px; align-items:center; flex-wrap:wrap;">
                        <label style="display:inline-flex; align-items:center; gap:6px; font-weight:bold; font-size:12px;">Search:
                            <input type="text" name="q" value="{}" placeholder="Search summary, description, id..." style="width:240px;">
                        </label>
                        <button type="submit" class="btn btn-sm">Apply Filter</button>
                    </div>
                </form>
                <div style="margin-top:8px;">
                    {}
                    {}
                </div>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Viewing Issues ({})</span>
                <div style="display:flex; gap:8px;">
                    <a class="btn btn-secondary btn-sm" href="/csv_export?q={}&status={}&priority={}&severity={}&tag_id={}">CSV Export</a>
                    <a class="btn btn-sm" href="/bug_report_page">Report Issue</a>
                </div>
            </div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Project</th>
                            <th>Category</th>
                            <th>Severity</th>
                            <th>Priority</th>
                            <th>Status</th>
                            <th>Summary</th>
                            <th>Updated</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
                <div style="display:flex; gap:8px; justify-content:flex-end;">
                    {} {}
                </div>
            </div>
        </div>
        {}"#,
        err_banner,
        reporter_opts,
        handler_opts,
        monitored_opts,
        cat_opts,
        severity_opts,
        priority_opts,
        status_opts,
        hide_status_opts,
        tag_opts,
        html_escape(search_val),
        saved_filters_html,
        save_filter_form,
        total,
        html_escape(search_val),
        filter.status.unwrap_or(0),
        filter.priority.unwrap_or(0),
        filter.severity.unwrap_or(0),
        filter.tag_id.unwrap_or(0),
        rows_html,
        prev_link,
        next_link,
        render_status_legend()
    );

    render_layout("View Issues", ctx, flash_msg, &content)
}

pub fn render_bug_report_page(
    ctx: &AuthContext,
    project_opts: &[(i64, String, bool)],
    categories: &[(i64, String)],
    versions: &[ProjectVersion],
    custom_fields: &[CustomField],
    flash_err: Option<&str>,
    flash_msg: Option<&str>,
) -> Html<String> {
    let mut proj_opts = String::new();
    for (pid, pname, is_selected) in project_opts {
        let sel = if *is_selected { "selected" } else { "" };
        proj_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            pid,
            sel,
            html_escape(pname)
        ));
    }

    let mut cat_opts = String::new();
    for (cid, cname) in categories {
        cat_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            cid,
            html_escape(cname)
        ));
    }

    let mut ver_opts = String::from(r#"<option value="">(none)</option>"#);
    for v in versions {
        ver_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            html_escape(&v.version),
            html_escape(&v.version)
        ));
    }

    let mut cf_rows_html = String::new();
    for cf in custom_fields {
        if cf.display_report != 0 {
            let req = if cf.require_report != 0 {
                "required"
            } else {
                ""
            };
            cf_rows_html.push_str(&format!(
                r#"<tr>
                    <td class="label-col">{}{}</td>
                    <td class="input-col"><input type="text" name="custom_field_{}" value="{}" {}></td>
                </tr>"#,
                html_escape(&cf.name),
                if cf.require_report != 0 { " *" } else { "" },
                cf.id,
                html_escape(&cf.default_value),
                req
            ));
        }
    }

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Enter Issue Details</div>
            <div class="panel-body">
                <form method="POST" action="/bug_report">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Project</td>
                            <td class="input-col"><select name="project_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Category</td>
                            <td class="input-col"><select name="category_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Reproducibility</td>
                            <td class="input-col">
                                <select name="reproducibility">
                                    <option value="10">always</option>
                                    <option value="30">sometimes</option>
                                    <option value="50">random</option>
                                    <option value="70" selected>have not tried</option>
                                    <option value="90">unable to reproduce</option>
                                    <option value="100">N/A</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Severity</td>
                            <td class="input-col">
                                <select name="severity">
                                    <option value="10">feature</option>
                                    <option value="20">trivial</option>
                                    <option value="30">text</option>
                                    <option value="40">tweak</option>
                                    <option value="50" selected>minor</option>
                                    <option value="60">major</option>
                                    <option value="70">crash</option>
                                    <option value="80">block</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority</td>
                            <td class="input-col">
                                <select name="priority">
                                    <option value="10">none</option>
                                    <option value="20">low</option>
                                    <option value="30" selected>normal</option>
                                    <option value="40">high</option>
                                    <option value="50">urgent</option>
                                    <option value="60">immediate</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Product Version</td>
                            <td class="input-col"><select name="version">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Platform / OS / Build</td>
                            <td class="input-col">
                                <input type="text" name="platform" placeholder="Platform" style="max-width:120px;">
                                <input type="text" name="os" placeholder="OS" style="max-width:120px;">
                                <input type="text" name="os_build" placeholder="OS Build" style="max-width:120px;">
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary *</td>
                            <td class="input-col"><input type="text" name="summary" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description *</td>
                            <td class="input-col"><textarea name="description"></textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Steps To Reproduce</td>
                            <td class="input-col"><textarea name="steps_to_reproduce"></textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Additional Information</td>
                            <td class="input-col"><textarea name="additional_information"></textarea></td>
                        </tr>
                        {}
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" selected>public</option>
                                    <option value="50">private</option>
                                </select>
                            </td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Submit Issue</button>
                </form>
            </div>
        </div>"#,
        err_banner, proj_opts, cat_opts, ver_opts, cf_rows_html
    );

    render_layout("Report Issue", ctx, flash_msg, &content)
}

#[allow(clippy::too_many_arguments)]
pub fn render_view_bug_page(
    ctx: &AuthContext,
    full: &BugFull,
    proj_name: &str,
    cat_name: &str,
    reporter_name: &str,
    handler_name: &str,
    cf_values: &[CustomFieldValue],
    actions_html: &str,
    tags_html: &str,
    rels_html: &str,
    rel_form_html: &str,
    files_html: &str,
    upload_form_html: &str,
    notes_html: &str,
    note_form_html: &str,
    history_html: &str,
    jira_link: Option<&JiraLink>,
    flash_msg: Option<&str>,
    flash_err: Option<&str>,
) -> Html<String> {
    let bug = &full.bug;
    let banner = if let Some(m) = flash_msg {
        format!(r#"<div class="flash-success">{}</div>"#, html_escape(m))
    } else if let Some(e) = flash_err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let jira_badge = if let Some(jl) = jira_link {
        format!(
            r#"<a href="{}" target="_blank" class="badge" style="background:#0052cc; color:#fff; text-decoration:none; margin-left:8px; font-size:11px; font-weight:bold;">Jira: {} &nearr;</a>"#,
            html_escape(&jl.jira_url),
            html_escape(&jl.jira_key)
        )
    } else {
        String::new()
    };

    let mut cf_rows_html = String::new();
    for cfv in cf_values {
        cf_rows_html.push_str(&format!(
            r#"<tr>
                <td class="label-col">{}</td>
                <td class="input-col" colspan="5">{}</td>
            </tr>"#,
            html_escape(&cfv.field_name),
            html_escape(&cfv.value)
        ));
    }

    let view_state_str = ViewState::from_i32(bug.view_state)
        .map(|v| v.as_str())
        .unwrap_or("public");

    let status_badge = render_status_badge(bug.status, status_label(bug.status));

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">View Issue Details [#{:07}]</div>
            <div class="panel-body">
                <table class="form-table" style="margin-bottom:12px;">
                    <thead>
                        <tr style="background:#edf3f7; font-weight:bold; color:#334e68;">
                            <td style="width:12%;">ID</td>
                            <td style="width:18%;">Project</td>
                            <td style="width:18%;">Category</td>
                            <td style="width:14%;">View Status</td>
                            <td style="width:19%;">Date Submitted</td>
                            <td style="width:19%;">Last Update</td>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{:07}{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Reporter</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Assigned To</td>
                            <td class="input-col" colspan="3">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Severity</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Reproducibility</td>
                            <td class="input-col">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Status</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Resolution</td>
                            <td class="input-col" colspan="3">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Platform</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">OS</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">OS Version</td>
                            <td class="input-col">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Product Version</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Target Version</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Fixed in Version</td>
                            <td class="input-col">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary</td>
                            <td class="input-col" colspan="5"><strong>{:07}: {}</strong></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col" colspan="5" style="white-space:pre-wrap;">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Steps To Reproduce</td>
                            <td class="input-col" colspan="5" style="white-space:pre-wrap;">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Additional Information</td>
                            <td class="input-col" colspan="5" style="white-space:pre-wrap;">{}</td>
                        </tr>
                        {}
                    </tbody>
                </table>
                <div style="display:flex; gap:10px; flex-wrap:wrap; align-items:center; padding-top:8px; border-top:1px solid #e0e6ed;">
                    {}
                </div>
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Tags</div>
            <div class="panel-body">
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Relationships</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Attached Files</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Activities / Bugnotes</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Issue History</div>
            <div class="panel-body">
                {}
            </div>
        </div>"#,
        banner,
        bug.id,
        bug.id,
        jira_badge,
        html_escape(proj_name),
        html_escape(cat_name),
        html_escape(view_state_str),
        format_timestamp(bug.date_submitted),
        format_timestamp(bug.last_updated),
        html_escape(reporter_name),
        html_escape(handler_name),
        html_escape(priority_label(bug.priority)),
        html_escape(severity_label(bug.severity)),
        html_escape(reproducibility_label(bug.reproducibility)),
        status_badge,
        html_escape(resolution_label(bug.resolution)),
        html_escape(&bug.platform),
        html_escape(&bug.os),
        html_escape(&bug.os_build),
        html_escape(&bug.version),
        html_escape(&bug.target_version),
        html_escape(&bug.fixed_in_version),
        bug.id,
        html_escape(&bug.summary),
        html_escape(&full.description),
        html_escape(&full.steps_to_reproduce),
        html_escape(&full.additional_information),
        cf_rows_html,
        actions_html,
        tags_html,
        rels_html,
        rel_form_html,
        files_html,
        upload_form_html,
        notes_html,
        note_form_html,
        history_html
    );

    render_layout(&format!("Issue #{:07}", bug.id), ctx, flash_msg, &content)
}

pub fn render_update_bug_page(
    ctx: &AuthContext,
    full: &BugFull,
    cat_opts: &str,
    cf_rows_html: &str,
) -> Html<String> {
    let bug = &full.bug;
    let content = format!(
        r#"<div class="panel">
            <div class="panel-header">Update Issue #{:07}</div>
            <div class="panel-body">
                <form method="POST" action="/update_bug/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Category</td>
                            <td class="input-col"><select name="category_id">{}</select></td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority</td>
                            <td class="input-col">
                                <select name="priority">
                                    <option value="10" {}>none</option>
                                    <option value="20" {}>low</option>
                                    <option value="30" {}>normal</option>
                                    <option value="40" {}>high</option>
                                    <option value="50" {}>urgent</option>
                                    <option value="60" {}>immediate</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Severity</td>
                            <td class="input-col">
                                <select name="severity">
                                    <option value="10" {}>feature</option>
                                    <option value="20" {}>trivial</option>
                                    <option value="30" {}>text</option>
                                    <option value="40" {}>tweak</option>
                                    <option value="50" {}>minor</option>
                                    <option value="60" {}>major</option>
                                    <option value="70" {}>crash</option>
                                    <option value="80" {}>block</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Reproducibility</td>
                            <td class="input-col">
                                <select name="reproducibility">
                                    <option value="10" {}>always</option>
                                    <option value="30" {}>sometimes</option>
                                    <option value="50" {}>random</option>
                                    <option value="70" {}>have not tried</option>
                                    <option value="90" {}>unable to reproduce</option>
                                    <option value="100" {}>N/A</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Product Version</td>
                            <td class="input-col"><input type="text" name="version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Fixed in Version</td>
                            <td class="input-col"><input type="text" name="fixed_in_version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Target Version</td>
                            <td class="input-col"><input type="text" name="target_version" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state">
                                    <option value="10" {}>public</option>
                                    <option value="50" {}>private</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary *</td>
                            <td class="input-col"><input type="text" name="summary" value="{}" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description">{}</textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Steps To Reproduce</td>
                            <td class="input-col"><textarea name="steps_to_reproduce">{}</textarea></td>
                        </tr>
                        <tr>
                            <td class="label-col">Additional Information</td>
                            <td class="input-col"><textarea name="additional_information">{}</textarea></td>
                        </tr>
                        {}
                    </table>
                    <button type="submit" class="btn">Update Information</button>
                </form>
            </div>
        </div>"#,
        bug.id,
        bug.id,
        cat_opts,
        if bug.priority == 10 { "selected" } else { "" },
        if bug.priority == 20 { "selected" } else { "" },
        if bug.priority == 30 { "selected" } else { "" },
        if bug.priority == 40 { "selected" } else { "" },
        if bug.priority == 50 { "selected" } else { "" },
        if bug.priority == 60 { "selected" } else { "" },
        if bug.severity == 10 { "selected" } else { "" },
        if bug.severity == 20 { "selected" } else { "" },
        if bug.severity == 30 { "selected" } else { "" },
        if bug.severity == 40 { "selected" } else { "" },
        if bug.severity == 50 { "selected" } else { "" },
        if bug.severity == 60 { "selected" } else { "" },
        if bug.severity == 70 { "selected" } else { "" },
        if bug.severity == 80 { "selected" } else { "" },
        if bug.reproducibility == 10 {
            "selected"
        } else {
            ""
        },
        if bug.reproducibility == 30 {
            "selected"
        } else {
            ""
        },
        if bug.reproducibility == 50 {
            "selected"
        } else {
            ""
        },
        if bug.reproducibility == 70 {
            "selected"
        } else {
            ""
        },
        if bug.reproducibility == 90 {
            "selected"
        } else {
            ""
        },
        if bug.reproducibility == 100 {
            "selected"
        } else {
            ""
        },
        html_escape(&bug.version),
        html_escape(&bug.fixed_in_version),
        html_escape(&bug.target_version),
        if bug.view_state == 10 { "selected" } else { "" },
        if bug.view_state == 50 { "selected" } else { "" },
        html_escape(&bug.summary),
        html_escape(&full.description),
        html_escape(&full.steps_to_reproduce),
        html_escape(&full.additional_information),
        cf_rows_html
    );

    render_layout(&format!("Update Issue #{:07}", bug.id), ctx, None, &content)
}
