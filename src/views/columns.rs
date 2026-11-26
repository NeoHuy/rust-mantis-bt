use crate::auth::AuthContext;
use crate::models::{custom_field_column_id, CustomField, STANDARD_COLUMNS};
use crate::views::html_escape;
use crate::views::layout::render_layout;
use axum::response::Html;

#[allow(clippy::too_many_arguments)]
pub fn render_manage_columns_page(
    ctx: &AuthContext,
    selected_project_id: i64,
    accessible_custom_fields: &[CustomField],
    view_cols: &[String],
    print_cols: &[String],
    csv_cols: &[String],
    excel_cols: &[String],
    msg: Option<&str>,
    err: Option<&str>,
) -> Html<String> {
    let err_banner = err
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut proj_opts = format!(
        r#"<option value="0" {}>All Projects (My Default)</option>"#,
        if selected_project_id == 0 {
            "selected"
        } else {
            ""
        }
    );
    for (proj, depth) in &ctx.visible_projects {
        let sel = if proj.id == selected_project_id {
            "selected"
        } else {
            ""
        };
        let indent = "&nbsp;&nbsp;".repeat(*depth);
        let prefix = if *depth > 0 { "» " } else { "" };
        proj_opts.push_str(&format!(
            r#"<option value="{}" {}>{}{}{}</option>"#,
            proj.id,
            sel,
            indent,
            prefix,
            html_escape(&proj.name)
        ));
    }

    let mut available_chips = String::new();
    let mut all_ids_preview = Vec::new();
    for (id, label) in STANDARD_COLUMNS {
        all_ids_preview.push((*id).to_string());
        available_chips.push_str(&format!(
            r#"<span class="reminder-badge" title="{}"><code>{}</code> <span class="text-gray-600">({})</span></span>"#,
            html_escape(label),
            html_escape(id),
            html_escape(label)
        ));
    }
    for cf in accessible_custom_fields {
        let cf_col = custom_field_column_id(cf.id);
        all_ids_preview.push(cf_col.clone());
        available_chips.push_str(&format!(
            r#"<span class="reminder-badge" title="Custom Field"><code>{}</code> <span class="text-gray-600">({})</span></span>"#,
            html_escape(&cf_col),
            html_escape(&cf.name)
        ));
    }

    let content = format!(
        r#"{}
        <div class="widget-box widget-color-blue2 mb-4">
            <div class="widget-header widget-header-small">
                <h4 class="widget-title lighter">
                    <i class="fa fa-columns" aria-hidden="true"></i> Manage Columns
                </h4>
                <div class="widget-toolbar">
                    <a href="/view_all_bug_page" class="btn btn-white btn-round btn-sm">
                        <i class="fa fa-list-alt" aria-hidden="true"></i> Back to View Issues
                    </a>
                </div>
            </div>
            <div class="widget-body">
                <div class="mb-3 pb-3 border-b border-gray-200 flex flex-wrap items-center gap-3">
                    <label for="columns_project_select" class="text-xs font-bold text-[#336199]">Project Scope:</label>
                    <select id="columns_project_select" data-reload-url="/account_manage_columns_page" class="select-auto">
                        {}
                    </select>
                    <span class="text-xs text-gray-600">Configure column order as comma-separated IDs per output context.</span>
                </div>

                <div class="mb-4 p-3 bg-[#f9fbfc] border border-[#dcebf7] rounded-sm">
                    <div class="text-xs font-bold text-[#336199] mb-2">Available Column IDs</div>
                    <div class="flex flex-wrap gap-1.5 mb-2">{}</div>
                    <textarea readonly rows="2" class="w-full text-xs bg-white text-gray-700" aria-label="All available column IDs">{}</textarea>
                </div>

                <form id="manage-columns-form" method="POST" action="/account_manage_columns_update">
                    <input type="hidden" name="project_id" value="{}">
                    <table class="form-table form-table-left">
                        <tr>
                            <td class="label-col">View Issues Columns</td>
                            <td class="input-col">
                                <textarea name="view_issues_columns" id="view_issues_columns" rows="3" class="w-full text-xs" required>{}</textarea>
                                <div class="text-xs text-gray-500 mt-1">Displayed in the View Issues table (<code>#buglist</code>).</div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Print Issues Columns</td>
                            <td class="input-col">
                                <textarea name="print_issues_columns" id="print_issues_columns" rows="3" class="w-full text-xs" required>{}</textarea>
                                <div class="text-xs text-gray-500 mt-1">Included in the printer-friendly View Issues report.</div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">CSV Export Columns</td>
                            <td class="input-col">
                                <textarea name="csv_columns" id="csv_columns" rows="3" class="w-full text-xs" required>{}</textarea>
                                <div class="text-xs text-gray-500 mt-1">Included in <code>/csv_export</code> downloads.</div>
                            </td>
                        </tr>
                        <tr>
                            <td class="label-col">Excel Export Columns</td>
                            <td class="input-col">
                                <textarea name="excel_columns" id="excel_columns" rows="3" class="w-full text-xs" required>{}</textarea>
                                <div class="text-xs text-gray-500 mt-1">Included in Excel SpreadsheetML XML exports.</div>
                            </td>
                        </tr>
                    </table>
                    <div class="mt-3 flex flex-wrap items-center gap-2">
                        <button type="submit" class="btn btn-primary btn-sm">
                            <i class="fa fa-save" aria-hidden="true"></i> Update Columns
                        </button>
                    </div>
                </form>

                <div class="mt-3 pt-3 border-t border-gray-200">
                    <form method="POST" action="/account_manage_columns_reset" class="inline-form">
                        <input type="hidden" name="project_id" value="{}">
                        <button type="submit" class="btn btn-white btn-round btn-sm" data-confirm="Reset all column settings for this project scope to defaults?">
                            <i class="fa fa-undo" aria-hidden="true"></i> Reset to Defaults
                        </button>
                    </form>
                </div>
            </div>
        </div>"#,
        err_banner,
        proj_opts,
        available_chips,
        html_escape(&all_ids_preview.join(", ")),
        selected_project_id,
        html_escape(&view_cols.join(", ")),
        html_escape(&print_cols.join(", ")),
        html_escape(&csv_cols.join(", ")),
        html_escape(&excel_cols.join(", ")),
        selected_project_id,
    );

    render_layout("Manage Columns", ctx, msg, &content)
}

#[cfg(test)]
#[path = "columns_test.rs"]
mod tests;
