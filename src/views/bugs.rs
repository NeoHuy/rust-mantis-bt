use crate::auth::AuthContext;
use crate::models::{
    build_csv_export_url, build_view_all_bug_page_url, clamp_per_page, Bug, BugFile, BugFilter,
    BugFull, BugHistory, BugPriority, BugRelationship, BugReproducibility, BugResolution,
    BugRevision, BugSeverity, BugStatus, BugTag, Bugnote, Category, CustomField, CustomFieldValue,
    JiraLink, ListBugsQuery, ProjectVersion, RelationshipType, SavedFilter, Tag, TimelineEvent,
    User, ViewState,
};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use crate::views::{
    format_date, format_timestamp, render_status_badge, render_status_legend, status_color,
};
use axum::response::Html;
use std::collections::{HashMap, HashSet};

pub fn format_history_value(
    field_name: &str,
    val: &str,
    users_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
) -> String {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match field_name {
        "status" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(st) = BugStatus::from_i32(code) {
                    return st.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "resolution" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(res) = BugResolution::from_i32(code) {
                    return res.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "priority" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(pr) = BugPriority::from_i32(code) {
                    return pr.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "severity" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(sev) = BugSeverity::from_i32(code) {
                    return sev.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "reproducibility" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(rep) = BugReproducibility::from_i32(code) {
                    return rep.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "view_state" => {
            if let Ok(code) = trimmed.parse::<i32>() {
                if let Some(vs) = ViewState::from_i32(code) {
                    return vs.as_str().to_string();
                }
            }
            trimmed.to_string()
        }
        "sticky" => {
            if trimmed == "1" {
                "yes".to_string()
            } else if trimmed == "0" {
                "no".to_string()
            } else {
                trimmed.to_string()
            }
        }
        "due_date" => {
            if let Ok(ts) = trimmed.parse::<i64>() {
                if ts > 0 {
                    let d = format_date(ts);
                    if !d.is_empty() {
                        return d;
                    }
                } else if ts == 0 {
                    return "none".to_string();
                }
            }
            trimmed.to_string()
        }
        "handler_id" => {
            if let Ok(uid) = trimmed.parse::<i64>() {
                if uid == 0 {
                    return "none".to_string();
                }
                if let Some(uname) = users_map.get(&uid) {
                    return uname.clone();
                }
                return format!("user#{}", uid);
            }
            trimmed.to_string()
        }
        "category_id" => {
            if let Ok(cid) = trimmed.parse::<i64>() {
                if cid == 0 {
                    return "none".to_string();
                }
                if let Some(cname) = categories_map.get(&cid) {
                    return cname.clone();
                }
                return format!("category#{}", cid);
            }
            trimmed.to_string()
        }
        "bugnote_view_state" => {
            if trimmed == "50" {
                "private".to_string()
            } else if trimmed == "10" {
                "public".to_string()
            } else {
                trimmed.to_string()
            }
        }
        "bugnote_updated" => {
            if let Ok(nid) = trimmed.parse::<i64>() {
                return format!("bugnote #{:07}", nid);
            }
            trimmed.to_string()
        }
        _ => trimmed.to_string(),
    }
}

pub fn render_bug_history_html(
    history: &[BugHistory],
    users_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
) -> String {
    if history.is_empty() {
        return "<p>No history entries.</p>".to_string();
    }
    let mut history_html = String::from(
        r#"<table class="data-table"><thead><tr><th>Date Modified</th><th>Username</th><th>Field</th><th>Change</th></tr></thead><tbody>"#,
    );
    for h in history {
        let uname = users_map
            .get(&h.user_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", h.user_id));
        let change_desc = if h.type_ == 1 && h.field_name.is_empty() {
            "New Issue".to_string()
        } else if h.field_name == "monitor_added" || (h.type_ == 12 && h.field_name.is_empty()) {
            let target = if !h.new_value.is_empty() {
                h.new_value.clone()
            } else if let Ok(uid) = h.old_value.parse::<i64>() {
                users_map.get(&uid).cloned().unwrap_or(h.old_value.clone())
            } else {
                h.old_value.clone()
            };
            format!("Monitor Added: {}", html_escape(&target))
        } else if h.field_name == "monitor_removed" || (h.type_ == 13 && h.field_name.is_empty()) {
            let target = if !h.old_value.is_empty() {
                h.old_value.clone()
            } else if let Ok(uid) = h.new_value.parse::<i64>() {
                users_map.get(&uid).cloned().unwrap_or(h.new_value.clone())
            } else {
                h.new_value.clone()
            };
            format!("Monitor Removed: {}", html_escape(&target))
        } else {
            let formatted_old =
                format_history_value(&h.field_name, &h.old_value, users_map, categories_map);
            let formatted_new =
                format_history_value(&h.field_name, &h.new_value, users_map, categories_map);
            format!(
                "{} =&gt; {}",
                html_escape(&formatted_old),
                html_escape(&formatted_new)
            )
        };
        history_html.push_str(&format!(
            r#"<tr>
                <td style="width:150px; white-space:nowrap;">{}</td>
                <td style="width:150px;">{}</td>
                <td style="width:180px;">{}</td>
                <td>{}</td>
            </tr>"#,
            format_timestamp(h.date_modified),
            html_escape(&uname),
            html_escape(&h.field_name),
            change_desc
        ));
    }
    history_html.push_str("</tbody></table>");
    history_html
}

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

#[derive(Debug, Clone, Default)]
pub struct MyViewMeta {
    pub categories_map: HashMap<i64, String>,
    pub projects_map: HashMap<i64, String>,
    pub editable_bug_ids: HashSet<i64>,
    pub timeline_events: Vec<TimelineEvent>,
    pub users_map: HashMap<i64, String>,
    pub active_project_id: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ViewIssuesMeta {
    pub note_counts: HashMap<i64, usize>,
    pub file_counts: HashMap<i64, usize>,
    pub editable_bug_ids: HashSet<i64>,
}

pub fn render_priority_icon(priority: i32) -> String {
    let label = priority_label(priority);
    match priority {
        20 => format!(
            r#"<i class="fa fa-chevron-down text-emerald-600" title="{}" aria-hidden="true"></i>"#,
            label
        ),
        30 => format!(
            r#"<i class="fa fa-minus text-amber-500" title="{}" aria-hidden="true"></i>"#,
            label
        ),
        40 => format!(
            r#"<i class="fa fa-chevron-up text-red-600" title="{}" aria-hidden="true"></i>"#,
            label
        ),
        50 => format!(
            r#"<i class="fa fa-arrow-up text-red-600" title="{}" aria-hidden="true"></i>"#,
            label
        ),
        60 => format!(
            r#"<i class="fa fa-exclamation-triangle text-red-600" title="{}" aria-hidden="true"></i>"#,
            label
        ),
        _ => format!(
            r#"<span class="inline-block w-3 text-center text-gray-400" title="{}">&nbsp;</span>"#,
            label
        ),
    }
}

pub fn render_status_square(status: i32) -> String {
    let color = status_color(status);
    let label = status_label(status);
    format!(
        r#"<i class="fa fa-square fa-status-box" style="color:{};" title="{}" aria-hidden="true"></i>"#,
        color,
        html_escape(label)
    )
}

fn my_view_box_meta_attrs(title: &str) -> (&'static str, &'static str) {
    match title {
        "Assigned to Me (Unresolved)" => ("my_view_assigned", "fa-user"),
        "Unassigned" => ("my_view_unassigned", "fa-inbox"),
        "Reported by Me" => ("my_view_reported", "fa-bug"),
        "Resolved" => ("my_view_resolved", "fa-check-square-o"),
        "Recently Modified" => ("my_view_recent", "fa-history"),
        "Monitored by Me" => ("my_view_monitored", "fa-eye"),
        _ => ("my_view_box", "fa-list-alt"),
    }
}

pub fn render_my_view_box(title: &str, link: &str, bugs: &[Bug]) -> String {
    let (widget_id, icon) = my_view_box_meta_attrs(title);
    render_my_view_box_with_meta(title, link, bugs, widget_id, icon, &MyViewMeta::default())
}

pub fn render_my_view_box_with_meta(
    title: &str,
    link: &str,
    bugs: &[Bug],
    widget_id: &str,
    icon: &str,
    meta: &MyViewMeta,
) -> String {
    let mut rows = String::new();
    let shown_count = bugs.len().min(10);
    let range_badge = if bugs.is_empty() {
        "0".to_string()
    } else {
        format!("1 - {} / {}", shown_count, bugs.len())
    };

    if bugs.is_empty() {
        rows.push_str(
            r#"<tr><td colspan="2" class="text-center py-3 text-xs text-gray-500">No issues found.</td></tr>"#,
        );
    } else {
        for b in bugs.iter().take(10) {
            let status_square = render_status_square(b.status);
            let priority_icon = render_priority_icon(b.priority);
            let edit_link = if meta.editable_bug_ids.contains(&b.id) {
                format!(
                    r#"<a href="/update_bug_page/{}" class="text-gray-600 hover:text-blue-600" title="Edit"><i class="fa fa-pencil" aria-hidden="true"></i></a>"#,
                    b.id
                )
            } else {
                String::new()
            };
            let priv_icon = if b.view_state == 50 {
                r#"<i class="fa fa-lock text-gray-400" title="private" aria-hidden="true"></i>"#
            } else {
                ""
            };
            let sticky_icon = if b.sticky == 1 {
                r#" <i class="fa fa-thumb-tack text-amber-600" title="Sticky" aria-hidden="true"></i>"#
            } else {
                ""
            };
            let resolved_cls = if b.status >= 80 {
                " issue-link-resolved"
            } else {
                ""
            };
            let cat_name = meta
                .categories_map
                .get(&b.category_id)
                .map(|s| s.as_str())
                .unwrap_or("General");
            let proj_prefix = if meta.active_project_id == 0 {
                meta.projects_map
                    .get(&b.project_id)
                    .map(|p| format!("[{}] ", html_escape(p)))
                    .unwrap_or_default()
            } else {
                String::new()
            };

            rows.push_str(&format!(
                r#"<tr>
                    <td class="w-32 whitespace-nowrap align-top">
                        <div class="flex items-center gap-1.5">
                            {}
                            {}
                            {}
                            {}
                        </div>
                        <div class="mt-0.5">
                            <a href="/view_bug_page/{}" class="font-bold{}">{:07}</a>
                        </div>
                    </td>
                    <td class="align-top">
                        <div>
                            <a href="/view_bug_page/{}" class="font-semibold hover:underline{}">{}</a>{}
                        </div>
                        <div class="text-[11px] text-gray-500 mt-0.5">
                            {}{} &middot; {} &middot; Updated: {}
                        </div>
                    </td>
                </tr>"#,
                status_square,
                edit_link,
                priority_icon,
                priv_icon,
                b.id,
                resolved_cls,
                b.id,
                b.id,
                resolved_cls,
                html_escape(&b.summary),
                sticky_icon,
                proj_prefix,
                html_escape(cat_name),
                html_escape(status_label(b.status)),
                format_timestamp(b.last_updated)
            ));
        }
    }

    format!(
        r#"<div class="widget-box widget-color-blue2 !mb-0" data-widget-id="{}">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa {}" aria-hidden="true"></i>
                    <a href="{}" class="text-white hover:text-white hover:underline">{} ({})</a>
                    <span class="badge" style="background:rgba(0,0,0,0.22); color:#ffffff;">{}</span>
                </h4>
                <div class="widget-toolbar">
                    <div class="widget-menu">
                        <a class="btn btn-white btn-round btn-sm" href="{}">View Issues</a>
                    </div>
                    <button type="button" data-action="collapse" aria-expanded="true" title="Toggle section">
                        <i class="fa fa-chevron-up" aria-hidden="true"></i>
                    </button>
                </div>
            </div>
            <div class="widget-body !p-0">
                <div class="widget-main !p-0">
                    <div class="table-responsive">
                        <table class="data-table !mb-0 !border-0">
                            <tbody>{}</tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>"#,
        html_escape(widget_id),
        html_escape(icon),
        html_escape(link),
        html_escape(title),
        bugs.len(),
        range_badge,
        html_escape(link),
        rows
    )
}

fn describe_timeline_event(
    ev: &TimelineEvent,
    users_map: &HashMap<i64, String>,
) -> (&'static str, &'static str, String) {
    if ev.type_ == 1 && ev.field_name.is_empty() {
        return ("fa-plus", "bg-emerald-600", "created issue".to_string());
    }
    match ev.field_name.as_str() {
        "bugnote_added" => (
            "fa-comment-o",
            "bg-sky-600",
            "commented on issue".to_string(),
        ),
        "bugnote_updated" => (
            "fa-pencil",
            "bg-sky-600",
            "edited a note on issue".to_string(),
        ),
        "bugnote_view_state" => (
            "fa-eye-slash",
            "bg-slate-600",
            "updated note visibility on issue".to_string(),
        ),
        "status" => {
            let old_code = ev.old_value.trim().parse::<i32>().unwrap_or(0);
            let new_code = ev.new_value.trim().parse::<i32>().unwrap_or(0);
            if old_code < 90 && new_code >= 90 {
                ("fa-power-off", "bg-gray-600", "closed issue".to_string())
            } else if old_code < 80 && new_code >= 80 {
                (
                    "fa-thumbs-o-up",
                    "bg-green-600",
                    "resolved issue".to_string(),
                )
            } else if old_code >= 80 && new_code < 80 {
                ("fa-refresh", "bg-amber-600", "reopened issue".to_string())
            } else {
                (
                    "fa-exchange",
                    "bg-blue-600",
                    format!(
                        "set status to {} on issue",
                        html_escape(status_label(new_code))
                    ),
                )
            }
        }
        "handler_id" => {
            let new_uid = ev.new_value.trim().parse::<i64>().unwrap_or(0);
            if new_uid > 0 {
                let target = users_map
                    .get(&new_uid)
                    .cloned()
                    .unwrap_or_else(|| format!("user#{}", new_uid));
                (
                    "fa-user",
                    "bg-indigo-600",
                    format!("assigned to {} on issue", html_escape(&target)),
                )
            } else {
                ("fa-user", "bg-slate-500", "unassigned issue".to_string())
            }
        }
        "file_added" => (
            "fa-paperclip",
            "bg-teal-600",
            format!("attached {} to issue", html_escape(&ev.new_value)),
        ),
        "file_deleted" => (
            "fa-trash-o",
            "bg-red-600",
            format!("removed {} from issue", html_escape(&ev.old_value)),
        ),
        "tag_attached" => (
            "fa-tag",
            "bg-purple-600",
            format!("tagged {} on issue", html_escape(&ev.new_value)),
        ),
        "tag_detached" => (
            "fa-tag",
            "bg-slate-500",
            format!("untagged {} on issue", html_escape(&ev.old_value)),
        ),
        "monitor_added" => (
            "fa-eye",
            "bg-cyan-600",
            "added monitor on issue".to_string(),
        ),
        "monitor_removed" => (
            "fa-eye-slash",
            "bg-slate-500",
            "removed monitor on issue".to_string(),
        ),
        "project_id" => ("fa-arrows", "bg-amber-600", "moved issue".to_string()),
        "created_from" | "cloned_to" => ("fa-copy", "bg-blue-600", "cloned issue".to_string()),
        other if !other.is_empty() => (
            "fa-edit",
            "bg-blue-500",
            format!("updated {} on issue", html_escape(other)),
        ),
        _ => ("fa-info-circle", "bg-blue-500", "updated issue".to_string()),
    }
}

pub fn render_timeline_widget(
    events: &[TimelineEvent],
    users_map: &HashMap<i64, String>,
) -> String {
    let body_html = if events.is_empty() {
        r#"<div class="widget-main"><p class="text-xs text-gray-500 text-center py-6 m-0">No recent activity found.</p></div>"#.to_string()
    } else {
        let mut items = String::new();
        for ev in events {
            let (icon, bg_cls, action_desc) = describe_timeline_event(ev, users_map);
            let resolved_cls = if ev.bug_status >= 80 {
                " issue-link-resolved"
            } else {
                ""
            };
            items.push_str(&format!(
                r#"<div class="profile-activity flex items-start gap-2.5 px-3 py-2.5 text-xs">
                    <span class="thumbicon {} text-white shrink-0"><i class="fa {}" aria-hidden="true"></i></span>
                    <div class="min-w-0 flex-1">
                        <div>
                            <strong>{}</strong> {} <a href="/view_bug_page/{}" class="font-bold{}">{:07}</a>
                        </div>
                        <div class="text-gray-600 truncate mt-0.5" title="{}">{}</div>
                        <div class="text-[11px] text-gray-400 mt-0.5">
                            <i class="fa fa-clock-o" aria-hidden="true"></i> {}
                        </div>
                    </div>
                </div>"#,
                bg_cls,
                icon,
                html_escape(&ev.username),
                action_desc,
                ev.bug_id,
                resolved_cls,
                ev.bug_id,
                html_escape(&ev.bug_summary),
                html_escape(&ev.bug_summary),
                format_timestamp(ev.date_modified)
            ));
        }
        format!(
            r#"<div class="widget-main !p-0"><div class="profile-feed divide-y divide-gray-200">{}</div></div>"#,
            items
        )
    };

    format!(
        r#"<div id="timeline" class="widget-box widget-color-blue2 !mb-0" data-widget-id="my_view_timeline">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa fa-clock-o" aria-hidden="true"></i> Timeline
                </h4>
                <div class="widget-toolbar">
                    <button type="button" data-action="collapse" aria-expanded="true" title="Toggle Timeline">
                        <i class="fa fa-chevron-up" aria-hidden="true"></i>
                    </button>
                </div>
            </div>
            <div class="widget-body !p-0">
                {}
            </div>
        </div>"#,
        body_html
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_my_view_page(
    ctx: &AuthContext,
    assigned_to_me: &[Bug],
    unassigned: &[Bug],
    reported_by_me: &[Bug],
    resolved: &[Bug],
    recently_modified: &[Bug],
    monitored_by_me: &[Bug],
    meta: &MyViewMeta,
) -> Html<String> {
    let assigned_link = format!(
        "/view_all_bug_page?handler_id={}&hide_status=80",
        ctx.user.id
    );
    let reported_link = format!("/view_all_bug_page?reporter_id={}", ctx.user.id);
    let monitored_link = format!("/view_all_bug_page?monitored_by={}", ctx.user.id);

    let content = format!(
        r#"<div class="grid grid-cols-1 lg:grid-cols-12 gap-4 items-start">
            <div class="lg:col-span-7 space-y-4">
                {}
                {}
                {}
                {}
                {}
                {}
            </div>
            <div class="lg:col-span-5">
                {}
            </div>
        </div>
        {}"#,
        render_my_view_box_with_meta(
            "Assigned to Me (Unresolved)",
            &assigned_link,
            assigned_to_me,
            "my_view_assigned",
            "fa-user",
            meta
        ),
        render_my_view_box_with_meta(
            "Unassigned",
            "/view_all_bug_page?handler_id=-1&hide_status=80",
            unassigned,
            "my_view_unassigned",
            "fa-inbox",
            meta
        ),
        render_my_view_box_with_meta(
            "Reported by Me",
            &reported_link,
            reported_by_me,
            "my_view_reported",
            "fa-bug",
            meta
        ),
        render_my_view_box_with_meta(
            "Resolved",
            "/view_all_bug_page?status=80",
            resolved,
            "my_view_resolved",
            "fa-check-square-o",
            meta
        ),
        render_my_view_box_with_meta(
            "Recently Modified",
            "/view_all_bug_page",
            recently_modified,
            "my_view_recent",
            "fa-history",
            meta
        ),
        render_my_view_box_with_meta(
            "Monitored by Me",
            &monitored_link,
            monitored_by_me,
            "my_view_monitored",
            "fa-eye",
            meta
        ),
        render_timeline_widget(&meta.timeline_events, &meta.users_map),
        render_status_legend()
    );

    render_layout("My View", ctx, None, &content)
}

fn render_sortable_th_with_title(
    q: &ListBugsQuery,
    sort_key: &str,
    label: &str,
    title: Option<&str>,
) -> String {
    let current_sort = q.sort.as_deref().unwrap_or("last_updated");
    let current_dir = q.dir.as_deref().unwrap_or("desc");
    let is_active = current_sort == sort_key;

    let next_dir = if is_active {
        if current_dir.eq_ignore_ascii_case("asc") {
            "desc"
        } else {
            "asc"
        }
    } else {
        "desc"
    };

    let caret_icon = if is_active {
        if current_dir.eq_ignore_ascii_case("asc") {
            r#" <i class="fa fa-caret-up text-blue-600" aria-hidden="true"></i>"#
        } else {
            r#" <i class="fa fa-caret-down text-blue-600" aria-hidden="true"></i>"#
        }
    } else {
        ""
    };

    let mut target_q = q.clone();
    target_q.sort = Some(sort_key.to_string());
    target_q.dir = Some(next_dir.to_string());
    let url = build_view_all_bug_page_url(&target_q, 1);
    let title_attr = title
        .map(|t| format!(r#" title="{}""#, html_escape(t)))
        .unwrap_or_default();

    format!(
        r#"<th class="whitespace-nowrap"{}><a href="{}" class="inline-flex items-center gap-1 text-inherit no-underline hover:underline">{}{}</a></th>"#,
        title_attr, url, label, caret_icon
    )
}

fn render_sortable_th(q: &ListBugsQuery, sort_key: &str, label: &str) -> String {
    render_sortable_th_with_title(q, sort_key, label, None)
}

fn count_active_filters(filter: &BugFilter, query: &ListBugsQuery) -> usize {
    let mut count = 0;
    if filter
        .search
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        count += 1;
    }
    if filter.status.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.hide_status.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.priority.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.severity.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.category_id.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.reporter_id.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.handler_id.unwrap_or(0) != 0 {
        count += 1;
    }
    if filter.tag_id.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter.monitored_by.unwrap_or(0) > 0 {
        count += 1;
    }
    if filter
        .target_version
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        count += 1;
    }
    if filter
        .fixed_in_version
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        count += 1;
    }
    if query.filter_id.unwrap_or(0) > 0 {
        count += 1;
    }
    count
}

fn build_clear_version_url(
    query: &ListBugsQuery,
    filter: &BugFilter,
    clear_target: bool,
) -> String {
    let mut clear_q = query.clone();
    let from_saved_filter = if clear_target {
        query
            .target_version
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
            && query.filter_id.unwrap_or(0) > 0
    } else {
        query
            .fixed_in_version
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
            && query.filter_id.unwrap_or(0) > 0
    };

    if from_saved_filter {
        clear_q.filter_id = None;
        if clear_q.project_id.is_none() {
            clear_q.project_id = filter.project_id.filter(|&v| v > 0);
        }
        if clear_q.search.is_none() && clear_q.q.is_none() {
            clear_q.search = filter.search.clone();
        }
        if clear_q.status.is_none() {
            clear_q.status = filter.status.filter(|&v| v > 0);
        }
        if clear_q.hide_status.is_none() {
            clear_q.hide_status = filter.hide_status.filter(|&v| v > 0);
        }
        if clear_q.priority.is_none() {
            clear_q.priority = filter.priority.filter(|&v| v > 0);
        }
        if clear_q.severity.is_none() {
            clear_q.severity = filter.severity.filter(|&v| v > 0);
        }
        if clear_q.category_id.is_none() {
            clear_q.category_id = filter.category_id.filter(|&v| v > 0);
        }
        if clear_q.reporter_id.is_none() {
            clear_q.reporter_id = filter.reporter_id.filter(|&v| v > 0);
        }
        if clear_q.handler_id.is_none() {
            clear_q.handler_id = filter.handler_id.filter(|&v| v != 0);
        }
        if clear_q.tag_id.is_none() {
            clear_q.tag_id = filter.tag_id.filter(|&v| v > 0);
        }
        if clear_q.monitored_by.is_none() {
            clear_q.monitored_by = filter.monitored_by.filter(|&v| v > 0);
        }
        if !clear_target && clear_q.target_version.is_none() {
            clear_q.target_version = filter.target_version.clone();
        }
        if clear_target && clear_q.fixed_in_version.is_none() {
            clear_q.fixed_in_version = filter.fixed_in_version.clone();
        }
    }

    if clear_target {
        clear_q.target_version = None;
    } else {
        clear_q.fixed_in_version = None;
    }
    build_view_all_bug_page_url(&clear_q, 1)
}

fn render_version_scope_bar(filter: &BugFilter, query: &ListBugsQuery) -> String {
    let mut chips = String::new();
    if let Some(ref tv) = filter.target_version {
        let trimmed = tv.trim();
        if !trimmed.is_empty() {
            let clear_url = build_clear_version_url(query, filter, true);
            chips.push_str(&format!(
                r#" <a class="btn btn-white btn-round btn-sm" href="{}" title="Clear Target Version filter">Target Version: {} <i class="fa fa-times" aria-hidden="true"></i></a>"#,
                html_escape(&clear_url),
                html_escape(trimmed)
            ));
        }
    }
    if let Some(ref fv) = filter.fixed_in_version {
        let trimmed = fv.trim();
        if !trimmed.is_empty() {
            let clear_url = build_clear_version_url(query, filter, false);
            chips.push_str(&format!(
                r#" <a class="btn btn-white btn-round btn-sm" href="{}" title="Clear Fixed in Version filter">Fixed in Version: {} <i class="fa fa-times" aria-hidden="true"></i></a>"#,
                html_escape(&clear_url),
                html_escape(trimmed)
            ));
        }
    }
    if chips.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="widget-toolbox flex flex-wrap items-center gap-2 text-xs"><span class="font-semibold text-gray-700">Version Scope:</span>{}</div>"#,
            chips
        )
    }
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
    query: &ListBugsQuery,
    meta: &ViewIssuesMeta,
) -> Html<String> {
    let offset = (page - 1) * per_page;
    let show_project_col = ctx.active_project_id == 0;
    let col_count = if show_project_col { 12 } else { 11 };

    let mut rows_html = String::new();
    if bugs.is_empty() {
        rows_html.push_str(&format!(
            r#"<tr><td colspan="{}" class="text-center py-4 text-gray-500">No issues found.</td></tr>"#,
            col_count
        ));
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

            let edit_cell = if meta.editable_bug_ids.contains(&b.id) {
                format!(
                    r#"<a href="/update_bug_page/{}" class="text-gray-600 hover:text-blue-600" title="Edit"><i class="fa fa-pencil" aria-hidden="true"></i></a>"#,
                    b.id
                )
            } else {
                "&nbsp;".to_string()
            };

            let priority_icon = render_priority_icon(b.priority);
            let resolved_cls = if b.status >= 80 {
                " issue-link-resolved"
            } else {
                ""
            };
            let sticky_pin = if b.sticky == 1 {
                r#" <i class="fa fa-thumb-tack text-amber-600" title="Sticky" aria-hidden="true"></i>"#
            } else {
                ""
            };

            let note_cnt = meta.note_counts.get(&b.id).copied().unwrap_or(0);
            let note_cell = if note_cnt > 0 {
                format!(
                    r#"<a href="/view_bug_page/{}#bugnotes" class="font-semibold text-blue-700 hover:underline" title="{} note(s)">{}</a>"#,
                    b.id, note_cnt, note_cnt
                )
            } else {
                "&nbsp;".to_string()
            };

            let file_cnt = meta.file_counts.get(&b.id).copied().unwrap_or(0);
            let file_cell = if file_cnt > 0 {
                format!(
                    r#"<a href="/view_bug_page/{}#attachments" class="inline-flex items-center gap-0.5 text-blue-700 hover:underline" title="{} attachment(s)"><i class="fa fa-paperclip text-gray-500" aria-hidden="true"></i>{}</a>"#,
                    b.id, file_cnt, file_cnt
                )
            } else {
                "&nbsp;".to_string()
            };

            let proj_cell = if show_project_col {
                format!(
                    r#"<td class="whitespace-nowrap">{}</td>"#,
                    html_escape(&proj_name)
                )
            } else {
                String::new()
            };

            let sev_str = html_escape(severity_label(b.severity));
            let severity_cell = if b.severity >= 60 {
                format!(r#"<span class="font-bold text-red-700">{}</span>"#, sev_str)
            } else {
                sev_str
            };

            let status_square = render_status_square(b.status);
            let handler_suffix = if !handler_name.is_empty() {
                format!(" ({})", html_escape(&handler_name))
            } else {
                String::new()
            };

            let priv_badge = if b.view_state == 50 {
                r#" <span class="badge badge-warning">private</span>"#
            } else {
                ""
            };
            let sticky_badge = if b.sticky == 1 {
                r#" <span class="badge" style="background:#e0a800; color:#fff; font-size:10px; font-weight:bold; padding:2px 4px; border-radius:3px;">[Sticky]</span>"#
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

            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td class="text-center"><input type="checkbox" name="bug_arr" value="{}"></td>
                    <td class="text-center">{}</td>
                    <td class="text-center">{}</td>
                    <td class="whitespace-nowrap"><a href="/view_bug_page/{}" class="font-bold{}">{:07}</a>{}</td>
                    <td class="text-center">{}</td>
                    <td class="text-center">{}</td>
                    {}
                    <td class="whitespace-nowrap">{}</td>
                    <td class="whitespace-nowrap">{}</td>
                    <td class="whitespace-nowrap"><span class="inline-flex items-center gap-1.5">{}<span>{}{}</span></span></td>
                    <td class="whitespace-nowrap">{}</td>
                    <td class="min-w-[200px]"><a href="/view_bug_page/{}" class="font-medium hover:underline{}">{}</a>{}{}{}</td>
                </tr>"#,
                row_class,
                b.id,
                edit_cell,
                priority_icon,
                b.id,
                resolved_cls,
                b.id,
                sticky_pin,
                note_cell,
                file_cell,
                proj_cell,
                html_escape(&cat_name),
                severity_cell,
                status_square,
                html_escape(status_label(b.status)),
                handler_suffix,
                format_timestamp(b.last_updated),
                b.id,
                resolved_cls,
                html_escape(&b.summary),
                priv_badge,
                sticky_badge,
                jira_badge
            ));
        }
    }

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let prev_link = if page > 1 {
        format!(
            r#"<a class="btn btn-white btn-round btn-sm" href="{}">« Prev</a>"#,
            build_view_all_bug_page_url(query, page - 1)
        )
    } else {
        String::new()
    };
    let next_link = if (offset as usize) + bugs.len() < total {
        format!(
            r#"<a class="btn btn-white btn-round btn-sm" href="{}">Next »</a>"#,
            build_view_all_bug_page_url(query, page + 1)
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
    let mut saved_filter_select_opts =
        String::from(r#"<option value="0">-- Load Saved Filter --</option>"#);
    for sf in saved_filters {
        let sf_sel = if query.filter_id == Some(sf.id) {
            "selected"
        } else {
            ""
        };
        saved_filter_select_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            sf.id,
            sf_sel,
            html_escape(&sf.name)
        ));
        let delete_btn = if sf.user_id == ctx.user.id || ctx.user.access_level >= 70 {
            format!(
                r#"<form method="POST" action="/filter_delete/{}" style="display:inline; margin-left:2px;"><button type="submit" class="btn btn-danger btn-sm" data-confirm="Delete filter?" title="Delete filter">Delete</button></form>"#,
                sf.id
            )
        } else {
            String::new()
        };
        saved_filters_html.push_str(&format!(
            r#"<span class="saved-filter-item inline-flex items-center gap-1 mr-2 mb-1"><a class="btn btn-white btn-round btn-sm" href="/view_all_bug_page?filter_id={}">{}</a>{}</span>"#,
            sf.id,
            html_escape(&sf.name),
            delete_btn
        ));
    }
    let saved_filters_section = if saved_filters.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="flex flex-wrap items-center gap-2 mb-2">
                <label class="text-xs font-semibold text-gray-700" for="saved_filter_select">Saved Filters:</label>
                <select id="saved_filter_select" class="!w-auto text-xs">{}</select>
                <div class="flex flex-wrap items-center">{}</div>
            </div>"#,
            saved_filter_select_opts, saved_filters_html
        )
    };

    let active_per_page = clamp_per_page(query.per_page.or(filter.per_page));
    let mut per_page_html =
        String::from(r#"<span class="text-xs font-medium text-gray-700 mr-1.5">Per page:</span>"#);
    for &cnt in &[10, 25, 50, 100] {
        if cnt == active_per_page {
            per_page_html.push_str(&format!(
                r#"<span class="btn btn-primary btn-sm font-bold pointer-events-none">{}</span> "#,
                cnt
            ));
        } else {
            let mut pp_q = query.clone();
            pp_q.per_page = Some(cnt);
            let url = build_view_all_bug_page_url(&pp_q, 1);
            per_page_html.push_str(&format!(
                r#"<a class="btn btn-white btn-round btn-sm" href="{}">{}</a> "#,
                url, cnt
            ));
        }
    }

    let cur_sort = query.sort.as_deref().unwrap_or("");
    let cur_dir = query.dir.as_deref().unwrap_or("");
    let cur_per_page_str = active_per_page.to_string();

    let mut bulk_proj_opts = String::new();
    for (p, _) in &ctx.visible_projects {
        if p.enabled == 1 {
            bulk_proj_opts.push_str(&format!(
                r#"<option value="{}">{}</option>"#,
                p.id,
                html_escape(&p.name)
            ));
        }
    }

    let mut bulk_handler_opts = String::from(r#"<option value="0">(Unassigned)</option>"#);
    for u in all_users {
        bulk_handler_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            u.id,
            html_escape(&u.username)
        ));
    }

    let mut bulk_cat_opts = String::new();
    for (cid, cname) in all_categories {
        bulk_cat_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            cid,
            html_escape(cname)
        ));
    }

    let current_page_url = build_view_all_bug_page_url(query, page);
    let return_query = current_page_url
        .split_once('?')
        .map(|(_, qs)| qs)
        .unwrap_or("");

    let th_priority = render_sortable_th_with_title(query, "priority", "P", Some("Priority"));
    let th_id = render_sortable_th(query, "id", "ID");
    let th_project = if show_project_col {
        r#"<th class="whitespace-nowrap">Project</th>"#
    } else {
        ""
    };
    let th_category = r#"<th class="whitespace-nowrap">Category</th>"#;
    let th_severity = render_sortable_th(query, "severity", "Severity");
    let th_status = render_sortable_th(query, "status", "Status");
    let th_updated = render_sortable_th(query, "last_updated", "Updated");
    let th_summary = render_sortable_th(query, "summary", "Summary");

    let search_val = filter.search.as_deref().unwrap_or("");
    let active_filter_count = count_active_filters(filter, query);
    let active_filter_badge = if active_filter_count > 0 {
        format!(
            r#"<span class="badge" style="background:#f89406; color:#ffffff;">Active ({})</span>"#,
            active_filter_count
        )
    } else {
        String::new()
    };

    let mut quick_hidden_inputs = String::new();
    if !cur_sort.is_empty() {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="sort" value="{}">"#,
            html_escape(cur_sort)
        ));
    }
    if !cur_dir.is_empty() {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="dir" value="{}">"#,
            html_escape(cur_dir)
        ));
    }
    quick_hidden_inputs.push_str(&format!(
        r#"<input type="hidden" name="per_page" value="{}">"#,
        cur_per_page_str
    ));
    if let Some(v) = filter.reporter_id.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="reporter_id" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.handler_id.filter(|&x| x != 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="handler_id" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.monitored_by.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="monitored_by" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.category_id.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="category_id" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.severity.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="severity" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.priority.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="priority" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.status.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="status" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.hide_status.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="hide_status" value="{}">"#,
            v
        ));
    }
    if let Some(v) = filter.tag_id.filter(|&x| x > 0) {
        quick_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="tag_id" value="{}">"#,
            v
        ));
    }

    let mut version_scope_hidden_inputs = String::new();
    if let Some(pid) = query.project_id.filter(|&x| x > 0) {
        version_scope_hidden_inputs.push_str(&format!(
            r#"<input type="hidden" name="project_id" value="{}">"#,
            pid
        ));
    }
    if let Some(ref tv) = filter.target_version {
        let trimmed = tv.trim();
        if !trimmed.is_empty() {
            version_scope_hidden_inputs.push_str(&format!(
                r#"<input type="hidden" name="target_version" value="{}">"#,
                html_escape(trimmed)
            ));
        }
    }
    if let Some(ref fv) = filter.fixed_in_version {
        let trimmed = fv.trim();
        if !trimmed.is_empty() {
            version_scope_hidden_inputs.push_str(&format!(
                r#"<input type="hidden" name="fixed_in_version" value="{}">"#,
                html_escape(trimmed)
            ));
        }
    }
    quick_hidden_inputs.push_str(&version_scope_hidden_inputs);

    let save_filter_form = if ctx.user.access_level >= 25 {
        format!(
            r#"<form method="POST" action="/filter_save" class="flex flex-wrap gap-2 items-center mt-2">
                <input type="hidden" name="search" value="{}">
                <input type="hidden" name="status" value="{}">
                <input type="hidden" name="hide_status" value="{}">
                <input type="hidden" name="priority" value="{}">
                <input type="hidden" name="severity" value="{}">
                <input type="hidden" name="category_id" value="{}">
                <input type="hidden" name="reporter_id" value="{}">
                <input type="hidden" name="handler_id" value="{}">
                <input type="hidden" name="tag_id" value="{}">
                <input type="hidden" name="sort" value="{}">
                <input type="hidden" name="dir" value="{}">
                <input type="hidden" name="per_page" value="{}">
                {}
                <input type="text" name="name" placeholder="Saved filter name" class="!max-w-[180px]" required>
                <label class="text-xs inline-flex items-center gap-1"><input type="checkbox" name="is_public" value="1"> Public</label>
                <button type="submit" class="btn btn-primary btn-sm">Save Current Filter</button>
            </form>"#,
            html_escape(search_val),
            filter.status.unwrap_or(0),
            filter.hide_status.unwrap_or(0),
            filter.priority.unwrap_or(0),
            filter.severity.unwrap_or(0),
            filter.category_id.unwrap_or(0),
            filter.reporter_id.unwrap_or(0),
            filter.handler_id.unwrap_or(0),
            filter.tag_id.unwrap_or(0),
            html_escape(cur_sort),
            html_escape(cur_dir),
            cur_per_page_str,
            version_scope_hidden_inputs
        )
    } else {
        String::new()
    };

    let range_badge = if bugs.is_empty() {
        format!("0 / {}", total)
    } else {
        let start = (offset as usize) + 1;
        let end = (offset as usize) + bugs.len();
        format!("{} - {} / {}", start, end, total)
    };

    let version_scope_bar = render_version_scope_bar(filter, query);

    let content = format!(
        r#"{}
        <noscript><style>#filter.collapsed > .widget-body {{ display: block; }}</style></noscript>
        <div id="filter" class="widget-box widget-color-blue2 collapsed" data-widget-id="view_issues_filter">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <button type="button" data-action="collapse" aria-expanded="false" title="Toggle Filters" class="bg-transparent border-0 text-white cursor-pointer p-0 mr-1">
                        <i class="fa fa-chevron-down" aria-hidden="true"></i>
                    </button>
                    <i class="fa fa-filter" aria-hidden="true"></i> Filters {}
                </h4>
                <div class="widget-toolbar flex-wrap">
                    <form method="GET" action="/view_all_bug_page" class="inline-flex items-center gap-1.5 m-0">
                        {}
                        <input type="text" name="q" value="{}" placeholder="Search..." aria-label="Quick search" class="!w-36 sm:!w-44 !py-1 !px-2 text-xs text-gray-800 bg-white">
                        <button type="submit" class="btn btn-white btn-round btn-sm">Apply Filter</button>
                    </form>
                    <a class="btn btn-white btn-round btn-sm" href="/view_all_bug_page" title="Reset Filter">
                        <i class="fa fa-times" aria-hidden="true"></i> Reset
                    </a>
                </div>
            </div>
            {}
            <div class="widget-body">
                <form method="GET" action="/view_all_bug_page">
                    <input type="hidden" name="sort" value="{}">
                    <input type="hidden" name="dir" value="{}">
                    <input type="hidden" name="per_page" value="{}">
                    {}
                    <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3 mb-3 pb-3 border-b border-gray-200">
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
                    <div class="flex flex-wrap gap-2 items-center">
                        <label class="inline-flex items-center gap-1.5 font-semibold text-xs text-gray-700">Search:
                            <input type="text" name="q" value="{}" placeholder="Search summary, description, id..." class="!w-60">
                        </label>
                        <button type="submit" class="btn btn-primary btn-sm">Apply Filter</button>
                        <a class="btn btn-white btn-round btn-sm" href="/view_all_bug_page">Reset Filter</a>
                    </div>
                </form>
                <div class="mt-3 pt-2 border-t border-gray-200">
                    {}
                    {}
                </div>
            </div>
        </div>

        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa fa-columns" aria-hidden="true"></i>
                    Viewing Issues ({})
                    <span class="badge" style="background:rgba(0,0,0,0.22); color:#ffffff;">{}</span>
                </h4>
                <div class="widget-toolbar">
                    <a class="btn btn-white btn-round btn-sm" href="{}"><i class="fa fa-download" aria-hidden="true"></i> CSV Export</a>
                    <a class="btn btn-success btn-sm" href="/bug_report_page"><i class="fa fa-edit" aria-hidden="true"></i> Report Issue</a>
                </div>
            </div>
            <div class="widget-body !p-0">
                <form id="bulk_action_form" method="POST" action="/bug_actiongroup">
                    <input type="hidden" name="return_query" value="{}">
                    <div class="widget-toolbox flex flex-wrap items-center justify-between gap-2">
                        <div class="flex flex-wrap items-center gap-1">
                            {}
                        </div>
                        <div class="flex items-center gap-1.5">
                            {} {}
                        </div>
                    </div>
                    <div class="widget-main !p-0">
                        <div class="table-responsive">
                            <table id="buglist" class="data-table !mb-0 !border-0">
                                <thead>
                                    <tr>
                                        <th class="w-7 text-center"><input type="checkbox" id="select_all_bugs" title="Select All"></th>
                                        <th class="w-7 text-center" title="Edit">&nbsp;</th>
                                        {}
                                        {}
                                        <th class="w-8 text-center" title="Notes"><i class="fa fa-comment-o" aria-hidden="true"></i></th>
                                        <th class="w-8 text-center" title="Attachments"><i class="fa fa-paperclip" aria-hidden="true"></i></th>
                                        {}
                                        {}
                                        {}
                                        {}
                                        {}
                                        {}
                                    </tr>
                                </thead>
                                <tbody>
                                    {}
                                </tbody>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox flex flex-wrap items-center justify-between gap-3">
                        <div class="bulk-action-bar flex flex-wrap gap-2 items-center">
                            <span class="font-bold text-xs text-gray-700">Action:</span>
                            <select name="action" id="bulk_action_select" class="!w-auto font-semibold text-xs">
                                <option value="" selected disabled>-- Select Action --</option>
                                <option value="MOVE">Move</option>
                                <option value="COPY">Copy</option>
                                <option value="ASSIGN">Assign</option>
                                <option value="CLOSE">Close</option>
                                <option value="DELETE">Delete</option>
                                <option value="RESOLVE">Resolve</option>
                                <option value="UP_STATUS">Update Status</option>
                                <option value="UP_PRIOR">Update Priority</option>
                                <option value="UP_SEV">Update Severity</option>
                                <option value="UP_CATEGORY">Update Category</option>
                                <option value="UP_TARGET_VERSION">Update Target Version</option>
                                <option value="UP_DUE_DATE">Update Due Date</option>
                                <option value="VIEW_STATUS">Update View Status</option>
                                <option value="SET_STICKY">Set Sticky</option>
                                <option value="ADD_NOTE">Add Note</option>
                                <option value="ATTACH_TAGS">Attach Tags</option>
                            </select>
                            <div class="bulk-input-group" data-actions="MOVE,COPY" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Project:</span>
                                <select name="target_project_id" class="!w-auto">{}</select>
                            </div>
                            <div class="bulk-input-group" data-actions="ASSIGN" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Assign To:</span>
                                <select name="handler_id" class="!w-auto">{}</select>
                            </div>
                            <div class="bulk-input-group" data-actions="UP_STATUS" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Status:</span>
                                <select name="status" class="!w-auto">
                                    <option value="10">new</option>
                                    <option value="20">feedback</option>
                                    <option value="30">acknowledged</option>
                                    <option value="40">confirmed</option>
                                    <option value="50">assigned</option>
                                    <option value="80">resolved</option>
                                    <option value="90">closed</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="RESOLVE,UP_STATUS" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Resolution:</span>
                                <select name="resolution" class="!w-auto">
                                    <option value="20" selected>fixed</option>
                                    <option value="10">open</option>
                                    <option value="30">reopened</option>
                                    <option value="40">unable to reproduce</option>
                                    <option value="50">not fixable</option>
                                    <option value="60">duplicate</option>
                                    <option value="70">not a bug</option>
                                    <option value="80">suspended</option>
                                    <option value="90">won't fix</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="UP_PRIOR" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Priority:</span>
                                <select name="priority" class="!w-auto">
                                    <option value="10">none</option>
                                    <option value="20">low</option>
                                    <option value="30" selected>normal</option>
                                    <option value="40">high</option>
                                    <option value="50">urgent</option>
                                    <option value="60">immediate</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="UP_SEV" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Severity:</span>
                                <select name="severity" class="!w-auto">
                                    <option value="10">feature</option>
                                    <option value="20">trivial</option>
                                    <option value="30">text</option>
                                    <option value="40">tweak</option>
                                    <option value="50" selected>minor</option>
                                    <option value="60">major</option>
                                    <option value="70">crash</option>
                                    <option value="80">block</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="UP_CATEGORY" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Category:</span>
                                <select name="category_id" class="!w-auto">{}</select>
                            </div>
                            <div class="bulk-input-group" data-actions="UP_TARGET_VERSION" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Target Version:</span>
                                <input type="text" name="target_version" placeholder="Version string" class="!w-[140px]">
                            </div>
                            <div class="bulk-input-group" data-actions="UP_DUE_DATE" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Due Date:</span>
                                <input type="date" name="due_date" class="!w-[140px]">
                            </div>
                            <div class="bulk-input-group" data-actions="VIEW_STATUS" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">View Status:</span>
                                <select name="view_state" class="!w-auto">
                                    <option value="10">public</option>
                                    <option value="50">private</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="SET_STICKY" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Sticky:</span>
                                <select name="sticky" class="!w-auto">
                                    <option value="0">no</option>
                                    <option value="1">yes</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="ADD_NOTE" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Note:</span>
                                <input type="text" name="bugnote_text" placeholder="Enter note text..." class="!w-[200px]">
                                <select name="note_view_state" class="!w-auto">
                                    <option value="10">public</option>
                                    <option value="50">private</option>
                                </select>
                            </div>
                            <div class="bulk-input-group" data-actions="ATTACH_TAGS" style="display:none; align-items:center; gap:6px;">
                                <span class="text-xs">Tags:</span>
                                <input type="text" name="tag_string" placeholder="Comma-separated tags" class="!w-[180px]">
                            </div>
                            <button type="submit" class="btn btn-primary btn-sm">Apply</button>
                        </div>
                        <div class="flex items-center gap-1.5 ml-auto">
                            {} {}
                        </div>
                    </div>
                </form>
            </div>
        </div>
        {}"#,
        err_banner,
        active_filter_badge,
        quick_hidden_inputs,
        html_escape(search_val),
        version_scope_bar,
        html_escape(cur_sort),
        html_escape(cur_dir),
        cur_per_page_str,
        version_scope_hidden_inputs,
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
        saved_filters_section,
        save_filter_form,
        total,
        range_badge,
        build_csv_export_url(query),
        html_escape(return_query),
        per_page_html,
        prev_link,
        next_link,
        th_priority,
        th_id,
        th_project,
        th_category,
        th_severity,
        th_status,
        th_updated,
        th_summary,
        rows_html,
        bulk_proj_opts,
        bulk_handler_opts,
        bulk_cat_opts,
        prev_link,
        next_link,
        render_status_legend()
    );

    render_layout("View Issues", ctx, flash_msg, &content)
}

#[allow(clippy::too_many_arguments)]
pub fn render_bug_report_page(
    ctx: &AuthContext,
    project_opts: &[(i64, String, bool)],
    categories: &[(i64, String)],
    selected_category_id: Option<i64>,
    versions: &[ProjectVersion],
    assignable_users: &[User],
    custom_fields: &[CustomField],
    effective_access: i32,
    flash_err: Option<&str>,
    flash_msg: Option<&str>,
    created_bug_id: Option<i64>,
    report_stay: bool,
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
        let sel = if selected_category_id == Some(*cid) {
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

    let mut ver_opts = String::from(r#"<option value="">(none)</option>"#);
    let mut target_ver_opts = String::from(r#"<option value="">(none)</option>"#);
    let allow_unreleased = effective_access >= crate::models::ACCESS_DEVELOPER;
    for v in versions {
        if v.obsolete == 0 {
            let escaped = html_escape(&v.version);
            if v.released != 0 || allow_unreleased {
                ver_opts.push_str(&format!(r#"<option value="{0}">{0}</option>"#, escaped));
            }
            if v.released == 0 && allow_unreleased {
                target_ver_opts.push_str(&format!(r#"<option value="{0}">{0}</option>"#, escaped));
            }
        }
    }

    let target_ver_row = if allow_unreleased {
        format!(
            r#"<tr>
                <th class="category"><label for="target_version">Target Version</label></th>
                <td class="input-col"><select id="target_version" name="target_version">{}</select></td>
            </tr>"#,
            target_ver_opts
        )
    } else {
        String::new()
    };

    let assign_to_row = if effective_access >= crate::models::ACCESS_DEVELOPER {
        let mut user_opts =
            String::from(r#"<option value="0" selected>(Unassigned / Category Default)</option>"#);
        for u in assignable_users {
            let display = if !u.realname.trim().is_empty() {
                format!("{} ({})", u.username, u.realname)
            } else {
                u.username.clone()
            };
            user_opts.push_str(&format!(
                r#"<option value="{}">{}</option>"#,
                u.id,
                html_escape(&display)
            ));
        }
        format!(
            r#"<tr>
                <th class="category"><label for="handler_id">Assign To</label></th>
                <td class="input-col"><select id="handler_id" name="handler_id">{}</select></td>
            </tr>"#,
            user_opts
        )
    } else {
        String::new()
    };

    let dummy_cats: Vec<Category> = categories
        .iter()
        .map(|(id, name)| Category {
            id: *id,
            project_id: 0,
            user_id: 0,
            name: name.clone(),
            status: 0,
        })
        .collect();
    let empty_vals = HashMap::new();
    let cf_rows_html = crate::views::custom_fields::render_custom_fields_form_rows(
        custom_fields,
        &empty_vals,
        effective_access,
        crate::views::custom_fields::CustomFieldFormMode::Report,
        versions,
        &dummy_cats,
    );

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let (created_banner, layout_flash) = if let Some(cid) = created_bug_id {
        (
            format!(
                r#"<div class="flash-message"><i class="fa fa-check-circle" aria-hidden="true"></i> Issue <a href="/view_bug_page/{cid}">#{cid:07}</a> submitted successfully.</div>"#,
                cid = cid
            ),
            None,
        )
    } else {
        (String::new(), flash_msg)
    };

    let dropzone_html = render_dropzone_html("report", "Drag and drop files here, or");

    let content = format!(
        r#"{}{}
        <form method="POST" action="/bug_report" enctype="multipart/form-data">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-edit" aria-hidden="true"></i>
                        Enter Issue Details
                    </h4>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category"><label for="project_id">Project</label></th>
                                    <td class="input-col"><select id="project_id" name="project_id" data-reload-url="/bug_report_page">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="category_id">Category</label></th>
                                    <td class="input-col"><select id="category_id" name="category_id">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="reproducibility">Reproducibility</label></th>
                                    <td class="input-col">
                                        <select id="reproducibility" name="reproducibility">
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
                                    <th class="category"><label for="severity">Severity</label></th>
                                    <td class="input-col">
                                        <select id="severity" name="severity">
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
                                    <th class="category"><label for="priority">Priority</label></th>
                                    <td class="input-col">
                                        <select id="priority" name="priority">
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
                                    <th class="category"><label for="platform">Platform</label></th>
                                    <td class="input-col"><input type="text" id="platform" name="platform" placeholder="e.g. x86_64, arm64"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="os">OS</label></th>
                                    <td class="input-col"><input type="text" id="os" name="os" placeholder="e.g. Linux, macOS, Windows"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="os_build">OS Version</label></th>
                                    <td class="input-col"><input type="text" id="os_build" name="os_build" placeholder="e.g. 14.5, 22.04 LTS"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="version">Product Version</label></th>
                                    <td class="input-col"><select id="version" name="version">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="build">Product Build</label></th>
                                    <td class="input-col"><input type="text" id="build" name="build" placeholder="Product Build" class="input-w-220"></td>
                                </tr>
                                {}
                                {}
                                <tr>
                                    <th class="category"><label for="due_date">Due Date</label></th>
                                    <td class="input-col"><input type="date" id="due_date" name="due_date" class="input-w-220"></td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="summary">Summary</label></th>
                                    <td class="input-col"><input type="text" id="summary" name="summary" required></td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="description">Description</label></th>
                                    <td class="input-col"><textarea id="description" name="description" rows="6"></textarea></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="steps_to_reproduce">Steps To Reproduce</label></th>
                                    <td class="input-col"><textarea id="steps_to_reproduce" name="steps_to_reproduce" rows="4"></textarea></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="additional_information">Additional Information</label></th>
                                    <td class="input-col"><textarea id="additional_information" name="additional_information" rows="4"></textarea></td>
                                </tr>
                                {}
                                <tr>
                                    <th class="category">Attach Files</th>
                                    <td class="input-col">{}</td>
                                </tr>
                                <tr>
                                    <th class="category">View Status</th>
                                    <td class="input-col">
                                        <label class="inline-label">
                                            <input type="radio" name="view_state" value="10" checked> public
                                        </label>
                                        <label class="inline-label">
                                            <input type="radio" name="view_state" value="50"> private
                                        </label>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="report_stay">Report Stay</label></th>
                                    <td class="input-col">
                                        <label class="inline-label">
                                            <input type="checkbox" id="report_stay" name="report_stay" value="1"{}>
                                            <span>Check to report more issues</span>
                                        </label>
                                    </td>
                                </tr>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox toolbox-split">
                        <button type="submit" class="btn btn-primary btn-white btn-round">
                            <i class="fa fa-check" aria-hidden="true"></i> Submit Issue
                        </button>
                        <span class="required">* required</span>
                    </div>
                </div>
            </div>
        </form>"#,
        err_banner,
        created_banner,
        proj_opts,
        cat_opts,
        ver_opts,
        target_ver_row,
        assign_to_row,
        cf_rows_html,
        dropzone_html,
        if report_stay { " checked" } else { "" }
    );

    render_layout("Report Issue", ctx, layout_flash, &content)
}

pub fn render_status_change_form(
    bug: &Bug,
    effective_access: i32,
    custom_fields: &[CustomField],
    cf_values: &[CustomFieldValue],
    versions: &[ProjectVersion],
    categories: &[Category],
) -> String {
    let mut status_opts = String::new();
    for (code, name) in [
        (10, "new"),
        (20, "feedback"),
        (30, "acknowledged"),
        (40, "confirmed"),
        (50, "assigned"),
        (80, "resolved"),
        (90, "closed"),
    ] {
        if code == 80 && effective_access < 55 {
            continue;
        }
        if code == 90 && effective_access < 70 {
            continue;
        }
        let sel = if bug.status == code { "selected" } else { "" };
        status_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            code, sel, name
        ));
    }

    let mut res_opts = String::new();
    for (code, name) in [
        (10, "open"),
        (20, "fixed"),
        (30, "reopened"),
        (40, "unable to reproduce"),
        (50, "not fixable"),
        (60, "duplicate"),
        (70, "not a bug"),
        (80, "suspended"),
        (90, "won't fix"),
    ] {
        let sel = if bug.resolution == code {
            "selected"
        } else {
            ""
        };
        res_opts.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            code, sel, name
        ));
    }

    let mut val_map = HashMap::new();
    for v in cf_values {
        val_map.insert(v.field_id, v.value.clone());
    }

    let mut cf_inputs = String::new();
    for cf in custom_fields {
        if effective_access < cf.access_level_r {
            continue;
        }
        let is_resolve_disp = cf.display_resolved != 0 && effective_access >= 55;
        let is_close_disp = cf.display_closed != 0 && effective_access >= 70;
        let qualifies = if bug.status < 80 {
            is_resolve_disp || is_close_disp
        } else if bug.status < 90 {
            is_close_disp
        } else {
            false
        };

        if qualifies {
            let is_req = (is_resolve_disp && cf.require_resolved != 0)
                || (is_close_disp && cf.require_closed != 0);
            let can_write = effective_access >= cf.access_level_rw;
            let current_val = val_map
                .get(&cf.id)
                .map(|s| s.as_str())
                .unwrap_or(cf.default_value.as_str());
            // Do not set HTML required attribute on the status form input: the status form handles
            // multiple transition targets (e.g. assigning or resolving), so client-side required
            // attributes would block intermediate non-resolve status changes. The server validates
            // requirements based on the chosen target status.
            let input_html = crate::views::custom_fields::render_custom_field_input(
                cf,
                current_val,
                false,
                !can_write,
                versions,
                categories,
            );
            let req_star = if is_req { " *" } else { "" };
            cf_inputs.push_str(&format!(
                r#"<label style="margin:0; font-size:12px; font-weight:bold; color:#555;">{}{}: {}</label>"#,
                html_escape(&cf.name),
                req_star,
                input_html
            ));
        }
    }

    format!(
        r#"<form method="POST" action="/bug_change_status/{}" style="display:inline-flex; gap:6px; align-items:center; flex-wrap:wrap;">
            <label style="margin:0; font-size:12px; font-weight:bold; color:#555;">Status: <select name="new_status" title="Status" style="width:auto; font-weight:normal;">{}</select></label>
            <label style="margin:0; font-size:12px; font-weight:bold; color:#555;">Resolution: <select name="resolution" title="Resolution" style="width:auto; font-weight:normal;">{}</select></label>
            <input type="text" name="note" placeholder="Optional status note" style="width:150px;">
            {}
            <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-exchange" aria-hidden="true"></i> Change Status</button>
        </form>"#,
        bug.id, status_opts, res_opts, cf_inputs
    )
}

pub fn render_dropzone_html(id_suffix: &str, prompt: &str) -> String {
    format!(
        r#"<div id="dropzone-{id}" style="border:2px dashed #bbb; border-radius:4px; padding:12px; text-align:center; background:#fafafa; cursor:pointer;">
            <input type="file" name="file" id="file-input-{id}" multiple style="display:none;">
            <p style="margin:0; font-size:13px; color:#555;">{prompt} <span style="color:#0052cc; text-decoration:underline;">browse</span></p>
            <div id="file-list-{id}" style="margin-top:6px; font-size:12px; color:#333;"></div>
        </div>
        <script>
        (function() {{
            var dz = document.getElementById('dropzone-{id}');
            var inp = document.getElementById('file-input-{id}');
            var fl = document.getElementById('file-list-{id}');
            if (!dz || !inp) return;
            dz.addEventListener('click', function(e) {{ if (e.target !== inp) inp.click(); }});
            dz.addEventListener('dragover', function(e) {{ e.preventDefault(); dz.style.borderColor = '#0052cc'; dz.style.background = '#f0f5ff'; }});
            dz.addEventListener('dragleave', function(e) {{ e.preventDefault(); dz.style.borderColor = '#bbb'; dz.style.background = '#fafafa'; }});
            dz.addEventListener('drop', function(e) {{
                e.preventDefault();
                dz.style.borderColor = '#bbb';
                dz.style.background = '#fafafa';
                if (e.dataTransfer && e.dataTransfer.files.length > 0) {{
                    inp.files = e.dataTransfer.files;
                    updateList();
                }}
            }});
            inp.addEventListener('change', updateList);
            function updateList() {{
                if (!fl) return;
                var files = inp.files;
                if (!files || files.length === 0) {{ fl.textContent = ''; return; }}
                var names = [];
                for (var i = 0; i < files.length; i++) {{
                    names.push(files[i].name + ' (' + Math.round(files[i].size / 1024) + ' KB)');
                }}
                fl.textContent = 'Selected: ' + names.join(', ');
            }}
        }})();
        </script>"#,
        id = id_suffix,
        prompt = prompt
    )
}

pub fn render_upload_form_html(bug_id: i64) -> String {
    let dropzone = render_dropzone_html("view", "Drag and drop files here to attach, or");
    format!(
        r#"<form method="POST" action="/bug_file_add/{}" enctype="multipart/form-data" style="margin-top:10px;">
            {}
            <div style="margin-top:8px;">
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-upload" aria-hidden="true"></i> Upload Files</button>
            </div>
        </form>"#,
        bug_id, dropzone
    )
}

pub fn render_monitors_html(monitors: &[User], bug_id: i64) -> String {
    if monitors.is_empty() {
        return "<p>No users monitoring this issue.</p>".to_string();
    }
    let mut out = String::from(r#"<div class="monitor-pill-list">"#);
    for m in monitors {
        let display_name = if !m.realname.trim().is_empty() {
            format!(
                "{} ({})",
                html_escape(&m.username),
                html_escape(&m.realname)
            )
        } else {
            html_escape(&m.username)
        };
        let remove_btn = format!(
            r#"<form method="POST" action="/bug_monitor_delete/{}" style="display:inline;">
                <input type="hidden" name="user_id" value="{}">
                <button type="submit" class="btn btn-danger btn-white btn-round btn-sm" title="Remove monitor" aria-label="Remove {}"><i class="fa fa-times" aria-hidden="true"></i></button>
            </form>"#,
            bug_id,
            m.id,
            html_escape(&m.username)
        );
        out.push_str(&format!(
            r#"<span class="monitor-pill"><i class="fa fa-user" aria-hidden="true"></i> <span>{}</span> {}</span>"#,
            display_name, remove_btn
        ));
    }
    out.push_str("</div>");
    out
}

pub fn render_monitor_form_html(bug_id: i64) -> String {
    format!(
        r#"<form method="POST" action="/bug_monitor_add/{}" style="margin-top:10px; display:flex; gap:8px; align-items:center; flex-wrap:wrap;">
            <input type="text" name="username" placeholder="Username" style="max-width:200px;" required>
            <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-user-plus" aria-hidden="true"></i> Add Monitor</button>
        </form>"#,
        bug_id
    )
}

pub fn render_file_preview_html(
    file_id: i64,
    filename: &str,
    is_safe_image: bool,
    text_content: Option<&str>,
) -> String {
    if is_safe_image {
        format!(
            r#"<div style="margin-top:6px;">
                <a href="/file/bug/{}/{}">
                    <img src="/file/bug/{}/{}?inline=1" alt="{}" style="max-width:150px; max-height:150px; object-fit:contain; border:1px solid #ddd; border-radius:4px; display:block;">
                </a>
            </div>"#,
            file_id,
            html_escape(filename),
            file_id,
            html_escape(filename),
            html_escape(filename)
        )
    } else if let Some(text) = text_content {
        format!(
            r#"<details style="margin-top:6px;">
                <summary style="cursor:pointer; color:#0052cc; font-size:12px;">Preview</summary>
                <pre style="background:#f8f9fa; border:1px solid #e9ecef; border-radius:4px; padding:8px; margin-top:4px; max-height:200px; overflow:auto; font-size:12px; font-family:monospace; white-space:pre-wrap; word-break:break-all;">{}</pre>
            </details>"#,
            html_escape(text)
        )
    } else {
        String::new()
    }
}

pub fn render_reminder_badge(note_type: i32) -> &'static str {
    if note_type == 1 {
        r#" <span class="badge" style="background:#17a2b8; color:#fff; font-size:10px; font-weight:bold; padding:2px 5px; border-radius:3px;">Reminder</span>"#
    } else {
        ""
    }
}

fn priority_icon_html(priority: i32) -> &'static str {
    match priority {
        60 => {
            r#"<i class="fa fa-exclamation-circle" style="color:#d15b47;" aria-hidden="true"></i> "#
        }
        50 => r#"<i class="fa fa-arrow-up" style="color:#d15b47;" aria-hidden="true"></i> "#,
        40 => r#"<i class="fa fa-chevron-up" style="color:#e67e22;" aria-hidden="true"></i> "#,
        30 => r#"<i class="fa fa-minus" style="color:#555555;" aria-hidden="true"></i> "#,
        20 => r#"<i class="fa fa-chevron-down" style="color:#3a87ad;" aria-hidden="true"></i> "#,
        10 => r#"<i class="fa fa-circle-o" style="color:#888888;" aria-hidden="true"></i> "#,
        _ => "",
    }
}

fn render_formatted_block(
    raw: &str,
    format_ctx: &crate::views::formatting::FormatContext,
) -> String {
    if format_ctx.enabled {
        crate::views::formatting::format_text(raw, format_ctx)
    } else {
        format!(
            r#"<div style="white-space:pre-wrap;">{}</div>"#,
            html_escape(raw)
        )
    }
}

fn render_collapsible_widget_box(
    widget_id: &str,
    anchor_id: Option<&str>,
    icon_class: &str,
    title: &str,
    extra_toolbar_html: &str,
    body_html: &str,
    toolbox_html: Option<&str>,
) -> String {
    let id_attr = anchor_id
        .map(|a| format!(r#" id="{}""#, html_escape(a)))
        .unwrap_or_default();
    let toolbox = toolbox_html
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!(r#"<div class="widget-toolbox action-toolbar">{}</div>"#, t))
        .unwrap_or_default();
    format!(
        r#"<div{id_attr} class="widget-box widget-color-blue2" data-widget-id="{wid}">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="ace-icon fa {icon}" aria-hidden="true"></i>
                    {title}
                </h4>
                <div class="widget-toolbar">
                    {extra_toolbar}
                    <button type="button" data-action="collapse" aria-expanded="true" aria-controls="widget-body-{wid}" aria-label="Toggle {title_attr}">
                        <i class="ace-icon fa fa-chevron-up" aria-hidden="true"></i>
                    </button>
                </div>
            </div>
            <div class="widget-body" id="widget-body-{wid}">
                {body}
                {toolbox}
            </div>
        </div>"#,
        id_attr = id_attr,
        wid = html_escape(widget_id),
        icon = html_escape(icon_class),
        title = html_escape(title),
        title_attr = html_escape(title),
        extra_toolbar = extra_toolbar_html,
        body = body_html,
        toolbox = toolbox
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_bug_action_toolbar(
    bug: &Bug,
    eff: i32,
    current_user_id: i64,
    handler_name: &str,
    users: &[User],
    is_monitoring: bool,
    can_clone: bool,
    can_move: bool,
    custom_fields: &[CustomField],
    cf_values: &[CustomFieldValue],
    versions: &[ProjectVersion],
    cats: &[Category],
) -> String {
    let mut actions_html = String::new();
    let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == current_user_id);
    if can_edit {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-primary btn-white btn-round btn-sm" href="/update_bug_page/{}"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>"#,
            bug.id
        ));
    }

    if eff >= 55 {
        let mut user_opts = String::from(r#"<option value="0">(Unassigned)</option>"#);
        let mut found_handler = bug.handler_id <= 0;
        for u in users {
            let sel = if u.id == bug.handler_id {
                found_handler = true;
                "selected"
            } else {
                ""
            };
            user_opts.push_str(&format!(
                r#"<option value="{}" {}>{}</option>"#,
                u.id,
                sel,
                html_escape(&u.username)
            ));
        }
        if !found_handler {
            user_opts.push_str(&format!(
                r#"<option value="{}" selected>{}</option>"#,
                bug.handler_id,
                html_escape(handler_name)
            ));
        }
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_assign/{}" class="inline-flex-form">
                <select name="handler_id" class="select-auto">{}</select>
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-user" aria-hidden="true"></i> Assign To</button>
            </form>"#,
            bug.id, user_opts
        ));
    }

    if eff >= 40 {
        let status_form =
            render_status_change_form(bug, eff, custom_fields, cf_values, versions, cats);
        actions_html.push_str(&status_form);
    }

    if is_monitoring {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_monitor_delete/{}" class="inline-form">
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-eye-slash" aria-hidden="true"></i> End Monitoring</button>
            </form>"#,
            bug.id
        ));
    } else {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_monitor_add/{}" class="inline-form">
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-eye" aria-hidden="true"></i> Monitor</button>
            </form>"#,
            bug.id
        ));
    }

    if eff >= 70 {
        let (stick_label, stick_cls) = if bug.sticky == 1 {
            ("Unstick", "btn btn-warning btn-white btn-round btn-sm")
        } else {
            ("Stick", "btn btn-white btn-round btn-sm")
        };
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_stick/{}" class="inline-form">
                <button type="submit" class="{}"><i class="fa fa-thumb-tack" aria-hidden="true"></i> {}</button>
            </form>"#,
            bug.id, stick_cls, stick_label
        ));
    }

    if can_clone {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-white btn-round btn-sm" href="/bug_clone_page/{}"><i class="fa fa-clone" aria-hidden="true"></i> Clone</a>"#,
            bug.id
        ));
    }

    if can_move {
        actions_html.push_str(&format!(
            r#"<a class="btn btn-white btn-round btn-sm" href="/bug_move_page/{}"><i class="fa fa-arrows" aria-hidden="true"></i> Move</a>"#,
            bug.id
        ));
    }

    if eff >= 70 && bug.status < 90 {
        let close_res = if bug.resolution > 10 {
            bug.resolution
        } else {
            20
        };
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_change_status/{}" class="inline-form" onsubmit="return confirm('Close this issue?');">
                <input type="hidden" name="new_status" value="90">
                <input type="hidden" name="resolution" value="{}">
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-check-square-o" aria-hidden="true"></i> Close</button>
            </form>"#,
            bug.id, close_res
        ));
    }

    actions_html.push_str(&format!(
        r#"<a class="btn btn-white btn-round btn-sm" href="/bug_revision_view_page/{}"><i class="fa fa-history" aria-hidden="true"></i> Revisions</a>"#,
        bug.id
    ));

    if eff >= 70 {
        actions_html.push_str(&format!(
            r#"<form method="POST" action="/bug_delete/{}" class="inline-form" onsubmit="return confirm('Delete this issue? This action cannot be undone.');">
                <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete Issue</button>
            </form>"#,
            bug.id
        ));
    }

    actions_html
}

pub fn render_bugnote_add_form(bug_id: i64, eff: i32) -> String {
    if eff < 25 {
        return String::new();
    }
    let priv_opt = if eff >= 55 {
        r#"<option value="50">private</option>"#
    } else {
        ""
    };
    let time_row = if eff >= 55 {
        r#"<tr>
                <th class="category">Time Tracking (HH:MM or mins)</th>
                <td class="input-col"><input type="text" name="time_tracking" placeholder="00:00 or minutes" class="input-w-160"></td>
            </tr>"#
    } else {
        ""
    };
    format!(
        r#"<form method="POST" action="/bugnote_add/{}">
                <div class="table-responsive">
                    <table class="form-table form-table-left">
                        <tr>
                            <th class="category">Note</th>
                            <td class="input-col"><textarea name="note" rows="5" required></textarea></td>
                        </tr>
                        <tr>
                            <th class="category">View Status</th>
                            <td class="input-col">
                                <select name="view_state" class="select-auto">
                                    <option value="10">public</option>
                                    {}
                                </select>
                            </td>
                        </tr>
                        {}
                    </table>
                </div>
                <div class="widget-toolbox mt-8">
                    <button type="submit" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-comment" aria-hidden="true"></i> Add Note</button>
                </div>
            </form>"#,
        bug_id, priv_opt, time_row
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_view_bug_page(
    ctx: &AuthContext,
    full: &BugFull,
    proj_name: &str,
    cat_name: &str,
    reporter_name: &str,
    handler_name: &str,
    eff: i32,
    cf_values: &[CustomFieldValue],
    actions_html: &str,
    tags_html: &str,
    rels_html: &str,
    rel_form_html: &str,
    files_html: &str,
    upload_form_html: &str,
    notes_html: &str,
    note_form_html: &str,
    monitors_html: &str,
    monitor_form_html: &str,
    history_html: &str,
    jira_link: Option<&JiraLink>,
    jira_section_html: &str,
    flash_msg: Option<&str>,
    flash_err: Option<&str>,
    format_ctx: &crate::views::formatting::FormatContext,
) -> Html<String> {
    let bug = &full.bug;
    // Note: flash_msg is rendered once by render_layout; only render flash_err here.
    let err_banner = if let Some(e) = flash_err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let jira_badge = if let Some(jl) = jira_link {
        format!(
            r#"<a href="{}" target="_blank" class="badge badge-jira">Jira: {} &nearr;</a>"#,
            html_escape(&jl.jira_url),
            html_escape(&jl.jira_key)
        )
    } else {
        String::new()
    };

    let mut cf_rows_html = String::new();
    for cfv in cf_values {
        let is_multi = matches!(
            crate::models::CustomFieldType::from_i32(cfv.field_type),
            Some(crate::models::CustomFieldType::Checkbox)
                | Some(crate::models::CustomFieldType::Multilist)
        );
        let display_value = if is_multi && cfv.value.contains('|') {
            cfv.value.replace('|', ", ")
        } else {
            cfv.value.clone()
        };
        cf_rows_html.push_str(&format!(
            r#"<tr>
                <th class="category">{}</th>
                <td class="input-col" colspan="5">{}</td>
            </tr>"#,
            html_escape(&cfv.field_name),
            html_escape(&display_value)
        ));
    }

    let view_state_str = ViewState::from_i32(bug.view_state)
        .map(|v| v.as_str())
        .unwrap_or("public");

    let sticky_badge = if bug.sticky == 1 {
        r#" <span class="badge badge-sticky">[Sticky]</span>"#
    } else {
        ""
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let due_date_html = if bug.due_date > 0 {
        let d = format_date(bug.due_date);
        let overdue = if bug.status < 80 && bug.due_date < now {
            r#" <span class="badge badge-danger badge-overdue">Overdue</span>"#
        } else {
            ""
        };
        format!("{}{}", html_escape(&d), overdue)
    } else {
        "-".to_string()
    };
    let build_str = if bug.build.trim().is_empty() {
        "-".to_string()
    } else {
        html_escape(&bug.build)
    };

    let status_badge = render_status_badge(bug.status, status_label(bug.status));
    let priority_html = format!(
        "{}{}",
        priority_icon_html(bug.priority),
        html_escape(priority_label(bug.priority))
    );

    let mut jump_toolbar = String::new();
    if eff >= crate::models::ACCESS_DEVELOPER {
        jump_toolbar.push_str(&format!(
            r#"<a class="btn btn-white btn-round btn-sm" href="/bug_reminder_page/{}"><i class="fa fa-bell" aria-hidden="true"></i> Send a Reminder</a>"#,
            bug.id
        ));
    }
    jump_toolbar.push_str(
        r##"<a class="btn btn-white btn-round btn-sm" href="#bugnotes"><i class="fa fa-comments" aria-hidden="true"></i> Jump to Notes</a>
        <a class="btn btn-white btn-round btn-sm" href="#history"><i class="fa fa-history" aria-hidden="true"></i> Jump to History</a>"##,
    );

    let details_body = format!(
        r#"<div class="table-responsive">
            <table class="form-table form-table-left bug-view-table mb-0">
                <thead>
                    <tr class="bug-header">
                        <th class="category">ID</th>
                        <th class="category">Project</th>
                        <th class="category">Category</th>
                        <th class="category">View Status</th>
                        <th class="category">Date Submitted</th>
                        <th class="category">Last Update</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td>{:07}{}{}</td>
                        <td>{}</td>
                        <td>{}</td>
                        <td>{}</td>
                        <td>{}</td>
                        <td>{}</td>
                    </tr>
                    <tr class="spacer-row"><td colspan="6"></td></tr>
                    <tr>
                        <th class="category">Reporter</th>
                        <td class="input-col">{}</td>
                        <th class="category">Assigned To</th>
                        <td class="input-col">{}</td>
                        <th class="category">Due Date</th>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Priority</th>
                        <td class="input-col">{}</td>
                        <th class="category">Severity</th>
                        <td class="input-col">{}</td>
                        <th class="category">Reproducibility</th>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Status</th>
                        <td class="input-col">{}</td>
                        <th class="category">Resolution</th>
                        <td class="input-col" colspan="3">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Platform</th>
                        <td class="input-col">{}</td>
                        <th class="category">OS</th>
                        <td class="input-col">{}</td>
                        <th class="category">OS Version</th>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Product Version</th>
                        <td class="input-col">{}</td>
                        <th class="category">Target Version</th>
                        <td class="input-col">{}</td>
                        <th class="category">Fixed in Version</th>
                        <td class="input-col">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Product Build</th>
                        <td class="input-col" colspan="5">{}</td>
                    </tr>
                    <tr class="spacer-row"><td colspan="6"></td></tr>
                    <tr>
                        <th class="category">Summary</th>
                        <td class="input-col" colspan="5"><strong>{:07}: {}</strong>{}</td>
                    </tr>
                    <tr>
                        <th class="category">Description</th>
                        <td class="input-col" colspan="5">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Steps To Reproduce</th>
                        <td class="input-col" colspan="5">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Additional Information</th>
                        <td class="input-col" colspan="5">{}</td>
                    </tr>
                    <tr>
                        <th class="category">Tags</th>
                        <td class="input-col" colspan="5">{}</td>
                    </tr>
                    {}
                </tbody>
            </table>
        </div>"#,
        bug.id,
        jira_badge,
        sticky_badge,
        html_escape(proj_name),
        html_escape(cat_name),
        html_escape(view_state_str),
        format_timestamp(bug.date_submitted),
        format_timestamp(bug.last_updated),
        html_escape(reporter_name),
        html_escape(handler_name),
        due_date_html,
        priority_html,
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
        build_str,
        bug.id,
        html_escape(&bug.summary),
        sticky_badge,
        render_formatted_block(&full.description, format_ctx),
        render_formatted_block(&full.steps_to_reproduce, format_ctx),
        render_formatted_block(&full.additional_information, format_ctx),
        tags_html,
        cf_rows_html
    );

    let details_box = render_collapsible_widget_box(
        "view_bug_details",
        None,
        "fa-bars",
        &format!("View Issue Details [#{:07}]", bug.id),
        &jump_toolbar,
        &details_body,
        Some(actions_html),
    );

    let files_box = render_collapsible_widget_box(
        "attachments",
        Some("attachments"),
        "fa-paperclip",
        "Attached Files",
        "",
        &format!(
            r#"<div class="table-responsive">{}{}</div>"#,
            files_html, upload_form_html
        ),
        None,
    );

    let rels_box = render_collapsible_widget_box(
        "relationships",
        Some("relationships"),
        "fa-sitemap",
        "Relationships",
        "",
        &format!(
            r#"<div class="table-responsive">{}{}</div>"#,
            rels_html, rel_form_html
        ),
        None,
    );

    let monitors_box = if !monitors_html.is_empty() || !monitor_form_html.is_empty() {
        render_collapsible_widget_box(
            "monitors",
            Some("monitors"),
            "fa-users",
            "Users Monitoring This Issue",
            "",
            &format!("{}{}", monitors_html, monitor_form_html),
            None,
        )
    } else {
        String::new()
    };

    let notes_box = render_collapsible_widget_box(
        "bugnotes",
        Some("bugnotes"),
        "fa-comments",
        "Activities / Bugnotes",
        "",
        notes_html,
        None,
    );

    let add_note_box = if !note_form_html.trim().is_empty() {
        render_collapsible_widget_box(
            "bugnote_add",
            Some("addbugnote"),
            "fa-comment",
            "Add Note",
            "",
            note_form_html,
            None,
        )
    } else {
        String::new()
    };

    let history_box = render_collapsible_widget_box(
        "history",
        Some("history"),
        "fa-history",
        "Issue History",
        "",
        &format!(r#"<div class="table-responsive">{}</div>"#, history_html),
        None,
    );

    let content = format!(
        r#"{}
        {}
        {}
        {}
        {}
        {}
        {}
        {}
        {}"#,
        err_banner,
        details_box,
        files_box,
        rels_box,
        monitors_box,
        jira_section_html,
        notes_box,
        add_note_box,
        history_box
    );

    render_layout(&format!("Issue #{:07}", bug.id), ctx, flash_msg, &content)
}

fn render_version_select_options(
    versions: &[ProjectVersion],
    current_value: &str,
    unreleased_only: bool,
) -> String {
    let mut out = format!(
        r#"<option value="" {}>(none)</option>"#,
        if current_value.is_empty() {
            "selected"
        } else {
            ""
        }
    );
    let mut found_current = current_value.is_empty();
    for v in versions {
        let is_current = v.version == current_value;
        if !is_current && (v.obsolete != 0 || (unreleased_only && v.released != 0)) {
            continue;
        }
        let sel = if is_current {
            found_current = true;
            "selected"
        } else {
            ""
        };
        let escaped = html_escape(&v.version);
        out.push_str(&format!(
            r#"<option value="{0}" {1}>{0}</option>"#,
            escaped, sel
        ));
    }
    if !found_current {
        let escaped = html_escape(current_value);
        out.push_str(&format!(
            r#"<option value="{0}" selected>{0}</option>"#,
            escaped
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn render_update_bug_page(
    ctx: &AuthContext,
    full: &BugFull,
    cat_opts: &str,
    custom_fields: &[CustomField],
    cf_values: &[CustomFieldValue],
    effective_access: i32,
    versions: &[ProjectVersion],
    categories: &[Category],
) -> Html<String> {
    let bug = &full.bug;
    let mut val_map = HashMap::new();
    for v in cf_values {
        val_map.insert(v.field_id, v.value.clone());
    }
    let cf_rows_html = crate::views::custom_fields::render_custom_fields_form_rows(
        custom_fields,
        &val_map,
        effective_access,
        crate::views::custom_fields::CustomFieldFormMode::Update,
        versions,
        categories,
    );
    let product_ver_opts = render_version_select_options(versions, &bug.version, false);
    let fixed_ver_opts = render_version_select_options(versions, &bug.fixed_in_version, false);
    let target_ver_opts = render_version_select_options(versions, &bug.target_version, true);

    let content = format!(
        r#"<form method="POST" action="/update_bug/{}">
            <input type="hidden" name="last_updated" value="{}">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-pencil" aria-hidden="true"></i>
                        Update Issue #{:07}
                    </h4>
                    <div class="widget-toolbar">
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back To Issue</a>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category"><label for="category_id">Category</label></th>
                                    <td class="input-col"><select id="category_id" name="category_id">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="priority">Priority</label></th>
                                    <td class="input-col">
                                        <select id="priority" name="priority">
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
                                    <th class="category"><label for="severity">Severity</label></th>
                                    <td class="input-col">
                                        <select id="severity" name="severity">
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
                                    <th class="category"><label for="reproducibility">Reproducibility</label></th>
                                    <td class="input-col">
                                        <select id="reproducibility" name="reproducibility">
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
                                    <th class="category"><label for="version">Product Version</label></th>
                                    <td class="input-col"><select id="version" name="version">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="fixed_in_version">Fixed in Version</label></th>
                                    <td class="input-col"><select id="fixed_in_version" name="fixed_in_version">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="target_version">Target Version</label></th>
                                    <td class="input-col"><select id="target_version" name="target_version">{}</select></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="platform">Platform</label></th>
                                    <td class="input-col"><input type="text" id="platform" name="platform" value="{}"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="os">OS</label></th>
                                    <td class="input-col"><input type="text" id="os" name="os" value="{}"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="os_build">OS Version</label></th>
                                    <td class="input-col"><input type="text" id="os_build" name="os_build" value="{}"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="build">Product Build</label></th>
                                    <td class="input-col"><input type="text" id="build" name="build" value="{}"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="due_date">Due Date</label></th>
                                    <td class="input-col"><input type="date" id="due_date" name="due_date" value="{}"></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="view_state">View Status</label></th>
                                    <td class="input-col">
                                        <select id="view_state" name="view_state">
                                            <option value="10" {}>public</option>
                                            <option value="50" {}>private</option>
                                        </select>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="summary">Summary</label></th>
                                    <td class="input-col"><input type="text" id="summary" name="summary" value="{}" required></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="description">Description</label></th>
                                    <td class="input-col"><textarea id="description" name="description" rows="6">{}</textarea></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="steps_to_reproduce">Steps To Reproduce</label></th>
                                    <td class="input-col"><textarea id="steps_to_reproduce" name="steps_to_reproduce" rows="4">{}</textarea></td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="additional_information">Additional Information</label></th>
                                    <td class="input-col"><textarea id="additional_information" name="additional_information" rows="4">{}</textarea></td>
                                </tr>
                                {}
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox toolbox-split">
                        <div style="display:flex; gap:8px; align-items:center;">
                            <button type="submit" class="btn btn-primary btn-white btn-round"><i class="fa fa-check" aria-hidden="true"></i> Update Information</button>
                            <a href="/view_bug_page/{}" class="btn btn-white btn-round">Back To Issue</a>
                        </div>
                        <span class="required">* required</span>
                    </div>
                </div>
            </div>
        </form>"#,
        bug.id,
        bug.last_updated,
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
        product_ver_opts,
        fixed_ver_opts,
        target_ver_opts,
        html_escape(&bug.platform),
        html_escape(&bug.os),
        html_escape(&bug.os_build),
        html_escape(&bug.build),
        html_escape(&format_date(bug.due_date)),
        if bug.view_state == 10 { "selected" } else { "" },
        if bug.view_state == 50 { "selected" } else { "" },
        html_escape(&bug.summary),
        html_escape(&full.description),
        html_escape(&full.steps_to_reproduce),
        html_escape(&full.additional_information),
        cf_rows_html,
        bug.id
    );

    render_layout(&format!("Update Issue #{:07}", bug.id), ctx, None, &content)
}

pub fn render_bug_tags_html(
    bug_id: i64,
    bug_tags: &[BugTag],
    eff: i32,
    current_user_id: i64,
) -> String {
    let mut tags_html = String::new();
    if bug_tags.is_empty() {
        tags_html.push_str("<span>No tags attached.</span>");
    } else {
        for bt in bug_tags {
            let can_detach = eff >= 55 || (eff >= 25 && bt.user_id == current_user_id);
            let detach_form = if can_detach {
                format!(
                    r#" <form method="POST" action="/bug_tag_detach/{}/{}" class="inline-form"><button type="submit" class="btn btn-danger btn-sm">x</button></form>"#,
                    bug_id, bt.tag_id
                )
            } else {
                String::new()
            };
            tags_html.push_str(&format!(
                r#"<span class="badge badge-success badge-tag"><a href="/view_all_bug_page?tag_id={}">{}</a>{}</span>"#,
                bt.tag_id,
                html_escape(&bt.tag_name),
                detach_form
            ));
        }
    }
    if eff >= 25 {
        tags_html.push_str(&format!(
            r#"<form method="POST" action="/bug_tag_attach/{}" class="inline-flex-row mt-8">
                <input type="text" name="tag_string" placeholder="Attach tags (comma-separated)" class="input-w-240">
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-tag" aria-hidden="true"></i> Attach Tags</button>
            </form>"#,
            bug_id
        ));
    }
    tags_html
}

pub fn render_bug_files_html(files_with_previews: &[(&BugFile, String)], eff: i32) -> String {
    if files_with_previews.is_empty() {
        return "<p>No attachments.</p>".to_string();
    }
    let mut files_html = String::from(
        r#"<table class="data-table"><thead><tr><th>Filename</th><th>Size</th><th>Type</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for (f, preview_html) in files_with_previews {
        let del_btn = if eff >= 70 {
            format!(
                r#"<form method="POST" action="/bug_file_delete/{}" class="inline-form"><button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button></form>"#,
                f.id
            )
        } else {
            String::new()
        };
        files_html.push_str(&format!(
            r#"<tr>
                    <td>
                        <a href="/file/bug/{}/{}">{}</a>
                        {}
                    </td>
                    <td>{} bytes</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
            f.id,
            html_escape(&f.filename),
            html_escape(&f.filename),
            preview_html,
            f.filesize,
            html_escape(&f.file_type),
            del_btn
        ));
    }
    files_html.push_str("</tbody></table>");
    files_html
}

pub fn render_bug_relationships_html(rels: &[BugRelationship], eff: i32) -> String {
    if rels.is_empty() {
        return "<p>No relationships.</p>".to_string();
    }
    let mut rels_html = String::from(
        r#"<table class="data-table"><thead><tr><th>Relationship</th><th>Target Issue</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for r in rels {
        let del_btn = if eff >= 55 {
            format!(
                r#"<form method="POST" action="/bug_relationship_delete/{}" class="inline-form"><button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button></form>"#,
                r.id
            )
        } else {
            String::new()
        };
        rels_html.push_str(&format!(
            r#"<tr>
                    <td>{}</td>
                    <td><a href="/view_bug_page/{}">{:07}</a></td>
                    <td>{}</td>
                </tr>"#,
            html_escape(relationship_label(r.relationship_type)),
            r.destination_bug_id,
            r.destination_bug_id,
            del_btn
        ));
    }
    rels_html.push_str("</tbody></table>");
    rels_html
}

pub fn render_bug_relationship_add_form(bug_id: i64, eff: i32) -> String {
    if eff < 55 {
        return String::new();
    }
    format!(
        r#"<form method="POST" action="/bug_relationship_add/{}" class="inline-flex-row mt-10">
                <select name="relationship_type" class="select-auto">
                    <option value="0">related to</option>
                    <option value="1">duplicate of</option>
                    <option value="2">has duplicate</option>
                    <option value="3">blocks</option>
                    <option value="4">blocked by</option>
                    <option value="5">parent of</option>
                    <option value="6">child of</option>
                </select>
                <input type="text" name="dest_bug_id" placeholder="Target Bug ID" class="input-w-140" required>
                <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-plus" aria-hidden="true"></i> Add Relationship</button>
            </form>"#,
        bug_id
    )
}

pub fn render_bugnotes_section_html(
    notes: &[Bugnote],
    users_map: &HashMap<i64, String>,
    users_access_map: &HashMap<i64, i32>,
    eff: i32,
    current_user_id: i64,
    format_ctx: &crate::views::formatting::FormatContext,
) -> String {
    let total_time_minutes: i32 = notes.iter().map(|n| n.time_tracking.max(0)).sum();
    let mut html = if total_time_minutes > 0 {
        format!(
            r#"<p class="mb-8"><strong>Total Time Spent:</strong> {:02}:{:02} ({} mins)</p>"#,
            total_time_minutes / 60,
            total_time_minutes % 60,
            total_time_minutes
        )
    } else {
        String::new()
    };
    html.push_str(&render_bugnotes_cards(
        notes,
        users_map,
        users_access_map,
        eff,
        current_user_id,
        format_ctx,
    ));
    html
}

pub fn render_bugnotes_cards(
    notes: &[Bugnote],
    users_map: &HashMap<i64, String>,
    users_access_map: &HashMap<i64, i32>,
    eff: i32,
    current_user_id: i64,
    format_ctx: &crate::views::formatting::FormatContext,
) -> String {
    if notes.is_empty() {
        return "<p>No notes attached to this issue.</p>".to_string();
    }
    let mut html =
        String::from(r#"<div class="table-responsive"><table class="bugnote-timeline"><tbody>"#);
    for n in notes {
        let author = users_map
            .get(&n.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", n.reporter_id));
        let access_badge = users_access_map
            .get(&n.reporter_id)
            .and_then(|&lvl| crate::models::AccessLevel::label_for_level(lvl))
            .map(|lbl| format!(r#" <span class="badge badge-info">{}</span>"#, lbl))
            .unwrap_or_default();

        let private_class = if n.view_state == 50 {
            " bugnote-private"
        } else {
            ""
        };
        let private_badge = if n.view_state == 50 {
            r#" <span class="badge badge-warning"><i class="fa fa-lock" aria-hidden="true"></i> private</span>"#
        } else {
            ""
        };
        let time_badge = if n.time_tracking > 0 {
            format!(
                r#" <span class="badge badge-info"><i class="fa fa-clock-o" aria-hidden="true"></i> {:02}:{:02}</span>"#,
                n.time_tracking / 60,
                n.time_tracking % 60
            )
        } else {
            String::new()
        };

        let can_edit_note = eff >= 70 || (eff >= 25 && n.reporter_id == current_user_id);
        let can_del_note = can_edit_note;
        let can_toggle_vs = eff >= 55;

        let mut actions = Vec::new();

        if can_edit_note {
            actions.push(format!(
                r#"<a href="/bugnote_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>"#,
                n.id
            ));
        }

        if can_toggle_vs {
            let (btn_label, next_vs, lock_icon) = if n.view_state == 50 {
                ("Make Public", 10, "fa-unlock")
            } else {
                ("Make Private", 50, "fa-lock")
            };
            actions.push(format!(
                r#"<form method="POST" action="/bugnote_set_view_state/{}" class="inline-form"><input type="hidden" name="view_state" value="{}"><button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa {}" aria-hidden="true"></i> {}</button></form>"#,
                n.id, next_vs, lock_icon, btn_label
            ));
        }

        actions.push(format!(
            r#"<a href="/bug_revision_view_page/{}?bugnote_id={}" class="btn btn-white btn-round btn-sm"><i class="fa fa-history" aria-hidden="true"></i> Revisions</a>"#,
            n.bug_id, n.id
        ));

        if can_del_note {
            actions.push(format!(
                r#"<form method="POST" action="/bugnote_delete/{}" class="inline-form" onsubmit="return confirm('Delete this note?');"><button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button></form>"#,
                n.id
            ));
        }

        let actions_html = actions.join(" ");
        let reminder_badge = render_reminder_badge(n.note_type);
        let formatted_note = render_formatted_block(&n.note, format_ctx);

        html.push_str(&format!(
            r#"<tr id="c{nid}" class="bugnote-card{priv_cls}">
                <td class="bugnote-meta">
                    <div class="bugnote-meta-header">
                        <span class="bugnote-avatar"><i class="fa fa-user" aria-hidden="true"></i></span>
                        <strong>{author}</strong>{access_badge}
                    </div>
                    <div class="bugnote-meta-time">
                        <i class="fa fa-clock-o" aria-hidden="true"></i> {ts}
                    </div>
                    <div class="bugnote-meta-links">
                        <a rel="bookmark" href="/view_bug_page/{bid}#c{nid}" title="Direct link to note">
                            <i class="fa fa-link" aria-hidden="true"></i> ~{nid:07}
                        </a>
                        {reminder_badge}{private_badge}{time_badge}
                    </div>
                    <div class="bugnote-actions">
                        {actions}
                    </div>
                </td>
                <td class="bugnote-content">
                    {body}
                </td>
            </tr>"#,
            nid = n.id,
            bid = n.bug_id,
            priv_cls = private_class,
            author = html_escape(&author),
            access_badge = access_badge,
            ts = format_timestamp(n.date_submitted),
            reminder_badge = reminder_badge,
            private_badge = private_badge,
            time_badge = time_badge,
            actions = actions_html,
            body = formatted_note
        ));
    }
    html.push_str("</tbody></table></div>");
    html
}

pub fn render_bugnote_edit_page(
    ctx: &AuthContext,
    bug: &Bug,
    note: &Bugnote,
    flash_err: Option<&str>,
) -> Html<String> {
    let banner = if let Some(e) = flash_err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let content = format!(
        r#"{}
        <form method="POST" action="/bugnote_update/{}">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-pencil" aria-hidden="true"></i>
                        Edit Bugnote #{:07} (Issue #{:07})
                    </h4>
                    <div class="widget-toolbar">
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back To Issue</a>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="note">Bugnote</label></th>
                                    <td class="input-col"><textarea id="note" name="note" rows="8" style="width:100%;" required>{}</textarea></td>
                                </tr>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox" style="display:flex; gap:8px; align-items:center;">
                        <button type="submit" class="btn btn-primary btn-white btn-round"><i class="fa fa-check" aria-hidden="true"></i> Update Information</button>
                        <a href="/view_bug_page/{}" class="btn btn-white btn-round">Cancel</a>
                    </div>
                </div>
            </div>
        </form>"#,
        banner,
        note.id,
        note.id,
        bug.id,
        bug.id,
        html_escape(&note.note),
        bug.id
    );

    render_layout(
        &format!("Edit Bugnote #{:07}", note.id),
        ctx,
        None,
        &content,
    )
}

pub fn render_bug_reminder_page(
    ctx: &AuthContext,
    bug: &Bug,
    eligible_users: &[User],
    flash_err: Option<&str>,
    flash_msg: Option<&str>,
) -> Html<String> {
    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut user_options = String::new();
    for u in eligible_users {
        let display = if !u.realname.trim().is_empty() {
            format!("{} ({})", u.username, u.realname)
        } else {
            u.username.clone()
        };
        user_options.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            u.id,
            html_escape(&display)
        ));
    }

    let content = format!(
        r#"{}
        <form method="POST" action="/bug_reminder/{}">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-bell" aria-hidden="true"></i>
                        Send a Reminder for Issue #{:07}: {}
                    </h4>
                    <div class="widget-toolbar">
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back To Issue</a>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="to">Recipient(s)</label></th>
                                    <td class="input-col">
                                        <select id="to" name="to" multiple size="8" style="min-width:280px; width:100%;" required>
                                            {}
                                        </select>
                                        <span style="font-size:11px; color:#666; display:block; margin-top:4px;">Hold Ctrl/Cmd to select multiple recipients.</span>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="body">Reminder Message</label></th>
                                    <td class="input-col">
                                        <textarea id="body" name="body" rows="6" style="width:100%;" required placeholder="Enter reminder message here..."></textarea>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Store as Note</th>
                                    <td class="input-col">
                                        <label class="inline-label">
                                            <input type="hidden" name="store_as_note" value="0">
                                            <input type="checkbox" name="store_as_note" value="1" checked>
                                            <span>Store reminder as an issue note</span>
                                        </label>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category"><label for="view_state">View Status</label></th>
                                    <td class="input-col">
                                        <select id="view_state" name="view_state" style="width:auto;">
                                            <option value="10" selected>public</option>
                                            <option value="50">private</option>
                                        </select>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Add to Monitors</th>
                                    <td class="input-col">
                                        <label class="inline-label">
                                            <input type="hidden" name="add_monitors" value="0">
                                            <input type="checkbox" name="add_monitors" value="1" checked>
                                            <span>Add recipients to monitoring list</span>
                                        </label>
                                    </td>
                                </tr>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox" style="display:flex; gap:8px; align-items:center;">
                        <button type="submit" class="btn btn-primary btn-white btn-round"><i class="fa fa-bell" aria-hidden="true"></i> Send Reminder</button>
                        <a href="/view_bug_page/{}" class="btn btn-white btn-round">Cancel</a>
                    </div>
                </div>
            </div>
        </form>"#,
        err_banner,
        bug.id,
        bug.id,
        html_escape(&bug.summary),
        bug.id,
        user_options,
        bug.id
    );

    render_layout(
        &format!("Send Reminder for Issue #{:07}", bug.id),
        ctx,
        flash_msg,
        &content,
    )
}

pub fn render_bug_revision_view_page(
    ctx: &AuthContext,
    bug: &Bug,
    revisions: &[BugRevision],
    users_map: &HashMap<i64, String>,
    can_drop: bool,
    bugnote_id: Option<i64>,
    flash_err: Option<&str>,
) -> Html<String> {
    let banner = if let Some(e) = flash_err {
        format!(r#"<div class="flash-error">{}</div>"#, html_escape(e))
    } else {
        String::new()
    };

    let title_suffix = if let Some(nid) = bugnote_id {
        format!("for Bugnote #{:07} (Issue #{:07})", nid, bug.id)
    } else {
        format!("for Issue #{:07}", bug.id)
    };

    let revs_html = if revisions.is_empty() {
        "<p>No revisions found.</p>".to_string()
    } else {
        let mut table = String::from(
            r#"<div class="table-responsive"><table class="data-table"><thead><tr><th>Timestamp</th><th>Username</th><th>Revision Type</th><th>Previous Content</th>"#,
        );
        if can_drop {
            table.push_str("<th>Actions</th>");
        }
        table.push_str("</tr></thead><tbody>");

        for r in revisions {
            let author = users_map
                .get(&r.user_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", r.user_id));

            let type_label = r.type_label();

            let drop_btn = if can_drop {
                format!(
                    r#"<td style="width:140px;"><form method="POST" action="/bug_revision_drop/{}" style="display:inline;" onsubmit="return confirm('Are you sure you want to drop this revision?');"><button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Drop Revision</button></form></td>"#,
                    r.id
                )
            } else {
                String::new()
            };

            table.push_str(&format!(
                r#"<tr>
                    <td style="width:160px; white-space:nowrap;">{}</td>
                    <td style="width:140px;">{}</td>
                    <td style="width:160px;"><span class="badge badge-info">{}</span></td>
                    <td style="white-space:pre-wrap; word-break:break-word;">{}</td>
                    {}
                </tr>"#,
                format_timestamp(r.timestamp),
                html_escape(&author),
                html_escape(type_label),
                html_escape(&r.value),
                drop_btn
            ));
        }

        table.push_str("</tbody></table></div>");
        table
    };

    let content = format!(
        r#"{}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="ace-icon fa fa-history" aria-hidden="true"></i>
                    Revisions {}
                </h4>
                <div class="widget-toolbar">
                    <a href="/view_bug_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back to Issue</a>
                </div>
            </div>
            <div class="widget-body">
                {}
            </div>
        </div>"#,
        banner, title_suffix, bug.id, revs_html
    );

    render_layout(&format!("Revisions {}", title_suffix), ctx, None, &content)
}

pub fn render_conflict_page(ctx: &AuthContext, bug_id: i64, message: &str) -> Html<String> {
    let content = format!(
        r#"<div class="flash-error" style="margin-bottom:16px;"><strong>Conflict:</strong> {}</div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="ace-icon fa fa-exclamation-triangle" aria-hidden="true"></i>
                    Edit Conflict - Issue #{:07}
                </h4>
            </div>
            <div class="widget-body">
                <p>This issue has been modified by another user or session since you loaded the update page. Please reload the issue to review the latest changes and re-apply your updates.</p>
            </div>
            <div class="widget-toolbox" style="display:flex; gap:8px; flex-wrap:wrap;">
                <a href="/view_bug_page/{}" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-refresh" aria-hidden="true"></i> Reload Issue Details</a>
                <a href="/update_bug_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Return to Edit Page</a>
            </div>
        </div>"#,
        html_escape(message),
        bug_id,
        bug_id,
        bug_id,
    );

    render_layout(
        &format!("Conflict - Issue {:07}", bug_id),
        ctx,
        None,
        &content,
    )
}

pub fn render_bug_clone_page(
    ctx: &AuthContext,
    full: &BugFull,
    source_proj_name: &str,
    target_projects: &[(i64, String, bool)],
    flash_err: Option<&str>,
    flash_msg: Option<&str>,
) -> Html<String> {
    let bug = &full.bug;
    let mut proj_options = String::new();
    for (pid, pname, is_sel) in target_projects {
        let sel = if *is_sel { "selected" } else { "" };
        proj_options.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            pid,
            sel,
            html_escape(pname)
        ));
    }

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <form method="POST" action="/bug_clone/{}">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-clone" aria-hidden="true"></i>
                        Clone Issue #{:07}: {}
                    </h4>
                    <div class="widget-toolbar">
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back To Issue</a>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category">Source Project</th>
                                    <td class="input-col">{}</td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="target_project_id">Target Project</label></th>
                                    <td class="input-col">
                                        <select id="target_project_id" name="target_project_id" style="max-width:320px;" required>
                                            {}
                                        </select>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Summary</th>
                                    <td class="input-col">{}</td>
                                </tr>
                                <tr>
                                    <th class="category">Priority / Severity</th>
                                    <td class="input-col">{} / {}</td>
                                </tr>
                                <tr>
                                    <th class="category">Copy Options</th>
                                    <td class="input-col">
                                        <div style="display:flex; flex-direction:column; gap:6px;">
                                            <label class="inline-label">
                                                <input type="checkbox" name="copy_relationships" value="1" checked> Copy Relationships
                                            </label>
                                            <label class="inline-label">
                                                <input type="checkbox" name="copy_files" value="1" checked> Copy File Attachments
                                            </label>
                                        </div>
                                    </td>
                                </tr>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox" style="display:flex; gap:8px; align-items:center;">
                        <button type="submit" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-clone" aria-hidden="true"></i> Clone Issue</button>
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}">Cancel</a>
                    </div>
                </div>
            </div>
        </form>"#,
        err_banner,
        bug.id,
        bug.id,
        html_escape(&bug.summary),
        bug.id,
        html_escape(source_proj_name),
        proj_options,
        html_escape(&bug.summary),
        html_escape(priority_label(bug.priority)),
        html_escape(severity_label(bug.severity)),
        bug.id
    );

    render_layout(
        &format!("Clone Issue #{:07}", bug.id),
        ctx,
        flash_msg,
        &content,
    )
}

pub fn render_bug_move_page(
    ctx: &AuthContext,
    full: &BugFull,
    source_proj_name: &str,
    target_projects: &[(i64, String, bool)],
    flash_err: Option<&str>,
    flash_msg: Option<&str>,
) -> Html<String> {
    let bug = &full.bug;
    let mut proj_options = String::new();
    for (pid, pname, is_sel) in target_projects {
        let sel = if *is_sel { "selected" } else { "" };
        proj_options.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            pid,
            sel,
            html_escape(pname)
        ));
    }

    let err_banner = flash_err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <form method="POST" action="/bug_move/{}">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header widget-header-small">
                    <h4 class="widget-title lighter">
                        <i class="ace-icon fa fa-arrows" aria-hidden="true"></i>
                        Move Issue #{:07}: {}
                    </h4>
                    <div class="widget-toolbar">
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}"><i class="fa fa-arrow-left" aria-hidden="true"></i> Back To Issue</a>
                    </div>
                </div>
                <div class="widget-body">
                    <div class="widget-main no-padding">
                        <div class="table-responsive">
                            <table class="form-table form-table-left">
                                <tr>
                                    <th class="category">Current Project</th>
                                    <td class="input-col">{}</td>
                                </tr>
                                <tr>
                                    <th class="category"><span class="required">*</span> <label for="target_project_id">Target Project</label></th>
                                    <td class="input-col">
                                        <select id="target_project_id" name="target_project_id" style="max-width:320px;" required>
                                            {}
                                        </select>
                                        <div style="margin-top:6px; color:#666; font-size:12px;">
                                            If the issue's current category is not available in the target project, it will be automatically reset to the target project's default category.
                                        </div>
                                    </td>
                                </tr>
                                <tr>
                                    <th class="category">Summary</th>
                                    <td class="input-col">{}</td>
                                </tr>
                            </table>
                        </div>
                    </div>
                    <div class="widget-toolbox" style="display:flex; gap:8px; align-items:center;">
                        <button type="submit" class="btn btn-primary btn-white btn-round btn-sm"><i class="fa fa-arrows" aria-hidden="true"></i> Move Issue</button>
                        <a class="btn btn-white btn-round btn-sm" href="/view_bug_page/{}">Cancel</a>
                    </div>
                </div>
            </div>
        </form>"#,
        err_banner,
        bug.id,
        bug.id,
        html_escape(&bug.summary),
        bug.id,
        html_escape(source_proj_name),
        proj_options,
        html_escape(&bug.summary),
        bug.id
    );

    render_layout(
        &format!("Move Issue #{:07}", bug.id),
        ctx,
        flash_msg,
        &content,
    )
}

#[cfg(test)]
#[path = "bugs_test.rs"]
mod tests;
