use crate::auth::AuthContext;
use crate::models::{
    compute_billing_cost, sanitize_spreadsheet_cell, url_encode, BillingFilterOptions,
    BillingIssueSummary, Bug, BugPriority, BugResolution, BugSeverity, BugStatus, EmailMessage,
    ExtendedSummaryData, NewsItem, RoadmapVersionGroup, SummaryCounts, TimeTrackingEntry,
    ViewState, SUMMARY_RESOLUTION_CODES,
};
use crate::views::charts::{
    render_donut_chart, render_horizontal_bar_chart, render_issues_trend_svg, CHART_PALETTE,
};
use crate::views::layout::render_layout;
use crate::views::{
    format_date, format_timestamp, html_escape, render_status_badge, status_color, xml_escape,
};
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

fn render_summary_subnav(active_graphs: bool) -> String {
    let summary_cls = if !active_graphs {
        "btn btn-primary btn-sm"
    } else {
        "btn btn-white btn-round btn-sm"
    };
    let graph_cls = if active_graphs {
        "btn btn-primary btn-sm"
    } else {
        "btn btn-white btn-round btn-sm"
    };
    format!(
        r#"<div class="flex flex-wrap items-center justify-between gap-2 mb-3">
            <h2 class="section-title mb-0 border-none"><i class="fa fa-bar-chart-o" aria-hidden="true"></i> Summary Statistics</h2>
            <div class="flex flex-wrap items-center gap-1.5">
                <a href="/summary_page" class="{summary_cls}"><i class="fa fa-table" aria-hidden="true"></i> Summary Tables</a>
                <a href="/summary_graph_page" class="{graph_cls}"><i class="fa fa-area-chart" aria-hidden="true"></i> Graphs &amp; Trends</a>
            </div>
        </div>"#,
        summary_cls = summary_cls,
        graph_cls = graph_cls,
    )
}

fn render_trend_period_links(base_path: &str, active_days: i64) -> String {
    let mut out = String::from(r#"<div class="inline-flex items-center gap-1">"#);
    for &d in &[7i64, 30, 90, 365] {
        let cls = if d == active_days {
            "btn btn-primary btn-sm pointer-events-none"
        } else {
            "btn btn-white btn-round btn-sm"
        };
        out.push_str(&format!(
            r#"<a href="{}?days={}" class="{}">{}d</a>"#,
            base_path, d, cls, d
        ));
    }
    out.push_str("</div>");
    out
}

fn render_summary_chart_widget(
    title: &str,
    icon: &str,
    toolbar_html: &str,
    chart_svg: &str,
) -> String {
    format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title"><i class="fa {icon}" aria-hidden="true"></i> {title}</h4>
                <div class="widget-toolbar">{toolbar_html}</div>
            </div>
            <div class="widget-body">{chart_svg}</div>
        </div>"#,
        icon = icon,
        title = html_escape(title),
        toolbar_html = toolbar_html,
        chart_svg = chart_svg,
    )
}

fn build_palette_items<I: Iterator<Item = (String, usize)>>(
    iter: I,
) -> Vec<(String, usize, &'static str)> {
    iter.enumerate()
        .map(|(i, (label, cnt))| (label, cnt, CHART_PALETTE[i % CHART_PALETTE.len()]))
        .collect()
}

pub fn render_summary_page_extended(
    ctx: &AuthContext,
    summary: &ExtendedSummaryData,
    show_user_metrics: bool,
) -> Html<String> {
    let proj_rows: Vec<(String, SummaryCounts)> = summary
        .by_project
        .iter()
        .map(|(p, &c)| (html_escape(p), c))
        .collect();
    let status_rows: Vec<(String, SummaryCounts)> = summary
        .by_status
        .iter()
        .map(|(&st, &c)| (render_status_badge(st, status_label(st)), c))
        .collect();
    let sev_rows: Vec<(String, SummaryCounts)> = summary
        .by_severity
        .iter()
        .map(|(&sev, &c)| (html_escape(severity_label(sev)), c))
        .collect();
    let pri_rows: Vec<(String, SummaryCounts)> = summary
        .by_priority
        .iter()
        .map(|(&pri, &c)| (html_escape(priority_label(pri)), c))
        .collect();
    let cat_rows: Vec<(String, SummaryCounts)> = summary
        .by_category
        .iter()
        .map(|(cat, &c)| (html_escape(cat), c))
        .collect();
    let res_rows: Vec<(String, SummaryCounts)> = summary
        .by_resolution
        .iter()
        .map(|(&res, &c)| (html_escape(resolution_label(res)), c))
        .collect();
    let rep_rows: Vec<(String, SummaryCounts)> = summary
        .by_reporter
        .iter()
        .map(|(rep, &c)| (html_escape(rep), c))
        .collect();
    let hnd_rows: Vec<(String, SummaryCounts)> = summary
        .by_handler
        .iter()
        .map(|(hnd, &c)| (html_escape(hnd), c))
        .collect();

    let proj_widget = render_summary_ratio_widget("By Project", "fa-cubes", "Project", &proj_rows);
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

    let mut date_rows_html = String::new();
    for (idx, b) in summary.by_date.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let bal_cls = if b.balance > 0 {
            "text-red-700 font-semibold"
        } else if b.balance < 0 {
            "text-emerald-700 font-semibold"
        } else {
            "text-gray-600"
        };
        let bal_str = if b.balance > 0 {
            format!("+{}", b.balance)
        } else {
            b.balance.to_string()
        };
        date_rows_html.push_str(&format!(
            r#"<tr class="{row_cls}"><td>{days} day(s)</td><td>{opened}</td><td>{resolved}</td><td class="{bal_cls}">{bal_str}</td></tr>"#,
            row_cls = row_cls,
            days = b.days,
            opened = b.opened,
            resolved = b.resolved,
            bal_cls = bal_cls,
            bal_str = bal_str,
        ));
    }
    let by_date_widget = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header"><h4 class="widget-title"><i class="fa fa-calendar" aria-hidden="true"></i> By Date (Opened vs Resolved)</h4></div>
            <div class="widget-body"><div class="table-responsive">
                <table class="data-table">
                    <thead><tr><th>Window</th><th>Opened</th><th>Resolved</th><th>Balance</th></tr></thead>
                    <tbody>{date_rows_html}</tbody>
                </table>
            </div></div>
        </div>"#
    );

    let mut longest_rows_html = String::new();
    if summary.longest_open.is_empty() {
        longest_rows_html.push_str(r#"<tr><td colspan="2">No open issues.</td></tr>"#);
    } else {
        for (idx, item) in summary.longest_open.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            longest_rows_html.push_str(&format!(
                r#"<tr class="{row_cls}"><td><a href="/view_bug_page/{id}">{id:07}</a> — {sum}</td><td class="whitespace-nowrap text-right">{days} d</td></tr>"#,
                row_cls = row_cls,
                id = item.id,
                sum = html_escape(&item.summary),
                days = item.days_open,
            ));
        }
    }
    let longest_widget = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header"><h4 class="widget-title"><i class="fa fa-hourglass-half" aria-hidden="true"></i> Longest Open Issues</h4></div>
            <div class="widget-body"><div class="table-responsive">
                <table class="data-table">
                    <thead><tr><th>Issue</th><th class="text-right">Days Open</th></tr></thead>
                    <tbody>{longest_rows_html}</tbody>
                </table>
            </div></div>
        </div>"#
    );

    let mut active_rows_html = String::new();
    if summary.most_active.is_empty() {
        active_rows_html.push_str(r#"<tr><td colspan="2">No active open issues.</td></tr>"#);
    } else {
        for (idx, item) in summary.most_active.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            active_rows_html.push_str(&format!(
                r#"<tr class="{row_cls}"><td><a href="/view_bug_page/{id}">{id:07}</a> — {sum}</td><td class="whitespace-nowrap text-right">{cnt}</td></tr>"#,
                row_cls = row_cls,
                id = item.id,
                sum = html_escape(&item.summary),
                cnt = item.activity_count,
            ));
        }
    }
    let most_active_widget = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header"><h4 class="widget-title"><i class="fa fa-bolt" aria-hidden="true"></i> Most Active Open Issues</h4></div>
            <div class="widget-body"><div class="table-responsive">
                <table class="data-table">
                    <thead><tr><th>Issue</th><th class="text-right">Updates</th></tr></thead>
                    <tbody>{active_rows_html}</tbody>
                </table>
            </div></div>
        </div>"#
    );

    let mut dev_res_head = String::from("<th>Developer</th>");
    for &code in &SUMMARY_RESOLUTION_CODES {
        dev_res_head.push_str(&format!("<th>{}</th>", html_escape(resolution_label(code))));
    }
    dev_res_head.push_str("<th>Total</th><th>% Fixed</th>");

    let mut dev_res_body = String::new();
    if summary.dev_resolution.is_empty() {
        dev_res_body.push_str(&format!(
            r#"<tr><td colspan="{}">No developer resolution data available.</td></tr>"#,
            SUMMARY_RESOLUTION_CODES.len() + 3
        ));
    } else {
        for (idx, row) in summary.dev_resolution.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let mut cells = format!("<td>{}</td>", html_escape(&row.username));
            for &code in &SUMMARY_RESOLUTION_CODES {
                let c = row.counts_by_resolution.get(&code).copied().unwrap_or(0);
                cells.push_str(&format!("<td>{}</td>", c));
            }
            cells.push_str(&format!(
                "<td><strong>{}</strong></td><td>{}%</td>",
                row.total, row.pct_fixed
            ));
            dev_res_body.push_str(&format!(r#"<tr class="{}">{}</tr>"#, row_cls, cells));
        }
    }
    let dev_res_widget = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header"><h4 class="widget-title"><i class="fa fa-th" aria-hidden="true"></i> Developer &times; Resolution Breakdown</h4></div>
            <div class="widget-body"><div class="table-responsive">
                <table class="data-table text-xs">
                    <thead><tr>{dev_res_head}</tr></thead>
                    <tbody>{dev_res_body}</tbody>
                </table>
            </div></div>
        </div>"#
    );

    let mut rep_eff_rows = String::new();
    if summary.reporter_effectiveness.is_empty() {
        rep_eff_rows.push_str(r#"<tr><td colspan="5">No reporter data available.</td></tr>"#);
    } else {
        for (idx, row) in summary.reporter_effectiveness.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            rep_eff_rows.push_str(&format!(
                r#"<tr class="{row_cls}"><td>{user}</td><td>{bugs}</td><td>{sev}</td><td>{fal}</td><td><strong>{eff}</strong></td></tr>"#,
                row_cls = row_cls,
                user = html_escape(&row.username),
                bugs = row.total_bugs,
                sev = row.severity_score,
                fal = row.false_score,
                eff = row.effectiveness,
            ));
        }
    }
    let rep_eff_widget = format!(
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header"><h4 class="widget-title"><i class="fa fa-bullseye" aria-hidden="true"></i> Reporter Effectiveness</h4></div>
            <div class="widget-body"><div class="table-responsive">
                <table class="data-table">
                    <thead><tr><th>Reporter</th><th>Issues</th><th>Severity Score</th><th>False / Non-Issue</th><th>Net Score</th></tr></thead>
                    <tbody>{rep_eff_rows}</tbody>
                </table>
            </div></div>
        </div>"#
    );

    let status_chart_items: Vec<(String, usize, &'static str)> = summary
        .by_status
        .iter()
        .map(|(&st, c)| (status_label(st).to_string(), c.total(), status_color(st)))
        .collect();
    let status_donut = render_summary_chart_widget(
        "Status Distribution (SVG)",
        "fa-pie-chart",
        "",
        &render_donut_chart("Status Distribution", &status_chart_items),
    );
    let trend_widget = render_summary_chart_widget(
        &format!("Issues Trend (Last {} Days)", summary.trend_days),
        "fa-line-chart",
        &render_trend_period_links("/summary_page", summary.trend_days),
        &render_issues_trend_svg("Issues Trend", &summary.trend_points),
    );

    let (rep_eff_slot, rep_slot, hnd_slot, dev_res_slot) = if show_user_metrics {
        (
            rep_eff_widget,
            rep_widget,
            hnd_widget,
            format!(r#"<div class="mt-4">{dev_res_widget}</div>"#),
        )
    } else {
        (String::new(), String::new(), String::new(), String::new())
    };

    let content = format!(
        r#"{subnav}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-dashboard" aria-hidden="true"></i> Overview (Total Issues: {total})</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table" style="margin-bottom:0;">
                        <thead>
                            <tr><th>Open</th><th>Resolved</th><th>Closed</th><th>Total</th></tr>
                        </thead>
                        <tbody>
                            <tr class="row-1"><td>{open}</td><td>{resolved}</td><td>{closed}</td><td><strong>{total}</strong></td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {status_donut}
            {trend_widget}
            {proj_widget}
            {status_widget}
            {cat_widget}
            {res_widget}
            {sev_widget}
            {pri_widget}
            {by_date_widget}
            {longest_widget}
            {most_active_widget}
            {rep_eff_slot}
            {rep_slot}
            {hnd_slot}
        </div>
        {dev_res_slot}"#,
        subnav = render_summary_subnav(false),
        total = summary.total_bugs,
        open = summary.open_count,
        resolved = summary.resolved_count,
        closed = summary.closed_count,
        status_donut = status_donut,
        trend_widget = trend_widget,
        proj_widget = proj_widget,
        status_widget = status_widget,
        cat_widget = cat_widget,
        res_widget = res_widget,
        sev_widget = sev_widget,
        pri_widget = pri_widget,
        by_date_widget = by_date_widget,
        longest_widget = longest_widget,
        most_active_widget = most_active_widget,
        rep_eff_slot = rep_eff_slot,
        rep_slot = rep_slot,
        hnd_slot = hnd_slot,
        dev_res_slot = dev_res_slot,
    );

    render_layout("Summary", ctx, None, &content)
}

pub fn render_summary_graph_page(
    ctx: &AuthContext,
    summary: &ExtendedSummaryData,
    show_user_metrics: bool,
) -> Html<String> {
    let status_items: Vec<(String, usize, &'static str)> = summary
        .by_status
        .iter()
        .map(|(&st, c)| (status_label(st).to_string(), c.total(), status_color(st)))
        .collect();
    let proj_items = build_palette_items(
        summary
            .by_project
            .iter()
            .map(|(k, c)| (k.clone(), c.total())),
    );
    let cat_items = build_palette_items(
        summary
            .by_category
            .iter()
            .map(|(k, c)| (k.clone(), c.total())),
    );
    let sev_items = build_palette_items(
        summary
            .by_severity
            .iter()
            .map(|(&k, c)| (severity_label(k).to_string(), c.total())),
    );
    let pri_items = build_palette_items(
        summary
            .by_priority
            .iter()
            .map(|(&k, c)| (priority_label(k).to_string(), c.total())),
    );
    let res_items = build_palette_items(
        summary
            .by_resolution
            .iter()
            .map(|(&k, c)| (resolution_label(k).to_string(), c.total())),
    );
    let dev_items = build_palette_items(
        summary
            .by_handler
            .iter()
            .map(|(k, c)| (k.clone(), c.total())),
    );
    let rep_items = build_palette_items(
        summary
            .by_reporter
            .iter()
            .map(|(k, c)| (k.clone(), c.total())),
    );

    let trend_widget = render_summary_chart_widget(
        &format!("Issues Trend (Last {} Days)", summary.trend_days),
        "fa-line-chart",
        &render_trend_period_links("/summary_graph_page", summary.trend_days),
        &render_issues_trend_svg("Issues Trend", &summary.trend_points),
    );
    let status_donut = render_summary_chart_widget(
        "By Status (Donut)",
        "fa-pie-chart",
        "",
        &render_donut_chart("By Status", &status_items),
    );
    let status_bar = render_summary_chart_widget(
        "By Status (Bar)",
        "fa-bar-chart",
        "",
        &render_horizontal_bar_chart("By Status", &status_items),
    );
    let proj_bar = render_summary_chart_widget(
        "By Project",
        "fa-cubes",
        "",
        &render_horizontal_bar_chart("By Project", &proj_items),
    );
    let cat_bar = render_summary_chart_widget(
        "By Category",
        "fa-folder-open",
        "",
        &render_horizontal_bar_chart("By Category", &cat_items),
    );
    let sev_bar = render_summary_chart_widget(
        "By Severity",
        "fa-exclamation-triangle",
        "",
        &render_horizontal_bar_chart("By Severity", &sev_items),
    );
    let pri_bar = render_summary_chart_widget(
        "By Priority",
        "fa-arrow-up",
        "",
        &render_horizontal_bar_chart("By Priority", &pri_items),
    );
    let res_bar = render_summary_chart_widget(
        "By Resolution",
        "fa-check-circle",
        "",
        &render_horizontal_bar_chart("By Resolution", &res_items),
    );
    let (dev_bar, rep_bar) = if show_user_metrics {
        (
            render_summary_chart_widget(
                "By Developer",
                "fa-wrench",
                "",
                &render_horizontal_bar_chart("By Developer", &dev_items),
            ),
            render_summary_chart_widget(
                "By Reporter",
                "fa-user",
                "",
                &render_horizontal_bar_chart("By Reporter", &rep_items),
            ),
        )
    } else {
        (String::new(), String::new())
    };

    let content = format!(
        r#"{subnav}
        <div class="mb-4">{trend_widget}</div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {status_donut}
            {status_bar}
            {proj_bar}
            {cat_bar}
            {sev_bar}
            {pri_bar}
            {res_bar}
            {dev_bar}
            {rep_bar}
        </div>"#,
        subnav = render_summary_subnav(true),
        trend_widget = trend_widget,
        status_donut = status_donut,
        status_bar = status_bar,
        proj_bar = proj_bar,
        cat_bar = cat_bar,
        sev_bar = sev_bar,
        pri_bar = pri_bar,
        res_bar = res_bar,
        dev_bar = dev_bar,
        rep_bar = rep_bar,
    );

    render_layout("Summary Graphs", ctx, None, &content)
}

pub fn render_billing_page(
    ctx: &AuthContext,
    entries: &[TimeTrackingEntry],
    by_user: &BTreeMap<String, i64>,
    total_mins: i64,
) -> Html<String> {
    render_billing_page_extended(
        ctx,
        entries,
        by_user,
        &[],
        total_mins,
        &BillingFilterOptions::default(),
        None,
    )
}

fn build_billing_export_url(base_path: &str, opts: &BillingFilterOptions) -> String {
    let mut params = Vec::new();
    if let Some(ref f) = opts.from {
        if !f.trim().is_empty() {
            params.push(format!("from={}", url_encode(f.trim())));
        }
    }
    if let Some(ref t) = opts.to {
        if !t.trim().is_empty() {
            params.push(format!("to={}", url_encode(t.trim())));
        }
    }
    if opts.include_subprojects {
        params.push("include_subprojects=1".to_string());
    }
    if opts.hourly_rate > 0.0 {
        params.push(format!("cost={:.2}", opts.hourly_rate));
    }
    if params.is_empty() {
        base_path.to_string()
    } else {
        format!("{}?{}", base_path, params.join("&"))
    }
}

pub fn render_billing_page_extended(
    ctx: &AuthContext,
    entries: &[TimeTrackingEntry],
    by_user: &BTreeMap<String, i64>,
    by_issue: &[BillingIssueSummary],
    total_mins: i64,
    opts: &BillingFilterOptions,
    warning: Option<&str>,
) -> Html<String> {
    let has_rate = opts.hourly_rate > 0.0;
    let total_cost = compute_billing_cost(total_mins, opts.hourly_rate);
    let cost_suffix = if has_rate {
        format!(" | Cost: ${:.2}", total_cost)
    } else {
        String::new()
    };

    let mut rows_html = String::new();
    for (idx, e) in entries.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let cost_td = if has_rate {
            format!(
                r#"<td class="whitespace-nowrap">${:.2}</td>"#,
                compute_billing_cost(i64::from(e.minutes), opts.hourly_rate)
            )
        } else {
            String::new()
        };
        let proj_label = if e.project_name.is_empty() {
            format!("#{}", e.project_id)
        } else {
            e.project_name.clone()
        };
        rows_html.push_str(&format!(
            r#"<tr class="{row_cls}">
                <td class="whitespace-nowrap">{date}</td>
                <td class="whitespace-nowrap">{project}</td>
                <td><a href="/view_bug_page/{id}">{id:07}</a>: {summary}</td>
                <td>{user}</td>
                <td class="whitespace-nowrap">{hh:02}:{mm:02} ({mins} mins)</td>
                {cost_td}
                <td>{note}</td>
            </tr>"#,
            row_cls = row_cls,
            date = html_escape(&format_timestamp(e.date_submitted)),
            project = html_escape(&proj_label),
            id = e.bug_id,
            summary = html_escape(&e.bug_summary),
            user = html_escape(&e.username),
            hh = e.minutes / 60,
            mm = e.minutes % 60,
            mins = e.minutes,
            cost_td = cost_td,
            note = html_escape(&e.note)
        ));
    }
    if rows_html.is_empty() {
        let colspan = if has_rate { 7 } else { 6 };
        rows_html.push_str(&format!(
            r#"<tr><td colspan="{}">No time tracking entries recorded.</td></tr>"#,
            colspan
        ));
    }

    let mut user_rows = String::new();
    for (idx, (u, m)) in by_user.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let cost_td = if has_rate {
            format!(
                r#"<td>${:.2}</td>"#,
                compute_billing_cost(*m, opts.hourly_rate)
            )
        } else {
            String::new()
        };
        user_rows.push_str(&format!(
            r#"<tr class="{}"><td>{}</td><td>{:02}:{:02} ({} mins)</td>{}</tr>"#,
            row_cls,
            html_escape(u),
            m / 60,
            m % 60,
            m,
            cost_td
        ));
    }
    if user_rows.is_empty() {
        let colspan = if has_rate { 3 } else { 2 };
        user_rows.push_str(&format!(
            r#"<tr><td colspan="{}">No user time recorded.</td></tr>"#,
            colspan
        ));
    }

    let mut issue_rows = String::new();
    for (idx, iss) in by_issue.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let cost_td = if has_rate {
            format!(
                r#"<td>${:.2}</td>"#,
                compute_billing_cost(iss.minutes, opts.hourly_rate)
            )
        } else {
            String::new()
        };
        issue_rows.push_str(&format!(
            r#"<tr class="{row_cls}">
                <td>{proj}</td>
                <td><a href="/view_bug_page/{id}">{id:07}</a>: {sum}</td>
                <td>{hh:02}:{mm:02} ({mins} mins)</td>
                {cost_td}
            </tr>"#,
            row_cls = row_cls,
            proj = html_escape(&iss.project_name),
            id = iss.bug_id,
            sum = html_escape(&iss.bug_summary),
            hh = iss.minutes / 60,
            mm = iss.minutes % 60,
            mins = iss.minutes,
            cost_td = cost_td,
        ));
    }
    if issue_rows.is_empty() {
        let colspan = if has_rate { 4 } else { 3 };
        issue_rows.push_str(&format!(
            r#"<tr><td colspan="{}">No issue time recorded.</td></tr>"#,
            colspan
        ));
    }

    let cost_th = if has_rate { "<th>Cost</th>" } else { "" };
    let sub_checked = if opts.include_subprojects {
        "checked"
    } else {
        ""
    };
    let rate_val = if opts.hourly_rate > 0.0 {
        format!("{:.2}", opts.hourly_rate)
    } else {
        String::new()
    };
    let csv_url = build_billing_export_url("/billing_export_csv", opts);
    let excel_url = build_billing_export_url("/billing_export_excel", opts);
    let warning_html = warning
        .filter(|w| !w.trim().is_empty())
        .map(|w| {
            format!(
                r#"<div class="flash-error mb-3"><i class="fa fa-exclamation-triangle" aria-hidden="true"></i> {}</div>"#,
                html_escape(w)
            )
        })
        .unwrap_or_default();

    let content = format!(
        r#"<div class="flex flex-wrap items-center justify-between gap-2 mb-3">
            <h2 class="section-title mb-0 border-none"><i class="fa fa-clock-o" aria-hidden="true"></i> Time Tracking / Billing</h2>
            <div class="flex flex-wrap items-center gap-1.5">
                <a href="{csv_url}" class="btn btn-white btn-round btn-sm"><i class="fa fa-download" aria-hidden="true"></i> CSV Export</a>
                <a href="{excel_url}" class="btn btn-white btn-round btn-sm"><i class="fa fa-file-excel-o" aria-hidden="true"></i> Excel Export</a>
            </div>
        </div>
        {warning_html}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title"><i class="fa fa-filter" aria-hidden="true"></i> Billing Filters &amp; Rate</h4>
            </div>
            <div class="widget-body">
                <form method="GET" action="/billing_page" class="flex flex-wrap items-end gap-3">
                    <div>
                        <label for="billing_from" class="block text-xs font-semibold text-gray-700 mb-1">From Date</label>
                        <input type="date" id="billing_from" name="from" value="{from}" class="text-xs">
                    </div>
                    <div>
                        <label for="billing_to" class="block text-xs font-semibold text-gray-700 mb-1">To Date</label>
                        <input type="date" id="billing_to" name="to" value="{to}" class="text-xs">
                    </div>
                    <div>
                        <label for="billing_cost" class="block text-xs font-semibold text-gray-700 mb-1">Hourly Rate ($/h)</label>
                        <input type="number" step="0.01" min="0" id="billing_cost" name="cost" value="{rate}" placeholder="0.00" class="!w-28 text-xs">
                    </div>
                    <label class="inline-flex items-center gap-1.5 text-xs font-semibold pb-1.5 cursor-pointer">
                        <input type="checkbox" name="include_subprojects" value="1" {sub_checked}>
                        <span>Include Subprojects</span>
                    </label>
                    <div class="flex items-center gap-1.5">
                        <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-check" aria-hidden="true"></i> Apply</button>
                        <a href="/billing_page" class="btn btn-white btn-round btn-sm">Reset</a>
                    </div>
                </form>
            </div>
        </div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-users" aria-hidden="true"></i> Summary by User (Total: {hh:02}:{mm:02} / {total_mins} mins{cost_suffix})</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="data-table">
                            <thead><tr><th>User</th><th>Time Spent</th>{cost_th}</tr></thead>
                            <tbody>{user_rows}</tbody>
                        </table>
                    </div>
                </div>
            </div>
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-bug" aria-hidden="true"></i> Summary by Issue</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="data-table">
                            <thead><tr><th>Project</th><th>Issue</th><th>Time Spent</th>{cost_th}</tr></thead>
                            <tbody>{issue_rows}</tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-list-alt" aria-hidden="true"></i> Detailed Time Entries ({entry_count})</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead><tr><th>Date</th><th>Project</th><th>Issue</th><th>User</th><th>Duration</th>{cost_th}<th>Note</th></tr></thead>
                        <tbody>{rows_html}</tbody>
                    </table>
                </div>
            </div>
        </div>"#,
        csv_url = html_escape(&csv_url),
        excel_url = html_escape(&excel_url),
        from = html_escape(opts.from.as_deref().unwrap_or("")),
        to = html_escape(opts.to.as_deref().unwrap_or("")),
        rate = html_escape(&rate_val),
        sub_checked = sub_checked,
        hh = total_mins / 60,
        mm = total_mins % 60,
        total_mins = total_mins,
        cost_suffix = cost_suffix,
        cost_th = cost_th,
        user_rows = user_rows,
        issue_rows = issue_rows,
        entry_count = entries.len(),
        rows_html = rows_html,
        warning_html = warning_html,
    );

    render_layout("Time Tracking", ctx, None, &content)
}

fn sanitize_billing_csv_cell(val: &str) -> String {
    let safe = sanitize_spreadsheet_cell(val);
    if safe.contains(',') || safe.contains('"') || safe.contains('\n') || safe.contains('\r') {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

pub fn render_billing_csv(entries: &[TimeTrackingEntry], hourly_rate: f64) -> String {
    let has_rate = hourly_rate.is_finite() && hourly_rate > 0.0;
    let mut header = vec![
        "Date", "Project", "Issue ID", "Summary", "User", "Minutes", "Duration",
    ];
    if has_rate {
        header.push("Cost");
    }
    header.push("Note");
    let mut out = format!("{}\r\n", header.join(","));

    for e in entries {
        let mut cells = vec![
            sanitize_billing_csv_cell(&format_timestamp(e.date_submitted)),
            sanitize_billing_csv_cell(&e.project_name),
            format!("{:07}", e.bug_id),
            sanitize_billing_csv_cell(&e.bug_summary),
            sanitize_billing_csv_cell(&e.username),
            e.minutes.to_string(),
            format!("{:02}:{:02}", e.minutes / 60, e.minutes % 60),
        ];
        if has_rate {
            cells.push(format!(
                "{:.2}",
                compute_billing_cost(i64::from(e.minutes), hourly_rate)
            ));
        }
        cells.push(sanitize_billing_csv_cell(&e.note));
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn render_billing_excel_xml(entries: &[TimeTrackingEntry], hourly_rate: f64) -> String {
    let has_rate = hourly_rate.is_finite() && hourly_rate > 0.0;
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <?mso-application progid=\"Excel.Sheet\"?>\n\
         <Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\"\n\
          xmlns:o=\"urn:schemas-microsoft-com:office:office\"\n\
          xmlns:x=\"urn:schemas-microsoft-com:office:excel\"\n\
          xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\">\n\
          <Worksheet ss:Name=\"Billing\">\n    <Table>\n      <Row>\n",
    );

    let mut headers = vec![
        "Date", "Project", "Issue ID", "Summary", "User", "Minutes", "Duration",
    ];
    if has_rate {
        headers.push("Cost");
    }
    headers.push("Note");
    for h in headers {
        out.push_str(&format!(
            "        <Cell><Data ss:Type=\"String\">{}</Data></Cell>\n",
            xml_escape(&sanitize_spreadsheet_cell(h))
        ));
    }
    out.push_str("      </Row>\n");

    for e in entries {
        out.push_str("      <Row>\n");
        let text_cell = |v: &str| -> String {
            let safe = sanitize_spreadsheet_cell(v);
            let esc = xml_escape(&safe).replace('\n', "&#10;");
            format!(
                "        <Cell><Data ss:Type=\"String\">{}</Data></Cell>\n",
                esc
            )
        };
        out.push_str(&text_cell(&format_timestamp(e.date_submitted)));
        out.push_str(&text_cell(&e.project_name));
        out.push_str(&text_cell(&format!("{:07}", e.bug_id)));
        out.push_str(&text_cell(&e.bug_summary));
        out.push_str(&text_cell(&e.username));
        out.push_str(&format!(
            "        <Cell><Data ss:Type=\"Number\">{}</Data></Cell>\n",
            e.minutes
        ));
        out.push_str(&text_cell(&format!(
            "{:02}:{:02}",
            e.minutes / 60,
            e.minutes % 60
        )));
        if has_rate {
            out.push_str(&format!(
                "        <Cell><Data ss:Type=\"Number\">{:.2}</Data></Cell>\n",
                compute_billing_cost(i64::from(e.minutes), hourly_rate)
            ));
        }
        out.push_str(&text_cell(&e.note));
        out.push_str("      </Row>\n");
    }

    out.push_str("    </Table>\n  </Worksheet>\n</Workbook>\n");
    out
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
                    <form method="POST" action="/manage_email_delete/{}" class="inline-block m-0" onsubmit="return confirm('Delete queued email?');">
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
