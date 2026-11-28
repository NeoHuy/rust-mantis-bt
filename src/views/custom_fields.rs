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
    Resolve,
    Close,
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
        let display_val = match CustomFieldType::from_i32(cf.field_type) {
            Some(CustomFieldType::Checkbox) | Some(CustomFieldType::Multilist) => {
                current_value.replace('|', ", ")
            }
            _ => current_value.to_string(),
        };
        return format!(
            r#"<span class="readonly-custom-field">{}</span>"#,
            html_escape(&display_val)
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
            let sel = if current_value.is_empty() {
                " selected"
            } else {
                ""
            };
            options_html.push_str(&format!(r#"<option value=""{}></option>"#, sel));
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
                    r#"<label class="inline-flex items-center mr-2 cursor-pointer"><input type="checkbox" name="custom_field_{}" value="1"{}{}> {}</label>"#,
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
                        r#"<label class="inline-flex items-center mr-2 cursor-pointer"><input type="checkbox" name="custom_field_{}" value="{}"{}> {}</label>"#,
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
            let mut radios_html = format!(
                r#"<input type="hidden" name="custom_field_{}_present" value="1">"#,
                cf.id
            );
            for (idx, opt) in opts.iter().enumerate() {
                let checked = if current_value == opt { " checked" } else { "" };
                let opt_req = if required && idx == 0 {
                    " required"
                } else {
                    ""
                };
                radios_html.push_str(&format!(
                    r#"<label class="inline-flex items-center mr-2 cursor-pointer"><input type="radio" name="custom_field_{}" value="{}"{}{}> {}</label>"#,
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
        CustomFieldFormMode::Resolve => (cf.display_resolved != 0, cf.require_resolved != 0),
        CustomFieldFormMode::Close => (cf.display_closed != 0, cf.require_closed != 0),
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
        // Read-only: on report page omit editable input, on other pages render as read-only
        match mode {
            CustomFieldFormMode::Report => None,
            CustomFieldFormMode::Update
            | CustomFieldFormMode::Resolve
            | CustomFieldFormMode::Close => {
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

fn render_custom_field_type_options(selected: i32) -> String {
    let type_ids = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut out = String::new();
    let mut matched = false;
    for val in type_ids {
        let label = CustomFieldType::from_i32(val)
            .map(|t| t.as_str())
            .unwrap_or("Unknown");
        let sel = if val == selected {
            matched = true;
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{}"{}>{}</option>"#,
            val, sel, label
        ));
    }
    if !matched {
        out.push_str(&format!(
            r#"<option value="{}" selected>Unknown ({})</option>"#,
            selected, selected
        ));
    }
    out
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
            r#"<tr><td colspan="5" class="text-center py-3 text-gray-500">No custom fields defined.</td></tr>"#,
        );
    } else {
        for (idx, (cf, linked_count)) in fields.iter().enumerate() {
            let row_class = if idx % 2 == 0 {
                "zebra-odd"
            } else {
                "zebra-even"
            };
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
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-sliders" aria-hidden="true"></i> Custom Fields ({})</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
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
        </div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-plus-circle" aria-hidden="true"></i> New Custom Field</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/manage_custom_field_create">
                    <div class="table-responsive">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Name <span class="required">*</span></td>
                                <td class="input-col"><input type="text" name="name" required></td>
                            </tr>
                            <tr>
                                <td class="label-col">Type</td>
                                <td class="input-col">
                                    <select name="field_type">
                                        {}
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
                                <div class="flex flex-wrap items-center gap-2">
                                    <input type="text" name="length_min" value="0" class="!w-20" aria-label="Minimum length">
                                    <input type="text" name="length_max" value="0" class="!w-20" aria-label="Maximum length">
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Regular Expression <small class="text-gray-500 font-normal">(metadata)</small></td>
                            <td class="input-col"><input type="text" name="valid_regexp"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Access Levels</td>
                            <td class="input-col">
                                <div class="flex flex-wrap items-center gap-3">
                                    <label class="inline-flex items-center gap-1.5">Read: <input type="number" name="access_level_r" value="10" class="!w-20"></label>
                                    <label class="inline-flex items-center gap-1.5">Write: <input type="number" name="access_level_rw" value="25" class="!w-20"></label>
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col space-y-1">
                                <label class="block cursor-pointer"><input type="checkbox" name="display_report" value="1" checked> Display When Reporting</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_update" value="1" checked> Display When Updating</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_resolved" value="1"> Display When Resolving</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_closed" value="1"> Display When Closing</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_report" value="1"> Required On Report</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_update" value="1"> Required On Update</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_resolved" value="1"> Required On Resolving</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_closed" value="1"> Required On Closing</label>
                            </td>
                        </tr>
                    </table>
                    </div>
                    <button type="submit" class="btn btn-primary mt-3"><i class="fa fa-plus" aria-hidden="true"></i> Create Custom Field</button>
                </form>
            </div>
        </div>"#,
        err_banner,
        fields.len(),
        rows_html,
        render_custom_field_type_options(0)
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
        linked_rows.push_str(r#"<tr><td colspan="3" class="text-center py-3 text-gray-500">Not linked to any projects.</td></tr>"#);
    } else {
        for (idx, (p, seq)) in linked.iter().enumerate() {
            let row_cls = if idx % 2 == 0 { "row-1" } else { "row-2" };
            linked_rows.push_str(&format!(
                r#"<tr class="{}">
                    <td>{}</td>
                    <td>{}</td>
                    <td>
                        <form method="POST" action="/manage_custom_field_proj_remove/{}/{}" class="inline-block m-0" data-confirm="Unlink project from this custom field?">
                            <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-unlink" aria-hidden="true"></i> Unlink</button>
                        </form>
                    </td>
                </tr>"#,
                row_cls,
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
        r#"<div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-pencil" aria-hidden="true"></i> Edit Custom Field: {}</h4>
            </div>
            <div class="widget-body">
                <form method="POST" action="/manage_custom_field_update/{}">
                    <div class="table-responsive">
                        <table class="form-table">
                            <tr>
                                <td class="label-col">Name <span class="required">*</span></td>
                                <td class="input-col"><input type="text" name="name" value="{}" required></td>
                            </tr>
                            <tr>
                                <td class="label-col">Type</td>
                                <td class="input-col">
                                    <select name="field_type">
                                        {}
                                    </select>
                                </td>
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
                                <div class="flex flex-wrap items-center gap-2">
                                    <input type="text" name="length_min" value="{}" class="!w-20" aria-label="Minimum length">
                                    <input type="text" name="length_max" value="{}" class="!w-20" aria-label="Maximum length">
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Regular Expression <small class="text-gray-500 font-normal">(metadata)</small></td>
                            <td class="input-col"><input type="text" name="valid_regexp" value="{}"></td>
                        </tr>
                        <tr>
                            <td class="label-col">Access Levels</td>
                            <td class="input-col">
                                <div class="flex flex-wrap items-center gap-3">
                                    <label class="inline-flex items-center gap-1.5">Read: <input type="number" name="access_level_r" value="{}" class="!w-20"></label>
                                    <label class="inline-flex items-center gap-1.5">Write: <input type="number" name="access_level_rw" value="{}" class="!w-20"></label>
                                </div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Options</td>
                            <td class="input-col space-y-1">
                                <label class="block cursor-pointer"><input type="checkbox" name="display_report" value="1" {}> Display When Reporting</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_update" value="1" {}> Display When Updating</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_resolved" value="1" {}> Display When Resolving</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="display_closed" value="1" {}> Display When Closing</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_report" value="1" {}> Required On Report</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_update" value="1" {}> Required On Update</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_resolved" value="1" {}> Required On Resolving</label>
                                <label class="block cursor-pointer"><input type="checkbox" name="require_closed" value="1" {}> Required On Closing</label>
                            </td>
                        </tr>
                    </table>
                    </div>
                    <div class="mt-3.5 flex flex-wrap gap-2">
                        <button type="submit" class="btn btn-primary"><i class="fa fa-save" aria-hidden="true"></i> Update Custom Field</button>
                        <a href="/manage_custom_field_page" class="btn btn-white btn-round">Back to Custom Fields</a>
                    </div>
                </form>
                <div class="mt-3 pt-3 border-t border-[#e0e6ed]">
                    <form method="POST" action="/manage_custom_field_delete/{}" class="inline-block m-0" data-confirm="Delete this custom field? This cannot be undone.">
                        <button type="submit" class="btn btn-danger btn-white btn-round btn-sm"><i class="fa fa-trash-o" aria-hidden="true"></i> Delete Custom Field</button>
                    </form>
                </div>
            </div>
        </div>
        <div class="widget-box widget-color-blue2">
            <div class="widget-header">
                <h4 class="widget-title"><i class="fa fa-link" aria-hidden="true"></i> Linked Projects</h4>
            </div>
            <div class="widget-body">
                <div class="table-responsive">
                    <table class="data-table">
                        <thead><tr><th>Project</th><th>Sequence</th><th>Actions</th></tr></thead>
                        <tbody>{}</tbody>
                    </table>
                </div>
                <form method="POST" action="/manage_custom_field_proj_add/{}" class="flex flex-wrap gap-2 items-center mt-3">
                    <select name="project_id" class="!w-auto">{}</select>
                    <input type="text" name="sequence" value="0" class="!w-20" placeholder="Seq">
                    <button type="submit" class="btn btn-white btn-round btn-sm"><i class="fa fa-link" aria-hidden="true"></i> Link Project</button>
                </form>
            </div>
        </div>"#,
        html_escape(&cf.name),
        cf.id,
        html_escape(&cf.name),
        render_custom_field_type_options(cf.field_type),
        html_escape(&cf.possible_values),
        html_escape(&cf.default_value),
        cf.length_min,
        cf.length_max,
        html_escape(&cf.valid_regexp),
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
