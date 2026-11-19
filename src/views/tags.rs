use crate::auth::AuthContext;
use crate::models::Tag;
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

pub fn render_manage_tags_page(
    ctx: &AuthContext,
    tags: &[Tag],
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    if tags.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="3" style="text-align:center; padding:12px;">No tags defined.</td></tr>"#,
        );
    } else {
        for (idx, t) in tags.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let actions = if ctx.user.access_level >= 70 {
                format!(
                    r#"<form method="POST" action="/tag_delete/{}" style="display:inline;"><button type="submit" class="btn btn-danger btn-sm">Delete</button></form>"#,
                    t.id
                )
            } else {
                String::new()
            };
            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><a href="/view_all_bug_page?tag_id={}"><strong>{}</strong></a></td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                row_class,
                t.id,
                html_escape(&t.name),
                html_escape(&t.description),
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

#[cfg(test)]
#[path = "tags_test.rs"]
mod tests;
