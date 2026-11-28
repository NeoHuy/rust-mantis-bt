use crate::auth::AuthContext;
use crate::models::{ApiToken, PluginItem};
use crate::views::layout::render_layout;
use crate::views::{format_timestamp, html_escape};
use axum::response::Html;

pub fn render_api_tokens_page(
    ctx: &AuthContext,
    tokens: &[ApiToken],
    all_tokens: &[(ApiToken, String, String)],
    raw_token: Option<&str>,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let token_banner = match raw_token {
        Some(raw) if !raw.is_empty() => format!(
            r#"<div class="flash-message">
                <strong>New API Token Created!</strong> Copy it now — it will not be shown again:
                <div class="flex flex-wrap items-center gap-2 mt-2">
                    <code id="new-api-token" class="inline-block px-2.5 py-1.5 bg-white border border-[#b2dba1] font-bold text-[13px] select-all">{}</code>
                    <button type="button" class="btn btn-white btn-round btn-sm" data-copy-target="new-api-token"><i class="fa fa-copy" aria-hidden="true"></i> Copy Token</button>
                </div>
            </div>"#,
            html_escape(raw)
        ),
        _ => String::new(),
    };

    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    for (idx, t) in tokens.iter().enumerate() {
        let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
        let used_str = if t.date_used > 0 {
            format!("Used ({})", format_timestamp(t.date_used))
        } else {
            "Never used".to_string()
        };
        let scope_badge = if t.scope == "rss" {
            r#"<span class="badge badge-info">RSS (read-only)</span>"#
        } else {
            r#"<span class="badge badge-warning">API (full)</span>"#
        };
        rows_html.push_str(&format!(
            r#"<tr class="{}">
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <form method="POST" action="/api_token_revoke/{}" class="inline-block m-0" data-confirm="Revoke this API token?">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Revoke</button>
                    </form>
                </td>
            </tr>"#,
            row_cls,
            t.id,
            html_escape(&t.name),
            scope_badge,
            format_timestamp(t.date_created),
            used_str,
            t.id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="6" class="text-center py-3 text-gray-500">No API tokens created yet.</td></tr>"#);
    }

    let admin_table = if ctx.user.access_level >= 90 && !all_tokens.is_empty() {
        let mut admin_rows = String::new();
        for (idx, (t, username, realname)) in all_tokens.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let used_str = if t.date_used > 0 {
                format!("Used ({})", format_timestamp(t.date_used))
            } else {
                "Never used".to_string()
            };
            let display_user = if realname.is_empty() {
                html_escape(username)
            } else {
                format!("{} ({})", html_escape(realname), html_escape(username))
            };
            let scope_badge = if t.scope == "rss" {
                r#"<span class="badge badge-info">RSS (read-only)</span>"#
            } else {
                r#"<span class="badge badge-warning">API (full)</span>"#
            };
            admin_rows.push_str(&format!(
                r#"<tr class="{}">
                    <td>{}</td>
                    <td><strong>{}</strong></td>
                    <td><strong>{}</strong></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>
                        <form method="POST" action="/api_token_revoke/{}" class="inline-block m-0" data-confirm="Revoke this user API token?">
                            <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Revoke</button>
                        </form>
                    </td>
                </tr>"#,
                row_cls,
                t.id,
                display_user,
                html_escape(&t.name),
                scope_badge,
                format_timestamp(t.date_created),
                used_str,
                t.id
            ));
        }
        if admin_rows.is_empty() {
            admin_rows.push_str(r#"<tr><td colspan="7" class="text-center py-3 text-gray-500">No tokens found in the system.</td></tr>"#);
        }
        format!(
            r#"<div class="widget-box widget-color-blue2 mt-5">
                <div class="widget-header">
                    <h4 class="widget-title"><i class="fa fa-shield" aria-hidden="true"></i> System-Wide API Tokens (Administrator View) ({})</h4>
                </div>
                <div class="widget-body">
                    <div class="table-responsive">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>ID</th>
                                    <th>Owner User</th>
                                    <th>Token Name</th>
                                    <th>Scope</th>
                                    <th>Created</th>
                                    <th>Last Used</th>
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
            all_tokens.len(),
            admin_rows
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{}
        {}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-plus-circle" aria-hidden="true"></i> Create API Token</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/api_token_create" class="flex flex-wrap items-center gap-2">
                    <input type="text" name="name" placeholder="Token Name (e.g. CI/CD, CLI)" required class="!max-w-[300px]">
                    <select name="scope" class="!w-auto" aria-label="Token Scope">
                        <option value="api" selected>API (Full Access)</option>
                        <option value="rss">RSS (Read-Only Feed Token)</option>
                    </select>
                    <button type="submit" class="btn btn-primary btn-sm"><i class="fa fa-key" aria-hidden="true"></i> Create API Token</button>
                </form>
            </div>
        </div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-key" aria-hidden="true"></i> My API Tokens ({})</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>ID</th>
                                <th>Token Name</th>
                                <th>Scope</th>
                                <th>Created</th>
                                <th>Last Used</th>
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
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-rss" aria-hidden="true"></i> Authenticated RSS Feeds</h4>
            </div>
            <div class="widget-body text-xs space-y-1.5">
                <p>Use an <strong>RSS (Read-Only)</strong> feed token via <code>?token=&lt;RSS_TOKEN&gt;</code> or <code>?username=&lt;USER&gt;&amp;key=&lt;RSS_TOKEN&gt;</code>, or send any active token in <code>Authorization: Bearer &lt;TOKEN&gt;</code>. Full-access API tokens are rejected in RSS query parameters unless <code>MANTIS_ALLOW_QUERY_TOKEN=1</code> is set.</p>
                <div class="flex flex-wrap gap-2">
                    <a href="/issues_rss" class="btn btn-white btn-round btn-sm"><i class="fa fa-rss text-orange-600" aria-hidden="true"></i> Issues RSS</a>
                    <a href="/news_rss" class="btn btn-white btn-round btn-sm"><i class="fa fa-rss text-orange-600" aria-hidden="true"></i> News RSS</a>
                </div>
            </div>
        </div>
        {}"#,
        err_banner,
        token_banner,
        tokens.len(),
        rows_html,
        admin_table
    );

    render_layout("API Tokens", ctx, msg, &content)
}

pub fn render_manage_plugin_page(
    ctx: &AuthContext,
    plugins: &[PluginItem],
    audit_count: &str,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    for p in plugins {
        let status_badge = if p.enabled != 0 {
            r#"<span class="badge badge-success">Enabled</span>"#
        } else {
            r#"<span class="badge badge-warning">Disabled</span>"#
        };
        let prot_str = if p.protected != 0 { "Yes" } else { "No" };
        let is_toggleable = p.protected == 0
            || crate::models::TOGGLEABLE_CORE_PLUGINS.contains(&p.basename.as_str());
        let action_html = if !is_toggleable {
            "<em>Protected</em>".to_string()
        } else {
            let next_enabled = if p.enabled != 0 { "0" } else { "1" };
            let btn_label = if p.enabled != 0 { "Disable" } else { "Enable" };
            let btn_cls = if p.enabled != 0 {
                "btn btn-secondary btn-sm"
            } else {
                "btn btn-primary btn-sm"
            };
            format!(
                r#"<form method="POST" action="/manage_plugin_toggle/{}" class="inline-block m-0">
                    <input type="hidden" name="enabled" value="{}">
                    <button type="submit" class="{}">{}</button>
                </form>"#,
                html_escape(&p.basename),
                next_enabled,
                btn_cls,
                btn_label
            )
        };

        rows_html.push_str(&format!(
            r#"<tr>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            html_escape(&p.basename),
            status_badge,
            prot_str,
            p.priority,
            action_html
        ));
    }

    let content = format!(
        r#"{}
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-plug" aria-hidden="true"></i> Installed Plugins ({}) — EventAuditLog Dispatched Events: {}</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>Plugin Basename</th>
                                <th>Status</th>
                                <th>Protected</th>
                                <th>Priority</th>
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
        err_banner,
        plugins.len(),
        html_escape(audit_count),
        rows_html
    );

    render_layout("Manage Plugins", ctx, msg, &content)
}

#[cfg(test)]
#[path = "api_test.rs"]
mod tests;
