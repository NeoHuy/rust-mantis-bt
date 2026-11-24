use crate::auth::AuthContext;
use crate::models::Tag;
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

pub fn render_manage_tags_page(
    ctx: &AuthContext,
    tags: &[(Tag, i64)],
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    if tags.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="4" style="text-align:center; padding:12px;">No tags defined.</td></tr>"#,
        );
    } else {
        for (idx, (t, count)) in tags.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let actions = if ctx.user.access_level >= 70 {
                format!(
                    r#"<form method="POST" action="/tag_delete/{}" style="display:inline;" onsubmit="return confirm('Delete tag?');"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    t.id
                )
            } else {
                String::new()
            };
            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><a href="/tag_view_page/{}"><strong>{}</strong></a></td>
                    <td>{}</td>
                    <td><a href="/view_all_bug_page?tag_id={}">{}</a></td>
                    <td>{}</td>
                </tr>"#,
                row_class,
                t.id,
                html_escape(&t.name),
                html_escape(&t.description),
                t.id,
                count,
                actions
            ));
        }
    }

    let create_form = if ctx.user.access_level >= 25 {
        r#"<div class="panel">
            <div class="panel-header">Create Tag</div>
            <div class="panel-body">
                <form method="POST" action="/tag_create">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Tag Name *</td>
                            <td class="input-col"><input type="text" name="name" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Description</td>
                            <td class="input-col"><textarea name="description"></textarea></td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Create Tag</button>
                </form>
            </div>
        </div>"#
    } else {
        ""
    };

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Manage Tags ({})</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Description</th>
                            <th>Issues</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>
        {}"#,
        err_banner,
        tags.len(),
        rows_html,
        create_form
    );

    render_layout("Manage Tags", ctx, msg, &content)
}

pub fn render_tag_view_page(
    ctx: &AuthContext,
    tag: &Tag,
    creator_username: &str,
    count: i64,
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let edit_form = if ctx.user.access_level >= 70 {
        format!(
            r#"<div class="panel">
                <div class="panel-header">Edit Tag</div>
                <div class="panel-body">
                    <form method="POST" action="/tag_update/{}">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Tag Name *</td>
                                <td class="input-col"><input type="text" name="name" value="{}" required></td>
                            </tr>
                            <tr>
                                <td class="label-col">Description</td>
                                <td class="input-col"><textarea name="description">{}</textarea></td>
                            </tr>
                        </table>
                        <div style="margin-top:14px;">
                            <button type="submit" class="btn">Update Tag</button>
                        </div>
                    </form>
                </div>
            </div>"#,
            tag.id,
            html_escape(&tag.name),
            html_escape(&tag.description)
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"{err_banner}
        <div class="panel">
            <div class="panel-header">Tag: {name}</div>
            <div class="panel-body">
                <table class="form-table">
                    <tr>
                        <td class="label-col">Tag Name</td>
                        <td class="input-col"><strong>{name}</strong></td>
                    </tr>
                    <tr>
                        <td class="label-col">Description</td>
                        <td class="input-col">{desc}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Created By</td>
                        <td class="input-col">{creator}</td>
                    </tr>
                    <tr>
                        <td class="label-col">Attached Issues</td>
                        <td class="input-col">
                            <a href="/view_all_bug_page?tag_id={tag_id}" class="btn btn-secondary btn-sm">{count} issue(s) &nearr;</a>
                        </td>
                    </tr>
                </table>
                <div style="margin-top:14px; display:flex; gap:8px;">
                    <a href="/manage_tags_page" class="btn btn-secondary">Back to Tags</a>
                    <a href="/view_all_bug_page?tag_id={tag_id}" class="btn btn-secondary">View Attached Issues</a>
                </div>
            </div>
        </div>
        {edit_form}"#,
        err_banner = err_banner,
        name = html_escape(&tag.name),
        desc = html_escape(&tag.description),
        creator = html_escape(creator_username),
        count = count,
        tag_id = tag.id,
        edit_form = edit_form
    );

    render_layout("View Tag", ctx, msg, &content)
}

#[cfg(test)]
#[path = "tags_test.rs"]
mod tests;
