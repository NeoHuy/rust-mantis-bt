use crate::auth::AuthContext;
use crate::models::{CustomField, CustomFieldType, Project};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

pub fn render_manage_custom_field_page(
    ctx: &AuthContext,
    fields: &[(CustomField, usize)],
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    if fields.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="5" style="text-align:center; padding:12px;">No custom fields defined.</td></tr>"#,
        );
    } else {
        for (idx, (cf, linked_count)) in fields.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let ftype = CustomFieldType::from_i32(cf.field_type)
                .map(|t| t.as_str())
                .unwrap_or("string");
            rows_html.push_str(&format!(
                r#"<tr class="{}">
                    <td><a href="/manage_custom_field_edit_page/{}"><strong>{}</strong></a></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                row_class,
                cf.id,
                html_escape(&cf.name),
                linked_count,
                html_escape(ftype),
                html_escape(&cf.possible_values),
                html_escape(&cf.default_value)
            ));
        }
    }

    let content = format!(
        r#"{}
        <div class="panel">
            <div class="panel-header">Custom Fields ({})</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Linked Projects</th>
                            <th>Type</th>
                            <th>Possible Values</th>
                            <th>Default Value</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </div>
        <div class="panel">
            <div class="panel-header">New Custom Field</div>
            <div class="panel-body">
                <form method="POST" action="/manage_custom_field_create">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Name *</td>
                            <td class="input-col"><input type="text" name="name" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Type</td>
                            <td class="input-col">
                                <select name="field_type">
                                    <option value="0">String</option>
                                    <option value="1">Numeric</option>
                                    <option value="2">Float</option>
                                    <option value="3">Enumeration</option>
                                    <option value="4">Email</option>
                                    <option value="5">Checkbox</option>
                                    <option value="6">List</option>
                                    <option value="7">Multiselection list</option>
                                    <option value="8">Date</option>
                                    <option value="9">Radio</option>
                                    <option value="10">Textarea</option>
                                </select>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Possible Values (pipe | separated)</td>
                            <td class="input-col"><input type="text" name="possible_values"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Default Value</td>
                            <td class="input-col"><input type="text" name="default_value"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Min / Max Length</td>
                            <td class="input-col">
                                <input type="text" name="length_min" value="0" style="max-width:80px;">
                                <input type="text" name="length_max" value="0" style="max-width:80px;">
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="display_report" value="1" checked> Display When Reporting</label><br>
                                <label><input type="checkbox" name="display_update" value="1" checked> Display When Updating</label><br>
                                <label><input type="checkbox" name="require_report" value="1"> Required On Report</label><br>
                                <label><input type="checkbox" name="require_update" value="1"> Required On Update</label>
                            </td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Create Custom Field</button>
                </form>
            </div>
        </div>"#,
        err_banner,
        fields.len(),
        rows_html
    );

    render_layout("Manage Custom Fields", ctx, msg, &content)
}

pub fn render_manage_custom_field_edit_page(
    ctx: &AuthContext,
    cf: &CustomField,
    linked: &[(Project, i32)],
    msg: Option<&str>,
    _err: Option<&str>,
) -> Html<String> {
    let mut linked_rows = String::new();
    if linked.is_empty() {
        linked_rows.push_str(r#"<tr><td colspan="3">Not linked to any projects.</td></tr>"#);
    } else {
        for (p, seq) in linked {
            linked_rows.push_str(&format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td>
                        <form method="POST" action="/manage_custom_field_proj_remove/{}/{}" style="display:inline;">
                            <button type="submit" class="btn btn-danger btn-sm">Unlink</button>
                        </form>
                    </td>
                </tr>"#,
                html_escape(&p.name),
                seq,
                cf.id,
                p.id
            ));
        }
    }

    let mut proj_opts = String::new();
    for (p, _) in &ctx.visible_projects {
        proj_opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            p.id,
            html_escape(&p.name)
        ));
    }

    let content = format!(
        r#"<div class="panel">
            <div class="panel-header">Edit Custom Field: {}</div>
            <div class="panel-body">
                <form method="POST" action="/manage_custom_field_update/{}">
                    <table class="form-table">
                        <tr>
                            <td class="label-col">Name *</td>
                            <td class="input-col"><input type="text" name="name" value="{}" required></td>
                        </tr>
                        <tr>
                            <td class="label-col">Type</td>
                            <td class="input-col"><input type="text" name="field_type" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Possible Values</td>
                            <td class="input-col"><input type="text" name="possible_values" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Default Value</td>
                            <td class="input-col"><input type="text" name="default_value" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Min / Max Length</td>
                            <td class="input-col">
                                <input type="text" name="length_min" value="{}" style="max-width:80px;">
                                <input type="text" name="length_max" value="{}" style="max-width:80px;">
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="display_report" value="1" {}> Display When Reporting</label><br>
                                <label><input type="checkbox" name="display_update" value="1" {}> Display When Updating</label><br>
                                <label><input type="checkbox" name="require_report" value="1" {}> Required On Report</label><br>
                                <label><input type="checkbox" name="require_update" value="1" {}> Required On Update</label>
                            </td>
                        </tr>
                    </table>
                    <button type="submit" class="btn">Update Custom Field</button>
                </form>
                <form method="POST" action="/manage_custom_field_delete/{}" style="margin-top:10px;">
                    <button type="submit" class="btn btn-danger">Delete Custom Field</button>
                </form>
            </div>
        </div>
        <div class="panel">
            <div class="panel-header">Linked Projects</div>
            <div class="panel-body">
                <table class="data-table">
                    <thead><tr><th>Project</th><th>Sequence</th><th>Actions</th></tr></thead>
                    <tbody>{}</tbody>
                </table>
                <form method="POST" action="/manage_custom_field_proj_add/{}" style="display:flex; gap:8px; align-items:center;">
                    <select name="project_id" style="width:auto;">{}</select>
                    <input type="text" name="sequence" value="0" style="width:70px;" placeholder="Seq">
                    <button type="submit" class="btn btn-secondary btn-sm">Link Project</button>
                </form>
            </div>
        </div>"#,
        html_escape(&cf.name),
        cf.id,
        html_escape(&cf.name),
        cf.field_type,
        html_escape(&cf.possible_values),
        html_escape(&cf.default_value),
        cf.length_min,
        cf.length_max,
        if cf.display_report != 0 {
            "checked"
        } else {
            ""
        },
        if cf.display_update != 0 {
            "checked"
        } else {
            ""
        },
        if cf.require_report != 0 {
            "checked"
        } else {
            ""
        },
        if cf.require_update != 0 {
            "checked"
        } else {
            ""
        },
        cf.id,
        linked_rows,
        cf.id,
        proj_opts
    );

    render_layout("Edit Custom Field", ctx, msg, &content)
}

#[cfg(test)]
#[path = "custom_fields_test.rs"]
mod tests;
