use crate::auth::AuthContext;
use crate::models::{Category, CustomField, CustomFieldType, Project, ProjectVersion};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomFieldFormMode {
    Report,
    Update,
}

pub fn render_custom_field_input(
    cf: &CustomField,
    current_value: &str,
    required: bool,
    read_only: bool,
    versions: &[ProjectVersion],
    categories: &[Category],
) -> String {
    if read_only {
        return format!(
            r#"<span class="readonly-custom-field">{}</span>"#,
            html_escape(current_value)
        );
    }

    let req_attr = if required { " required" } else { "" };

    match CustomFieldType::from_i32(cf.field_type) {
        Some(CustomFieldType::String) => {
            let min_attr = if cf.length_min > 0 {
                format!(r#" minlength="{}""#, cf.length_min)
            } else {
                String::new()
            };
            let max_attr = if cf.length_max > 0 {
                format!(r#" maxlength="{}""#, cf.length_max)
            } else {
                String::new()
            };
            format!(
                r#"<input type="text" name="custom_field_{}" value="{}"{}{}{}>"#,
                cf.id,
                html_escape(current_value),
                req_attr,
                min_attr,
                max_attr
            )
        }
        Some(CustomFieldType::Numeric) => {
            format!(
                r#"<input type="number" step="1" name="custom_field_{}" value="{}"{}>"#,
                cf.id,
                html_escape(current_value),
                req_attr
            )
        }
        Some(CustomFieldType::Float) => {
            format!(
                r#"<input type="number" step="any" name="custom_field_{}" value="{}"{}>"#,
                cf.id,
                html_escape(current_value),
                req_attr
            )
        }
        Some(CustomFieldType::Enum) | Some(CustomFieldType::List) => {
            let opts =
                crate::models::resolve_possible_values(&cf.possible_values, versions, categories);
            let mut options_html = String::new();
            if !opts.iter().any(|o| o.is_empty()) {
                let sel = if current_value.is_empty() {
                    " selected"
                } else {
                    ""
                };
                options_html.push_str(&format!(r#"<option value=""{}></option>"#, sel));
            }
            for opt in &opts {
                let sel = if current_value == opt {
                    " selected"
                } else {
                    ""
                };
                options_html.push_str(&format!(
                    r#"<option value="{}"{}>{}</option>"#,
                    html_escape(opt),
                    sel,
                    html_escape(opt)
                ));
            }
            format!(
                r#"<select name="custom_field_{}"{}>{}</select>"#,
                cf.id, req_attr, options_html
            )
        }
        Some(CustomFieldType::Email) => {
            format!(
                r#"<input type="email" name="custom_field_{}" value="{}"{}>"#,
                cf.id,
                html_escape(current_value),
                req_attr
            )
        }
        Some(CustomFieldType::Checkbox) => {
            let selected_vals: Vec<&str> = current_value.split('|').collect();
            let opts =
                crate::models::resolve_possible_values(&cf.possible_values, versions, categories);
            let mut boxes_html = format!(
                r#"<input type="hidden" name="custom_field_{}_present" value="1">"#,
                cf.id
            );
            if opts.is_empty() {
                let checked = if current_value == "1" || current_value.eq_ignore_ascii_case("true")
                {
                    " checked"
                } else {
                    ""
                };
                boxes_html.push_str(&format!(
                    r#"<label style="margin-right:8px;"><input type="checkbox" name="custom_field_{}" value="1"{}{}> {}</label>"#,
                    cf.id,
                    checked,
                    req_attr,
                    html_escape(&cf.name)
                ));
            } else {
                for opt in &opts {
                    if opt.is_empty() {
                        continue;
                    }
                    let checked = if selected_vals.contains(&opt.as_str()) {
                        " checked"
                    } else {
                        ""
                    };
                    boxes_html.push_str(&format!(
                        r#"<label style="margin-right:8px;"><input type="checkbox" name="custom_field_{}" value="{}"{}> {}</label>"#,
                        cf.id,
                        html_escape(opt),
                        checked,
                        html_escape(opt)
                    ));
                }
            }
            boxes_html
        }
        Some(CustomFieldType::Multilist) => {
            let selected_vals: Vec<&str> = current_value.split('|').collect();
            let opts =
                crate::models::resolve_possible_values(&cf.possible_values, versions, categories);
            let mut options_html = String::new();
            for opt in &opts {
                if opt.is_empty() {
                    continue;
                }
                let sel = if selected_vals.contains(&opt.as_str()) {
                    " selected"
                } else {
                    ""
                };
                options_html.push_str(&format!(
                    r#"<option value="{}"{}>{}</option>"#,
                    html_escape(opt),
                    sel,
                    html_escape(opt)
                ));
            }
            format!(
                r#"<input type="hidden" name="custom_field_{}_present" value="1"><select name="custom_field_{}" multiple{}>{}</select>"#,
                cf.id, cf.id, req_attr, options_html
            )
        }
        Some(CustomFieldType::Date) => {
            let date_val = if current_value.contains('-') {
                current_value.to_string()
            } else if let Ok(ts) = current_value.parse::<i64>() {
                crate::models::timestamp_to_ymd(ts).unwrap_or_else(|| current_value.to_string())
            } else {
                current_value.to_string()
            };
            format!(
                r#"<input type="date" name="custom_field_{}" value="{}"{}>"#,
                cf.id,
                html_escape(&date_val),
                req_attr
            )
        }
        Some(CustomFieldType::Radio) => {
            let opts =
                crate::models::resolve_possible_values(&cf.possible_values, versions, categories);
            let mut radios_html = String::new();
            for (idx, opt) in opts.iter().enumerate() {
                let checked = if current_value == opt { " checked" } else { "" };
                let opt_req = if required && idx == 0 {
                    " required"
                } else {
                    ""
                };
                radios_html.push_str(&format!(
                    r#"<label style="margin-right:8px;"><input type="radio" name="custom_field_{}" value="{}"{}{}> {}</label>"#,
                    cf.id,
                    html_escape(opt),
                    checked,
                    opt_req,
                    html_escape(opt)
                ));
            }
            radios_html
        }
        Some(CustomFieldType::Textarea) => {
            format!(
                r#"<textarea name="custom_field_{}"{}>{}</textarea>"#,
                cf.id,
                req_attr,
                html_escape(current_value)
            )
        }
        None => format!(
            r#"<input type="text" name="custom_field_{}" value="{}"{}>"#,
            cf.id,
            html_escape(current_value),
            req_attr
        ),
    }
}

pub fn render_custom_field_form_row(
    cf: &CustomField,
    current_value: &str,
    effective_access: i32,
    mode: CustomFieldFormMode,
    versions: &[ProjectVersion],
    categories: &[Category],
) -> Option<String> {
    // 1. Hide custom field if effective access < access_level_r
    if effective_access < cf.access_level_r {
        return None;
    }

    // 2. Check display flag and required flag for the form mode
    let (should_display, is_required) = match mode {
        CustomFieldFormMode::Report => (cf.display_report != 0, cf.require_report != 0),
        CustomFieldFormMode::Update => (cf.display_update != 0, cf.require_update != 0),
    };

    if !should_display {
        return None;
    }

    // 3. Check read/write permission
    let can_write = effective_access >= cf.access_level_rw;

    if can_write {
        let req_star = if is_required { " *" } else { "" };
        let input_html =
            render_custom_field_input(cf, current_value, is_required, false, versions, categories);
        Some(format!(
            r#"<tr>
                <td class="label-col">{}{}</td>
                <td class="input-col">{}</td>
            </tr>"#,
            html_escape(&cf.name),
            req_star,
            input_html
        ))
    } else {
        // Read-only: on report page omit editable input, on update page render as read-only
        match mode {
            CustomFieldFormMode::Report => None,
            CustomFieldFormMode::Update => {
                let input_html =
                    render_custom_field_input(cf, current_value, false, true, versions, categories);
                Some(format!(
                    r#"<tr>
                        <td class="label-col">{}</td>
                        <td class="input-col">{}</td>
                    </tr>"#,
                    html_escape(&cf.name),
                    input_html
                ))
            }
        }
    }
}

pub fn render_custom_fields_form_rows(
    fields: &[CustomField],
    values: &HashMap<i64, String>,
    effective_access: i32,
    mode: CustomFieldFormMode,
    versions: &[ProjectVersion],
    categories: &[Category],
) -> String {
    let mut rows_html = String::new();
    for cf in fields {
        let current_val = values
            .get(&cf.id)
            .map(|s| s.as_str())
            .unwrap_or(&cf.default_value);
        if let Some(row) = render_custom_field_form_row(
            cf,
            current_val,
            effective_access,
            mode,
            versions,
            categories,
        ) {
            rows_html.push_str(&row);
        }
    }
    rows_html
}

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
                            <td class="label-col">Access Levels</td>
                            <td class="input-col">
                                <label>Read: <input type="number" name="access_level_r" value="10" style="max-width:80px;"></label>
                                <label style="margin-left:12px;">Write: <input type="number" name="access_level_rw" value="10" style="max-width:80px;"></label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="display_report" value="1" checked> Display When Reporting</label><br>
                                <label><input type="checkbox" name="display_update" value="1" checked> Display When Updating</label><br>
                                <label><input type="checkbox" name="display_resolved" value="1"> Display When Resolving</label><br>
                                <label><input type="checkbox" name="display_closed" value="1"> Display When Closing</label><br>
                                <label><input type="checkbox" name="require_report" value="1"> Required On Report</label><br>
                                <label><input type="checkbox" name="require_update" value="1"> Required On Update</label><br>
                                <label><input type="checkbox" name="require_resolved" value="1"> Required On Resolving</label><br>
                                <label><input type="checkbox" name="require_closed" value="1"> Required On Closing</label>
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
                            <td class="label-col">Access Levels</td>
                            <td class="input-col">
                                <label>Read: <input type="number" name="access_level_r" value="{}" style="max-width:80px;"></label>
                                <label style="margin-left:12px;">Write: <input type="number" name="access_level_rw" value="{}" style="max-width:80px;"></label>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col">
                                <label><input type="checkbox" name="display_report" value="1" {}> Display When Reporting</label><br>
                                <label><input type="checkbox" name="display_update" value="1" {}> Display When Updating</label><br>
                                <label><input type="checkbox" name="display_resolved" value="1" {}> Display When Resolving</label><br>
                                <label><input type="checkbox" name="display_closed" value="1" {}> Display When Closing</label><br>
                                <label><input type="checkbox" name="require_report" value="1" {}> Required On Report</label><br>
                                <label><input type="checkbox" name="require_update" value="1" {}> Required On Update</label><br>
                                <label><input type="checkbox" name="require_resolved" value="1" {}> Required On Resolving</label><br>
                                <label><input type="checkbox" name="require_closed" value="1" {}> Required On Closing</label>
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
        cf.access_level_r,
        cf.access_level_rw,
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
        if cf.display_resolved != 0 {
            "checked"
        } else {
            ""
        },
        if cf.display_closed != 0 {
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
        if cf.require_resolved != 0 {
            "checked"
        } else {
            ""
        },
        if cf.require_closed != 0 {
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
