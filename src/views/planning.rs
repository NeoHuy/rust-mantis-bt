use crate::auth::AuthContext;
use crate::models::{
    url_encode, Bug, BugPriority, BugResolution, BugSeverity, BugStatus, EmailMessage, NewsItem,
    RoadmapVersionGroup, SummaryCounts, TimeTrackingEntry, ViewState,
};
use crate::views::layout::render_layout;
use crate::views::{format_date, format_timestamp, html_escape, render_status_badge};
use axum::response::Html;
use std::collections::{BTreeMap, HashMap};

fn status_label(v: i32) -> &'static str {
    BugStatus::from_i32(v).map(|s| s.as_str()).unwrap_or("new")
}

fn priority_label(v: i32) -> &'static str {
    BugPriority::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("normal")
}

fn severity_label(v: i32) -> &'static str {
    BugSeverity::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("minor")
}

fn resolution_label(v: i32) -> &'static str {
    BugResolution::from_i32(v)
        .map(|s| s.as_str())
        .unwrap_or("open")
}

pub fn render_roadmap_page(
    ctx: &AuthContext,
    version_groups: &[RoadmapVersionGroup],
    users_map: &HashMap<i64, String>,
) -> Html<String> {
    let empty_cats = HashMap::new();
    render_roadmap_page_with_options(ctx, version_groups, users_map, &empty_cats, false)
}

pub fn render_roadmap_page_with_options(
    ctx: &AuthContext,
    version_groups: &[RoadmapVersionGroup],
    users_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
    show_released: bool,
) -> Html<String> {
    let mut sections_html = String::new();
    let distinct_projects_count = version_groups
        .iter()
        .filter_map(|g| {
            g.project_id
                .map(|id| id.to_string())
                .or_else(|| g.project_name.clone())
        })
        .collect::<std::collections::HashSet<_>>()
        .len();

    let toggle_btn = if show_released {
        r#"<a href="/roadmap_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-filter" aria-hidden="true"></i> Unreleased Only</a>"#
    } else {
        r#"<a href="/roadmap_page?show_released=1" class="btn btn-white btn-round btn-sm"><i class="fa fa-history" aria-hidden="true"></i> Show Released Versions</a>"#
    };

    if version_groups.is_empty() {
        sections_html.push_str(
            r#"<div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-road" aria-hidden="true"></i> Roadmap</h4>
                </div>
                <div class="widget-body">
                    <p>No roadmap items with a Target Version found.</p>
                    <p class="mt-2"><a href="/changelog_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-retweet" aria-hidden="true"></i> View Changelog for Released Versions</a></p>
                </div>
            </div>"#,
        );
    } else {
        for group in version_groups {
            let total = group.bugs.len();
            let resolved = group.bugs.iter().filter(|b| b.status >= 80).count();
            let pct = (resolved * 100).checked_div(total).unwrap_or(0);
            let badge = if group.released {
                r#" <span class="badge badge-success">Released</span>"#
            } else {
                r#" <span class="badge badge-warning">Planned</span>"#
            };
            let proj_badge = if distinct_projects_count > 1 {
                if let Some(ref pname) = group.project_name {
                    format!(
                        r#" <span class="badge badge-info">{}</span>"#,
                        html_escape(pname)
                    )
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let scheduled_html = if group.date_order > 0 {
                let d = format_date(group.date_order);
                if d.is_empty() {
                    String::new()
                } else {
                    let date_label = if group.released {
                        "Released"
                    } else {
                        "Scheduled"
                    };
                    format!(
                        r#"<span class="badge badge-info"><i class="fa fa-calendar" aria-hidden="true"></i> {}: {}</span>"#,
                        date_label,
                        html_escape(&d)
                    )
                }
            } else {
                String::new()
            };
            let view_issues_href = if let Some(pid) = group.project_id.filter(|&id| id > 0) {
                format!(
                    "/view_all_bug_page?project_id={}&target_version={}",
                    pid,
                    url_encode(&group.version)
                )
            } else {
                format!(
                    "/view_all_bug_page?target_version={}",
                    url_encode(&group.version)
                )
            };

            let mut list_html = String::new();
            if group.bugs.is_empty() {
                list_html.push_str(
                    r#"<li class="list-none text-[#595959]">No issues targeted for this version.</li>"#,
                );
            } else {
                for b in &group.bugs {
                    let handler_str = if b.handler_id > 0 {
                        format!(
                            " ({})",
                            html_escape(
                                users_map
                                    .get(&b.handler_id)
                                    .map(|s| s.as_str())
                                    .unwrap_or("assigned")
                            )
                        )
                    } else {
                        String::new()
                    };
                    let cat_badge = categories_map
                        .get(&b.category_id)
                        .map(|c| {
                            format!(
                                r#"<span class="badge badge-info">[{}]</span> "#,
                                html_escape(c)
                            )
                        })
                        .unwrap_or_default();
                    let strike_open = if b.status >= 80 { "<s>" } else { "" };
                    let strike_close = if b.status >= 80 { "</s>" } else { "" };
                    list_html.push_str(&format!(
                        r#"<li class="mb-1.5">{cat_badge}{strike_open}<a href="/view_bug_page/{id}">{id:07}</a>: <strong>{summary}</strong>{strike_close} - [{status}]{handler}</li>"#,
                        cat_badge = cat_badge,
                        strike_open = strike_open,
                        strike_close = strike_close,
                        id = b.id,
                        summary = html_escape(&b.summary),
                        status = html_escape(status_label(b.status)),
                        handler = handler_str
                    ));
                }
            }

            sections_html.push_str(&format!(
                r#"<div class="widget-box widget-color-blue2">
                    <div class="widget-header">
                        <h4 class="widget-title"><i class="fa fa-road" aria-hidden="true"></i> Target Version: {version} — {resolved} / {total} resolved ({pct}%){badge}{proj_badge}</h4>
                        <div class="widget-toolbar">
                            {scheduled_html}
                            <a href="{view_issues_href}" class="btn btn-white btn-round btn-sm"><i class="fa fa-list" aria-hidden="true"></i> View Issues</a>
                        </div>
                    </div>
                    <div class="widget-body">
                        <div class="w-full max-w-[400px] h-3 bg-[#e0e6ed] rounded overflow-hidden mb-3">
                            <div class="bg-[#5cb85c] h-full" style="width:{pct}%;"></div>
                        </div>
                        <ul class="pl-5">
                            {list_html}
                        </ul>
                    </div>
                </div>"#,
                version = html_escape(&group.version),
                resolved = resolved,
                total = total,
                pct = pct,
                badge = badge,
                proj_badge = proj_badge,
                scheduled_html = scheduled_html,
                view_issues_href = view_issues_href,
                list_html = list_html
            ));
        }
    }

    let content = format!(
        r#"<div class="flex flex-wrap items-center justify-between gap-2 mb-3">
            <h2 class="section-title mb-0 border-none"><i class="fa fa-road" aria-hidden="true"></i> Roadmap</h2>
            <div class="flex items-center gap-2">
                {toggle_btn}
                <a href="/changelog_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-retweet" aria-hidden="true"></i> Changelog</a>
            </div>
        </div>
        {sections_html}"#,
        toggle_btn = toggle_btn,
        sections_html = sections_html
    );

    render_layout("Roadmap", ctx, None, &content)
}

pub fn render_changelog_page(
    ctx: &AuthContext,
    by_fixed: &BTreeMap<String, Vec<Bug>>,
    users_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
) -> Html<String> {
    let mut sections_html = String::new();
    if by_fixed.is_empty() {
        sections_html.push_str(
            r#"<div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-retweet" aria-hidden="true"></i> Changelog</h4>
                </div>
                <div class="widget-body">
                    <p>No resolved issues with a Fixed in Version found.</p>
                </div>
            </div>"#,
        );
    } else {
        for (ver, items) in by_fixed {
            let mut list_html = String::new();
            for b in items {
                let handler_str = if b.handler_id > 0 {
                    format!(
                        " ({})",
                        html_escape(
                            users_map
                                .get(&b.handler_id)
                                .map(|s| s.as_str())
                                .unwrap_or("assigned")
                        )
                    )
                } else {
                    String::new()
                };
                let cat_badge = categories_map
                    .get(&b.category_id)
                    .map(|c| {
                        format!(
                            r#"<span class="badge badge-info">[{}]</span> "#,
                            html_escape(c)
                        )
                    })
                    .unwrap_or_default();
                list_html.push_str(&format!(
                    r#"<li class="mb-1.5">{cat_badge}<s><a href="/view_bug_page/{id}">{id:07}</a></s>: <strong>{summary}</strong> - [{resolution}]{handler}</li>"#,
                    cat_badge = cat_badge,
                    id = b.id,
                    summary = html_escape(&b.summary),
                    resolution = html_escape(resolution_label(b.resolution)),
                    handler = handler_str
                ));
            }

            let changelog_proj_id = if ctx.active_project_id > 0 {
                Some(ctx.active_project_id)
            } else {
                let first_pid = items.first().map(|b| b.project_id);
                if first_pid.is_some() && items.iter().all(|b| Some(b.project_id) == first_pid) {
                    first_pid
                } else {
                    None
                }
            };
            let view_issues_href = if let Some(pid) = changelog_proj_id.filter(|&id| id > 0) {
                format!(
                    "/view_all_bug_page?project_id={}&fixed_in_version={}",
                    pid,
                    url_encode(ver)
                )
            } else {
                format!("/view_all_bug_page?fixed_in_version={}", url_encode(ver))
            };
            sections_html.push_str(&format!(
                r#"<div class="widget-box widget-color-blue2">
                    <div class="widget-header">
                        <h4 class="widget-title"><i class="fa fa-retweet" aria-hidden="true"></i> Version: {ver} ({count} issues fixed)</h4>
                        <div class="widget-toolbar">
                            <a href="{view_issues_href}" class="btn btn-white btn-round btn-sm"><i class="fa fa-list" aria-hidden="true"></i> View Issues</a>
                        </div>
                    </div>
                    <div class="widget-body">
                        <ul class="pl-5">
                            {list_html}
                        </ul>
                    </div>
                </div>"#,
                ver = html_escape(ver),
                count = items.len(),
                view_issues_href = view_issues_href,
                list_html = list_html
            ));
        }
    }

    let content = format!(
        r#"<div class="flex flex-wrap items-center justify-between gap-2 mb-3">
            <h2 class="section-title mb-0 border-none"><i class="fa fa-retweet" aria-hidden="true"></i> Changelog</h2>
            <a href="/roadmap_page" class="btn btn-white btn-round btn-sm"><i class="fa fa-road" aria-hidden="true"></i> Roadmap</a>
        </div>
        {}"#,
        sections_html
    );

    render_layout("Changelog", ctx, None, &content)
}

fn render_summary_ratio_widget(
    title: &str,
    icon: &str,
    key_col: &str,
    rows: &[(String, SummaryCounts)],
) -> String {
    let mut rows_html = String::new();
    if rows.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="5">No data available.</td></tr>"#);
    } else {
        for (idx, (label_html, counts)) in rows.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            rows_html.push_str(&format!(
                r#"<tr class="{}"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><strong>{}</strong></td></tr>"#,
                row_cls,
                label_html,
                counts.open,
                counts.resolved,
                counts.closed,
                counts.total()
            ));
        }
    }

    format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa {icon}" aria-hidden="true"></i> {title}</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>{key_col}</th>
                                <th>Open</th>
                                <th>Resolved</th>
                                <th>Closed</th>
                                <th>Total</th>
                            </tr>
                        </thead>
                        <tbody>{rows_html}</tbody>
                    </table>
                </div>
            </div>
        </div>"#,
        icon = icon,
        title = html_escape(title),
        key_col = html_escape(key_col),
        rows_html = rows_html
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_summary_page(
    ctx: &AuthContext,
    total_bugs: usize,
    open_count: usize,
    resolved_count: usize,
    closed_count: usize,
    by_status: &BTreeMap<i32, SummaryCounts>,
    by_severity: &BTreeMap<i32, SummaryCounts>,
    by_priority: &BTreeMap<i32, SummaryCounts>,
    by_category: &BTreeMap<String, SummaryCounts>,
    by_resolution: &BTreeMap<i32, SummaryCounts>,
    by_reporter: &BTreeMap<String, SummaryCounts>,
    by_handler: &BTreeMap<String, SummaryCounts>,
) -> Html<String> {
    let status_rows: Vec<(String, SummaryCounts)> = by_status
        .iter()
        .map(|(&st, &c)| (render_status_badge(st, status_label(st)), c))
        .collect();
    let sev_rows: Vec<(String, SummaryCounts)> = by_severity
        .iter()
        .map(|(&sev, &c)| (html_escape(severity_label(sev)), c))
        .collect();
    let pri_rows: Vec<(String, SummaryCounts)> = by_priority
        .iter()
        .map(|(&pri, &c)| (html_escape(priority_label(pri)), c))
        .collect();
    let cat_rows: Vec<(String, SummaryCounts)> = by_category
        .iter()
        .map(|(cat, &c)| (html_escape(cat), c))
        .collect();
    let res_rows: Vec<(String, SummaryCounts)> = by_resolution
        .iter()
        .map(|(&res, &c)| (html_escape(resolution_label(res)), c))
        .collect();
    let rep_rows: Vec<(String, SummaryCounts)> = by_reporter
        .iter()
        .map(|(rep, &c)| (html_escape(rep), c))
        .collect();
    let hnd_rows: Vec<(String, SummaryCounts)> = by_handler
        .iter()
        .map(|(hnd, &c)| (html_escape(hnd), c))
        .collect();

    let status_widget = render_summary_ratio_widget("By Status", "fa-flag", "Status", &status_rows);
    let sev_widget = render_summary_ratio_widget(
        "By Severity",
        "fa-exclamation-triangle",
        "Severity",
        &sev_rows,
    );
    let pri_widget =
        render_summary_ratio_widget("By Priority", "fa-arrow-up", "Priority", &pri_rows);
    let cat_widget =
        render_summary_ratio_widget("By Category", "fa-folder-open", "Category", &cat_rows);
    let res_widget =
        render_summary_ratio_widget("By Resolution", "fa-check-circle", "Resolution", &res_rows);
    let rep_widget = render_summary_ratio_widget("By Reporter", "fa-user", "Reporter", &rep_rows);
    let hnd_widget = render_summary_ratio_widget(
        "By Developer / Handler",
        "fa-wrench",
        "Developer",
        &hnd_rows,
    );

    let content = format!(
        r#"<div class="section-title"><i class="fa fa-bar-chart-o" aria-hidden="true"></i> Summary Statistics</div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-dashboard" aria-hidden="true"></i> Overview (Total Issues: {})</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table" style="margin-bottom:0;">
                        <thead>
                            <tr><th>Open</th><th>Resolved</th><th>Closed</th><th>Total</th></tr>
                        </thead>
                        <tbody>
                            <tr class="row-1"><td>{}</td><td>{}</td><td>{}</td><td><strong>{}</strong></td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {status_widget}
            {sev_widget}
            {pri_widget}
            {cat_widget}
            {res_widget}
            <div>
                {rep_widget}
                {hnd_widget}
            </div>
        </div>"#,
        total_bugs,
        open_count,
        resolved_count,
        closed_count,
        total_bugs,
        status_widget = status_widget,
        sev_widget = sev_widget,
        pri_widget = pri_widget,
        cat_widget = cat_widget,
        res_widget = res_widget,
        rep_widget = rep_widget,
        hnd_widget = hnd_widget
    );

    render_layout("Summary", ctx, None, &content)
}

pub fn render_billing_page(
    ctx: &AuthContext,
    entries: &[TimeTrackingEntry],
    by_user: &BTreeMap<String, i32>,
    total_mins: i32,
) -> Html<String> {
    let mut rows_html = String::new();
    for (idx, e) in entries.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        rows_html.push_str(&format!(
            r#"<tr class="{row_cls}">
                <td><a href="/view_bug_page/{id}">{id:07}</a>: {summary}</td>
                <td>{user}</td>
                <td>{hh:02}:{mm:02} ({mins} mins)</td>
                <td>{note}</td>
            </tr>"#,
            row_cls = row_cls,
            id = e.bug_id,
            summary = html_escape(&e.bug_summary),
            user = html_escape(&e.username),
            hh = e.minutes / 60,
            mm = e.minutes % 60,
            mins = e.minutes,
            note = html_escape(&e.note)
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="4">No time tracking entries recorded.</td></tr>"#);
    }

    let mut user_rows = String::new();
    for (idx, (u, m)) in by_user.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        user_rows.push_str(&format!(
            r#"<tr class="{}"><td>{}</td><td>{:02}:{:02} ({} mins)</td></tr>"#,
            row_cls,
            html_escape(u),
            m / 60,
            m % 60,
            m
        ));
    }
    if user_rows.is_empty() {
        user_rows.push_str(r#"<tr><td colspan="2">No user time recorded.</td></tr>"#);
    }

    let content = format!(
        r#"<div class="section-title"><i class="fa fa-clock-o" aria-hidden="true"></i> Time Tracking / Billing</div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-users" aria-hidden="true"></i> Summary by User (Total: {:02}:{:02} / {} mins)</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead><tr><th>User</th><th>Time Spent</th></tr></thead>
                        <tbody>{}</tbody>
                    </table>
                </div>
            </div>
        </div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-list-alt" aria-hidden="true"></i> Detailed Time Entries</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead><tr><th>Issue</th><th>User</th><th>Duration</th><th>Note</th></tr></thead>
                        <tbody>{}</tbody>
                    </table>
                </div>
            </div>
        </div>"#,
        total_mins / 60,
        total_mins % 60,
        total_mins,
        user_rows,
        rows_html
    );

    render_layout("Time Tracking", ctx, None, &content)
}

pub fn render_news_page(
    ctx: &AuthContext,
    items: &[NewsItem],
    users_map: &HashMap<i64, String>,
    eff: i32,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let mut list_html = String::new();
    if items.is_empty() {
        list_html.push_str("<p>No news announcements posted yet.</p>");
    } else {
        for n in items {
            let poster = users_map
                .get(&n.poster_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", n.poster_id));
            let vs_str = ViewState::from_i32(n.view_state)
                .map(|v| v.as_str())
                .unwrap_or("public");
            let ann_badge = if n.announcement != 0 {
                r#" <span class="badge badge-warning">Announcement</span>"#
            } else {
                ""
            };
            let toolbar = if eff >= 70 {
                format!(
                    r#"<div class="widget-toolbar">
                        <a href="/news_edit_page/{}" class="btn btn-white btn-round btn-sm"><i class="fa fa-pencil" aria-hidden="true"></i> Edit</a>
                        <form method="POST" action="/news_delete/{}" class="inline-block m-0" onsubmit="return confirm('Delete news item?');">
                            <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                        </form>
                    </div>"#,
                    n.id, n.id
                )
            } else {
                String::new()
            };
            list_html.push_str(&format!(
                r#"<div class="widget-box widget-color-blue2">
                    <div class="widget-header">
                        <h4 class="widget-title"><i class="fa fa-newspaper-o" aria-hidden="true"></i> {headline}{ann_badge} <span class="font-normal text-xs opacity-90">— posted by {poster} on {date} [{vs}]</span></h4>
                        {toolbar}
                    </div>
                    <div class="widget-body whitespace-pre-wrap">{body}</div>
                </div>"#,
                headline = html_escape(&n.headline),
                ann_badge = ann_badge,
                poster = html_escape(&poster),
                date = format_timestamp(n.date_posted),
                vs = vs_str,
                toolbar = toolbar,
                body = html_escape(&n.body)
            ));
        }
    }

    let add_form_html = if eff >= 70 {
        format!(
            r#"<div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-plus-circle" aria-hidden="true"></i> Post News Item</h4>
                </div>
                <div class="widget-body">
                    <form method="POST" action="/news_add">
                        <input type="hidden" name="project_id" value="{}">
                        <div class="table-responsive">
                            <table class="form-table">
                                <tr>
                                    <td class="label-col">Headline <span class="required">*</span></td>
                                    <td class="input-col"><input type="text" name="headline" required></td>
                                </tr>
                                <tr>
                                    <td class="label-col">Body <span class="required">*</span></td>
                                    <td class="input-col"><textarea name="body" required></textarea></td>
                                </tr>
                                <tr>
                                    <td class="label-col">Announcement</td>
                                    <td class="input-col"><label><input type="checkbox" name="announcement" value="1"> Pin to top</label></td>
                                </tr>
                                <tr>
                                    <td class="label-col">View Status</td>
                                    <td class="input-col">
                                        <select name="view_state" style="width:auto;">
                                            <option value="10">public</option>
                                            <option value="50">private</option>
                                        </select>
                                    </td>
                                </tr>
                            </table>
                        </div>
                        <button type="submit" class="btn btn-primary"><i class="fa fa-paper-plane" aria-hidden="true"></i> Post News</button>
                    </form>
                </div>
            </div>"#,
            ctx.active_project_id
        )
    } else {
        String::new()
    };

    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let content = format!(
        r#"{}
        <div class="section-title"><i class="fa fa-newspaper-o" aria-hidden="true"></i> Project News</div>
        {}
        {}"#,
        err_banner, list_html, add_form_html
    );

    render_layout("News", ctx, msg, &content)
}

pub fn render_manage_email_queue_page(
    ctx: &AuthContext,
    emails: &[EmailMessage],
    msg: Option<&str>,
) -> Html<String> {
    let mut rows_html = String::new();
    for (idx, e) in emails.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        rows_html.push_str(&format!(
            r#"<tr class="{}">
                <td>{}</td>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td class="whitespace-pre-wrap">{}</td>
                <td class="whitespace-nowrap">{}</td>
                <td>
                    <form method="POST" action="/manage_email_delete/{}" class="inline-block m-0">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete</button>
                    </form>
                </td>
            </tr>"#,
            row_cls,
            e.email_id,
            html_escape(&e.email),
            html_escape(&e.subject),
            html_escape(&e.body),
            format_timestamp(e.submitted),
            e.email_id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="6">Email queue is empty.</td></tr>"#);
    }

    let content = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-envelope-o" aria-hidden="true"></i> Email Notification Queue ({})</h4>
                <div class="widget-toolbar">
                    <form method="POST" action="/manage_email_queue_flush" class="inline-block m-0">
                        <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-paper-plane" aria-hidden="true"></i> Send / Flush Queue</button>
                    </form>
                </div>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>ID</th>
                                <th>Recipient</th>
                                <th>Subject</th>
                                <th>Body</th>
                                <th>Submitted</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>"#,
        emails.len(),
        rows_html
    );

    render_layout("Email Queue", ctx, msg, &content)
}

pub fn render_news_edit_page(
    ctx: &AuthContext,
    item: &NewsItem,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let ann_checked = if item.announcement != 0 {
        "checked"
    } else {
        ""
    };
    let public_selected = if item.view_state == 10 {
        "selected"
    } else {
        ""
    };
    let private_selected = if item.view_state == 50 {
        "selected"
    } else {
        ""
    };

    let content = format!(
        r#"{err_banner}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-pencil" aria-hidden="true"></i> Edit News Item</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/news_update/{id}">
                    <div class="table-responsive">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Headline <span class="required">*</span></td>
                                <td class="input-col"><input type="text" name="headline" value="{headline}" required style="width:100%; max-width:500px;"></td>
                            </tr>
                            <tr>
                                <td class="label-col">Body <span class="required">*</span></td>
                                <td class="input-col"><textarea name="body" required style="width:100%; max-width:600px; height:140px;">{body}</textarea></td>
                            </tr>
                            <tr>
                                <td class="label-col">Announcement</td>
                                <td class="input-col"><label style="cursor:pointer;"><input type="checkbox" name="announcement" value="1" {ann_checked}> Pin to top</label></td>
                            </tr>
                            <tr>
                                <td class="label-col">View Status</td>
                                <td class="input-col">
                                    <select name="view_state" style="width:auto;">
                                        <option value="10" {public_selected}>public</option>
                                        <option value="50" {private_selected}>private</option>
                                    </select>
                                </td>
                            </tr>
                        </table>
                    </div>
                    <div class="mt-3.5 flex gap-2">
                        <button type="submit" class="btn btn-primary"><i class="fa fa-save" aria-hidden="true"></i> Update News</button>
                        <a href="/news_page" class="btn btn-secondary">Cancel</a>
                    </div>
                </form>
            </div>
        </div>"#,
        err_banner = err_banner,
        id = item.id,
        headline = html_escape(&item.headline),
        body = html_escape(&item.body),
        ann_checked = ann_checked,
        public_selected = public_selected,
        private_selected = private_selected,
    );

    render_layout("Edit News", ctx, msg, &content)
}

#[cfg(test)]
#[path = "planning_test.rs"]
mod tests;
