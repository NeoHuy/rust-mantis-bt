use crate::auth::AuthContext;
use crate::models::{ApiToken, PluginItem};
use crate::views::layout::render_layout;
use crate::views::{format_timestamp, html_escape};
use axum::response::Html;

pub fn render_api_tokens_page(
    ctx: &AuthContext,
    tokens: &[ApiToken],
    raw_token: Option<&str>,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let token_banner = match raw_token {
        Some(raw) if !raw.is_empty() => format!(
            r#"<div class="flash-message">
                <strong>New API Token Created!</strong> Copy it now — it will not be shown again:<br>
                <code style="display:inline-block; margin-top:6px; padding:6px 10px; background:#fff; border:1px solid #b2dba1; font-weight:bold; font-size:13px;">{}</code>
            </div>"#,
            html_escape(raw)
        ),
        _ => String::new(),
    };

    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    for t in tokens {
        let used_str = if t.date_used > 0 {
            format!("Used ({})", format_timestamp(t.date_used))
        } else {
            "Never used".to_string()
        };
        rows_html.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>
                    <form method="POST" action="/api_token_revoke/{}" style="display:inline;">
                        <button type="submit" class="btn btn-danger btn-sm">Revoke</button>
                    </form>
                </td>
            </tr>"#,
            t.id,
            html_escape(&t.name),
            format_timestamp(t.date_created),
            used_str,
            t.id
        ));
    }
    if rows_html.is_empty() {
        rows_html.push_str(r#"<tr><td colspan="5">No API tokens created yet.</td></tr>"#);
    }

    let content = format!(
        r#"{}
        {}
        <div class="panel">
            <div class="panel-header">Create API Token</div>
            <div class="panel-body">
                <form method="POST" action="/api_token_create" style="display:flex; gap:8px; align-items:center;">
                    <input type="text" name="name" placeholder="Token Name (e.g. CI/CD, CLI)" required style="max-width:300px;">
                    <button type="submit" class="btn">Create API Token</button>
                </form>
            </div>
        </div>
        <div class="panel">
            <div class="panel-header">My API Tokens ({})</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Token Name</th>
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
        </div>"#,
        err_banner,
        token_banner,
        tokens.len(),
        rows_html
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
        let action_html = if p.protected != 0 {
            "<em>Protected</em>".to_string()
        } else {
            let next_enabled = if p.enabled != 0 { "0" } else { "1" };
            let btn_label = if p.enabled != 0 { "Disable" } else { "Enable" };
            format!(
                r#"<form method="POST" action="/manage_plugin_toggle/{}" style="display:inline;">
                    <input type="hidden" name="enabled" value="{}">
                    <button type="submit" class="btn btn-secondary btn-sm">{}</button>
                </form>"#,
                html_escape(&p.basename),
                next_enabled,
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
        <div class="panel">
            <div class="panel-header">Installed Plugins ({}) — EventAuditLog Dispatched Events: {}</div>
            <div class="panel-body">
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
        </div>"#,
        err_banner,
        plugins.len(),
        html_escape(audit_count),
        rows_html
    );

    render_layout("Manage Plugins", ctx, msg, &content)
}
