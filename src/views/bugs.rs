use crate::auth::AuthContext;
use crate::models::{
    build_csv_export_url, build_view_all_bug_page_url, clamp_per_page, Bug, BugFilter, BugFull,
    BugHistory, BugPriority, BugReproducibility, BugResolution, BugRevision, BugSeverity,
    BugStatus, Bugnote, Category, CustomField, CustomFieldValue, JiraLink, ListBugsQuery,
    ProjectVersion, RelationshipType, SavedFilter, Tag, TimelineEvent, User, ViewState,
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
    if query.filter_id.unwrap_or(0) > 0 {
        count += 1;
    }
    count
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
            cur_per_page_str
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
            <div class="widget-body">
                <form method="GET" action="/view_all_bug_page">
                    <input type="hidden" name="sort" value="{}">
                    <input type="hidden" name="dir" value="{}">
                    <input type="hidden" name="per_page" value="{}">
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
        html_escape(cur_sort),
        html_escape(cur_dir),
        cur_per_page_str,
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
    versions: &[ProjectVersion],
    custom_fields: &[CustomField],
    effective_access: i32,
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

    let dropzone_html = render_dropzone_html("report", "Drag and drop files here, or");

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Enter Issue Details</div>
            <div class="panel-body">
                <form method="POST" action="/bug_report" enctype="multipart/form-data">
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
                            <td class="label-col">Build</td>
                            <td class="input-col"><input type="text" name="build" placeholder="Build" style="max-width:200px;"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Due Date</td>
                            <td class="input-col"><input type="date" name="due_date" style="max-width:200px;"></td>
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
                            <td class="label-col">Attach Files</td>
                            <td class="input-col">
                                {}
                            </td>
                        </tr>
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
        err_banner, proj_opts, cat_opts, ver_opts, cf_rows_html, dropzone_html
    );

    render_layout("Report Issue", ctx, flash_msg, &content)
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
            <button type="submit" class="btn btn-secondary btn-sm">Change Status</button>
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
                <button type="submit" class="btn btn-secondary btn-sm">Upload Files</button>
            </div>
        </form>"#,
        bug_id, dropzone
    )
}

pub fn render_monitors_html(monitors: &[User], bug_id: i64) -> String {
    if monitors.is_empty() {
        return "<p>No users monitoring this issue.</p>".to_string();
    }
    let mut out = String::from(
        r#"<table class="data-table"><thead><tr><th>Username</th><th>Real Name</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for m in monitors {
        let remove_btn = format!(
            r#"<form method="POST" action="/bug_monitor_delete/{}" style="display:inline;">
                <input type="hidden" name="user_id" value="{}">
                <button type="submit" class="btn btn-danger btn-sm">Remove</button>
            </form>"#,
            bug_id, m.id
        );
        out.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            html_escape(&m.username),
            html_escape(&m.realname),
            remove_btn
        ));
    }
    out.push_str("</tbody></table>");
    out
}

pub fn render_monitor_form_html(bug_id: i64) -> String {
    format!(
        r#"<form method="POST" action="/bug_monitor_add/{}" style="margin-top:10px; display:flex; gap:8px; align-items:center;">
            <input type="text" name="username" placeholder="Username" style="max-width:200px;" required>
            <button type="submit" class="btn btn-secondary btn-sm">Add Monitor</button>
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
                <td class="label-col">{}</td>
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
        r#" <span class="badge" style="background:#e0a800; color:#fff; font-size:11px; font-weight:bold; padding:2px 6px; border-radius:3px; margin-left:6px;">[Sticky]</span>"#
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
            r#" <span class="badge badge-danger" style="background:#d9534f; color:#fff; font-size:11px; padding:2px 6px; border-radius:3px; margin-left:6px;">Overdue</span>"#
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
                            <td>{:07}{}{}</td>
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
                            <td class="label-col">Build</td>
                            <td class="input-col">{}</td>
                            <td class="label-col">Due Date</td>
                            <td class="input-col" colspan="3">{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary</td>
                            <td class="input-col" colspan="5"><strong>{:07}: {}</strong>{}</td>
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

        <div id="attachments" class="panel">
            <div class="panel-header">Attached Files</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div id="bugnotes" class="panel">
            <div class="panel-header">Activities / Bugnotes</div>
            <div class="panel-body">
                {}
                {}
            </div>
        </div>

        <div class="panel">
            <div class="panel-header">Users Monitoring This Issue</div>
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
        </div>

        {}"#,
        banner,
        bug.id,
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
        build_str,
        due_date_html,
        bug.id,
        html_escape(&bug.summary),
        sticky_badge,
        crate::views::formatting::format_text(&full.description, format_ctx),
        crate::views::formatting::format_text(&full.steps_to_reproduce, format_ctx),
        crate::views::formatting::format_text(&full.additional_information, format_ctx),
        cf_rows_html,
        actions_html,
        tags_html,
        rels_html,
        rel_form_html,
        files_html,
        upload_form_html,
        notes_html,
        note_form_html,
        monitors_html,
        monitor_form_html,
        history_html,
        jira_section_html
    );

    render_layout(&format!("Issue #{:07}", bug.id), ctx, flash_msg, &content)
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
    let content = format!(
        r#"<div class="panel">
            <div class="panel-header">Update Issue #{:07}</div>
            <div class="panel-body">
                <form method="POST" action="/update_bug/{}">
                    <input type="hidden" name="last_updated" value="{}">
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
                            <td class="label-col">Platform</td>
                            <td class="input-col"><input type="text" name="platform" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">OS</td>
                            <td class="input-col"><input type="text" name="os" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">OS Build</td>
                            <td class="input-col"><input type="text" name="os_build" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Build</td>
                            <td class="input-col"><input type="text" name="build" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Due Date</td>
                            <td class="input-col"><input type="date" name="due_date" value="{}"></td>
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
        bug.last_updated,
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
        cf_rows_html
    );

    render_layout(&format!("Update Issue #{:07}", bug.id), ctx, None, &content)
}

pub fn render_bugnotes_table(
    notes: &[Bugnote],
    users_map: &HashMap<i64, String>,
    eff: i32,
    current_user_id: i64,
    format_ctx: &crate::views::formatting::FormatContext,
) -> String {
    if notes.is_empty() {
        return "<p>No notes attached to this issue.</p>".to_string();
    }
    let mut html = String::from(
        r#"<table class="data-table"><thead><tr><th>Author</th><th>Note</th><th>Time Spent</th><th>Visibility</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for n in notes {
        let author = users_map
            .get(&n.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", n.reporter_id));
        let vis = if n.view_state == 50 {
            "private"
        } else {
            "public"
        };
        let time_str = if n.time_tracking > 0 {
            format!("{:02}:{:02}", n.time_tracking / 60, n.time_tracking % 60)
        } else {
            "-".to_string()
        };
        let can_edit_note = eff >= 70 || (eff >= 25 && n.reporter_id == current_user_id);
        let can_del_note = can_edit_note;
        let can_toggle_vs = eff >= 55;

        let mut actions = Vec::new();

        if can_edit_note {
            actions.push(format!(
                r#"<a href="/bugnote_edit_page/{}" class="btn btn-sm">Edit</a>"#,
                n.id
            ));
        }

        if can_toggle_vs {
            let (btn_label, next_vs) = if n.view_state == 50 {
                ("Make Public", 10)
            } else {
                ("Make Private", 50)
            };
            actions.push(format!(
                r#"<form method="POST" action="/bugnote_set_view_state/{}" style="display:inline;"><input type="hidden" name="view_state" value="{}"><button type="submit" class="btn btn-sm">{}</button></form>"#,
                n.id, next_vs, btn_label
            ));
        }

        actions.push(format!(
            r#"<a href="/bug_revision_view_page/{}?bugnote_id={}" class="btn btn-sm">Revisions</a>"#,
            n.bug_id, n.id
        ));

        if can_del_note {
            actions.push(format!(
                r#"<form method="POST" action="/bugnote_delete/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                n.id
            ));
        }

        let actions_html = actions.join(" ");
        let reminder_badge = render_reminder_badge(n.note_type);

        let formatted_note = crate::views::formatting::format_text(&n.note, format_ctx);

        html.push_str(&format!(
            r#"<tr id="c{}">
                <td style="width:180px; background:#f2f5f8;">
                    <strong>{}</strong>{}<br>
                    <span style="font-size:11px; color:#666;">{}</span>
                </td>
                <td style="white-space:pre-wrap;">{}</td>
                <td style="width:90px;">{}</td>
                <td style="width:90px;">{}</td>
                <td style="width:260px;">{}</td>
            </tr>"#,
            n.id,
            html_escape(&author),
            reminder_badge,
            format_timestamp(n.date_submitted),
            formatted_note,
            time_str,
            vis,
            actions_html
        ));
    }
    html.push_str("</tbody></table>");
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
        r#"{}<div class="panel">
            <div class="panel-header">Edit Bugnote #{:07} (Issue #{:07})</div>
            <div class="panel-body">
                <form method="POST" action="/bugnote_update/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Bugnote *</td>
                            <td class="input-col"><textarea name="note" rows="8" style="width:100%;" required>{}</textarea></td>
                        </tr>
                    </table>
                    <div style="margin-top:12px; display:flex; gap:8px;">
                        <button type="submit" class="btn">Update Information</button>
                        <a href="/view_bug_page/{}" class="btn btn-secondary" style="text-decoration:none;">Cancel</a>
                    </div>
                </form>
            </div>
        </div>"#,
        banner,
        note.id,
        bug.id,
        note.id,
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
        <div class="panel">
            <div class="panel-header">Send a Reminder for Issue #{:07}: {}</div>
            <div class="panel-body">
                <form method="POST" action="/bug_reminder/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Recipient(s) *</td>
                            <td class="input-col">
                                <select name="to" multiple size="8" style="min-width:280px; width:100%;" required>
                                    {}
                                </select>
                                <span style="font-size:11px; color:#666; display:block; margin-top:4px;">Hold Ctrl/Cmd to select multiple recipients.</span>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Reminder Message *</td>
                            <td class="input-col">
                                <textarea name="body" rows="6" style="width:100%;" required placeholder="Enter reminder message here..."></textarea>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Store as Note</td>
                            <td class="input-col">
                                <label style="display:inline-flex; align-items:center; gap:6px; cursor:pointer;">
                                    <input type="hidden" name="store_as_note" value="0">
                                    <input type="checkbox" name="store_as_note" value="1" checked>
                                    <span>Store reminder as an issue note</span>
                                </label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">View Status</td>
                            <td class="input-col">
                                <select name="view_state" style="width:auto;">
                                    <option value="10" selected>public</option>
                                    <option value="50">private</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Add to Monitors</td>
                            <td class="input-col">
                                <label style="display:inline-flex; align-items:center; gap:6px; cursor:pointer;">
                                    <input type="hidden" name="add_monitors" value="0">
                                    <input type="checkbox" name="add_monitors" value="1" checked>
                                    <span>Add recipients to monitoring list</span>
                                </label>
                            </td>
                        </tr>
                    </table>
                    <div style="margin-top:12px; display:flex; gap:8px;">
                        <button type="submit" class="btn">Send Reminder</button>
                        <a href="/view_bug_page/{}" class="btn btn-secondary">Cancel</a>
                    </div>
                </form>
            </div>
        </div>"#,
        err_banner,
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
            r#"<table class="data-table"><thead><tr><th>Timestamp</th><th>Username</th><th>Revision Type</th><th>Previous Content</th>"#,
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
                    r#"<td style="width:120px;"><form method="POST" action="/bug_revision_drop/{}" style="display:inline;" onsubmit="return confirm('Are you sure you want to drop this revision?');"><button type="submit" class="btn btn-danger btn-sm">Drop Revision</button></form></td>"#,
                    r.id
                )
            } else {
                String::new()
            };

            table.push_str(&format!(
                r#"<tr>
                    <td style="width:160px; white-space:nowrap;">{}</td>
                    <td style="width:140px;">{}</td>
                    <td style="width:160px;"><span class="badge">{}</span></td>
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

        table.push_str("</tbody></table>");
        table
    };

    let content = format!(
        r#"{}<div class="panel">
            <div class="panel-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Revisions {}</span>
                <a href="/view_bug_page/{}" class="btn btn-secondary btn-sm" style="text-decoration:none;">Back to Issue</a>
            </div>
            <div class="panel-body">
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
<div class="well">
    <p>This issue has been modified by another user or session since you loaded the update page. Please reload the issue to review the latest changes and re-apply your updates.</p>
    <div style="margin-top:16px;">
        <a href="/view_bug_page/{}" class="btn btn-primary btn-sm"><i class="fa fa-refresh"></i> Reload Issue Details</a>
        <a href="/update_bug_page/{}" class="btn btn-secondary btn-sm" style="margin-left:8px;"><i class="fa fa-pencil"></i> Return to Edit Page</a>
    </div>
</div>"#,
        html_escape(message),
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
        <div class="panel">
            <div class="panel-header">Clone Issue #{:07}: {}</div>
            <div class="panel-body">
                <form method="POST" action="/bug_clone/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Source Project</td>
                            <td>{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Target Project <span class="required">*</span></td>
                            <td>
                                <select name="target_project_id" style="max-width:320px;" required>
                                    {}
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary</td>
                            <td>{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Priority / Severity</td>
                            <td>{} / {}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Copy Options</td>
                            <td>
                                <div style="display:flex; flex-direction:column; gap:6px;">
                                    <label style="display:inline-flex; align-items:center; gap:6px;">
                                        <input type="checkbox" name="copy_relationships" value="1" checked> Copy Relationships
                                    </label>
                                    <label style="display:inline-flex; align-items:center; gap:6px;">
                                        <input type="checkbox" name="copy_files" value="1" checked> Copy File Attachments
                                    </label>
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td></td>
                            <td>
                                <button type="submit" class="btn btn-primary btn-sm">Clone Issue</button>
                                <a class="btn btn-secondary btn-sm" href="/view_bug_page/{}" style="margin-left:8px;">Cancel</a>
                            </td>
                        </tr>
                    </table>
                </form>
            </div>
        </div>"#,
        err_banner,
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
        <div class="panel">
            <div class="panel-header">Move Issue #{:07}: {}</div>
            <div class="panel-body">
                <form method="POST" action="/bug_move/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Current Project</td>
                            <td>{}</td>
                        </tr>
                        <tr>
                            <td class="label-col">Target Project <span class="required">*</span></td>
                            <td>
                                <select name="target_project_id" style="max-width:320px;" required>
                                    {}
                                </select>
                                <div style="margin-top:6px; color:#666; font-size:12px;">
                                    If the issue's current category is not available in the target project, it will be automatically reset to the target project's default category.
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Summary</td>
                            <td>{}</td>
                        </tr>
                        <tr>
                            <td></td>
                            <td>
                                <button type="submit" class="btn btn-primary btn-sm">Move Issue</button>
                                <a class="btn btn-secondary btn-sm" href="/view_bug_page/{}" style="margin-left:8px;">Cancel</a>
                            </td>
                        </tr>
                    </table>
                </form>
            </div>
        </div>"#,
        err_banner,
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
