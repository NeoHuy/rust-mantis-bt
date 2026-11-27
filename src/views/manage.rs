use crate::auth::AuthContext;
use crate::models::{SiteOverviewStats, SiteRuntimeConfig};
use crate::views::{html_escape, layout};
use axum::response::Html;

/// Renders the `/manage_overview_page` Site Information dashboard (Admin >= 90 only).
pub fn render_manage_overview_page(
    ctx: &AuthContext,
    stats: &SiteOverviewStats,
    runtime: &SiteRuntimeConfig,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let runtime_rows_data = [
        (
            "Cookie Secure / HSTS (MANTIS_COOKIE_SECURE)",
            if runtime.cookie_secure {
                "Enabled"
            } else {
                "Disabled"
            },
        ),
        (
            "Attachment Storage (MANTIS_UPLOAD_DIR)",
            if runtime.upload_fs {
                "Filesystem (MANTIS_UPLOAD_DIR)"
            } else {
                "Database-backed"
            },
        ),
        (
            "SendGrid Email Dispatcher",
            if runtime.sendgrid_enabled {
                "Enabled"
            } else {
                "Disabled (DB Queue Only)"
            },
        ),
        (
            "Jira Cloud Integration",
            if runtime.jira_enabled {
                "Enabled"
            } else {
                "Disabled"
            },
        ),
        (
            "MCP Query Token Auth (MANTIS_ALLOW_QUERY_TOKEN)",
            if runtime.query_token_enabled {
                "Enabled"
            } else {
                "Disabled"
            },
        ),
    ];

    let mut config_rows = String::new();
    for (idx, (k, v)) in runtime_rows_data.iter().enumerate() {
        let row_cls = if idx % 2 == 0 {
            "zebra-odd"
        } else {
            "zebra-even"
        };
        config_rows.push_str(&format!(
            r#"<tr class="{}"><td class="label-col">{}</td><td class="input-col">{}</td></tr>"#,
            row_cls,
            html_escape(k),
            html_escape(v)
        ));
    }

    let content = format!(
        r#"{}
        <div class="section-title"><i class="fa fa-info-circle" aria-hidden="true"></i> Site Information</div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-server" aria-hidden="true"></i> Application &amp; Database Summary</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="form-table" style="margin-bottom:0;">
                            <tr><td class="label-col">rust-mantis-bt Version</td><td class="input-col"><strong>{}</strong></td></tr>
                            <tr><td class="label-col">Database Engine</td><td class="input-col">SQLite (rusqlite bundled)</td></tr>
                            <tr><td class="label-col">Projects</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Users</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Issues</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Bugnotes</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Attachments</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Registered Plugins</td><td class="input-col">{}</td></tr>
                            <tr><td class="label-col">Pending Email Queue</td><td class="input-col">{}</td></tr>
                        </table>
                    </div>
                </div>
            </div>
            <div class="widget-box widget-color-blue2">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-sliders" aria-hidden="true"></i> Runtime Configuration Status</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="form-table" style="margin-bottom:0;">
                            {}
                        </table>
                    </div>
                </div>
            </div>
        </div>"#,
        err_banner,
        html_escape(env!("CARGO_PKG_VERSION")),
        stats.projects_count,
        stats.users_count,
        stats.issues_count,
        stats.bugnotes_count,
        stats.attachments_count,
        stats.plugins_count,
        stats.pending_emails_count,
        config_rows
    );

    layout::render_layout("Manage Overview", ctx, None, &content)
}

#[cfg(test)]
#[path = "manage_test.rs"]
mod tests;
