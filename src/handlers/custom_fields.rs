use crate::auth::AuthContext;
use crate::db::Db;
use crate::handlers::parse_bool;
use crate::models::CustomFieldType;
use crate::ui::{html_escape, render_layout};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

fn forbidden(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Access Denied: {}</div>"#,
        html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

pub async fn manage_custom_field_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    let fields = db.list_custom_fields().unwrap_or_default();
    let err_banner = q
        .err
        .as_deref()
        .map(|e| format!(r#"<div class="flash-error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let mut rows_html = String::new();
    if fields.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="5" style="text-align:center; padding:12px;">No custom fields defined.</td></tr>"#,
        );
    } else {
        for (idx, cf) in fields.iter().enumerate() {
            let row_class = if idx % 2 == 0 { "row-1" } else { "row-2" };
            let ftype = CustomFieldType::from_i32(cf.field_type)
                .map(|t| t.as_str())
                .unwrap_or("string");
            let linked_count = db
                .list_projects_for_custom_field(cf.id)
                .map(|v| v.len())
                .unwrap_or(0);
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

    render_layout("Manage Custom Fields", &ctx, q.msg.as_deref(), &content).into_response()
}

#[derive(Deserialize)]
pub struct CustomFieldForm {
    pub name: String,
    pub field_type: Option<i32>,
    pub possible_values: Option<String>,
    pub default_value: Option<String>,
    pub valid_regexp: Option<String>,
    pub length_min: Option<i32>,
    pub length_max: Option<i32>,
    pub require_report: Option<String>,
    pub require_update: Option<String>,
    pub display_report: Option<String>,
    pub display_update: Option<String>,
}

pub async fn manage_custom_field_create(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<CustomFieldForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    match db.create_custom_field(
        &form.name,
        form.field_type.unwrap_or(0),
        form.possible_values.as_deref().unwrap_or(""),
        form.default_value.as_deref().unwrap_or(""),
        form.valid_regexp.as_deref().unwrap_or(""),
        form.length_min.unwrap_or(0),
        form.length_max.unwrap_or(0),
        parse_bool(form.require_report.as_deref()),
        parse_bool(form.require_update.as_deref()),
        parse_bool(form.display_report.as_deref()),
        parse_bool(form.display_update.as_deref()),
    ) {
        Ok(id) => Redirect::to(&format!(
            "/manage_custom_field_edit_page/{}?msg=Custom+field+created",
            id
        ))
        .into_response(),
        Err(_) => Redirect::to("/manage_custom_field_page?err=Failed+to+create+custom+field")
            .into_response(),
    }
}

pub async fn manage_custom_field_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }

    let cf = match db.get_custom_field_by_id(id) {
        Ok(Some(c)) => c,
        _ => return Redirect::to("/manage_custom_field_page?err=Not+found").into_response(),
    };

    let linked = db.list_projects_for_custom_field(id).unwrap_or_default();
    let mut linked_rows = String::new();
    if linked.is_empty() {
        linked_rows.push_str(r#"<tr><td colspan="3">Not linked to any projects.</td></tr>"#);
    } else {
        for (p, seq) in &linked {
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

    render_layout("Edit Custom Field", &ctx, q.msg.as_deref(), &content).into_response()
}

pub async fn manage_custom_field_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<CustomFieldForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.update_custom_field(
        id,
        &form.name,
        form.field_type.unwrap_or(0),
        form.possible_values.as_deref().unwrap_or(""),
        form.default_value.as_deref().unwrap_or(""),
        form.valid_regexp.as_deref().unwrap_or(""),
        form.length_min.unwrap_or(0),
        form.length_max.unwrap_or(0),
        parse_bool(form.require_report.as_deref()),
        parse_bool(form.require_update.as_deref()),
        parse_bool(form.display_report.as_deref()),
        parse_bool(form.display_update.as_deref()),
    );
    Redirect::to(&format!(
        "/manage_custom_field_edit_page/{}?msg=Updated",
        id
    ))
    .into_response()
}

pub async fn manage_custom_field_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.delete_custom_field(id);
    Redirect::to("/manage_custom_field_page?msg=Custom+field+deleted").into_response()
}

#[derive(Deserialize)]
pub struct LinkProjectForm {
    pub project_id: i64,
    pub sequence: Option<i32>,
}

pub async fn manage_custom_field_proj_add(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<LinkProjectForm>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.link_custom_field_to_project(id, form.project_id, form.sequence.unwrap_or(0));
    Redirect::to(&format!("/manage_custom_field_edit_page/{}?msg=Linked", id)).into_response()
}

pub async fn manage_custom_field_proj_remove(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((id, project_id)): Path<(i64, i64)>,
) -> Response {
    if ctx.user.access_level < 90 {
        return forbidden(&ctx, "Administrator access required.");
    }
    let _ = db.unlink_custom_field_from_project(id, project_id);
    Redirect::to(&format!(
        "/manage_custom_field_edit_page/{}?msg=Unlinked",
        id
    ))
    .into_response()
}
