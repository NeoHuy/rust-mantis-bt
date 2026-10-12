use crate::auth::AuthContext;
use crate::db::Db;
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

pub async fn manage_tags_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    let tags = db.list_tags().unwrap_or_default();
    let err_banner = q
        .err
        .as_deref()
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

    render_layout("Manage Tags", &ctx, q.msg.as_deref(), &content).into_response()
}

#[derive(Deserialize)]
pub struct TagForm {
    pub name: String,
    pub description: Option<String>,
}

pub async fn tag_create(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<TagForm>,
) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to create tags.");
    }
    match db.create_tag(
        ctx.user.id,
        &form.name,
        form.description.as_deref().unwrap_or(""),
    ) {
        Ok(_) => Redirect::to("/manage_tags_page?msg=Tag+created").into_response(),
        Err(_) => Redirect::to("/manage_tags_page?err=Failed+to+create+tag").into_response(),
    }
}

pub async fn tag_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<TagForm>,
) -> Response {
    if ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to update tags.");
    }
    match db.update_tag(id, &form.name, form.description.as_deref().unwrap_or("")) {
        Ok(()) => Redirect::to("/manage_tags_page?msg=Tag+updated").into_response(),
        Err(_) => Redirect::to("/manage_tags_page?err=Failed+to+update+tag").into_response(),
    }
}

pub async fn tag_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    if ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to delete tags.");
    }
    let _ = db.delete_tag(id);
    Redirect::to("/manage_tags_page?msg=Tag+deleted").into_response()
}

#[derive(Deserialize)]
pub struct BugTagAttachForm {
    pub tag_id: Option<i64>,
    pub tag_string: Option<String>,
}

pub async fn bug_tag_attach(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(bug_id): Path<i64>,
    axum::Form(form): axum::Form<BugTagAttachForm>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return Redirect::to("/view_all_bug_page?err=Issue+not+found").into_response(),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    if eff < 25 || (bug.view_state == 50 && eff < 55) {
        return forbidden(&ctx, "Reporter access required to attach tags.");
    }

    if let Some(tid) = form.tag_id {
        if tid > 0 {
            let _ = db.attach_tag_to_bug(bug_id, tid, ctx.user.id);
        }
    }

    if let Some(ref raw_tags) = form.tag_string {
        for part in raw_tags.split(',') {
            let tname = part.trim();
            if tname.is_empty() {
                continue;
            }
            let tag_id = match db.get_tag_by_name(tname) {
                Ok(Some(t)) => t.id,
                _ => match db.create_tag(ctx.user.id, tname, "") {
                    Ok(id) => id,
                    Err(_) => continue,
                },
            };
            let _ = db.attach_tag_to_bug(bug_id, tag_id, ctx.user.id);
        }
    }

    Redirect::to(&format!("/view_bug_page/{}?msg=Tag+attached", bug_id)).into_response()
}

pub async fn bug_tag_detach(
    State(db): State<Db>,
    ctx: AuthContext,
    Path((bug_id, tag_id)): Path<(i64, i64)>,
) -> Response {
    let bug = match db.get_bug(bug_id) {
        Ok(Some(b)) => b,
        _ => return Redirect::to("/view_all_bug_page?err=Issue+not+found").into_response(),
    };

    let eff = db
        .effective_access_level(&ctx.user, bug.project_id)
        .unwrap_or(0);
    let bug_tags = db.list_bug_tags(bug_id).unwrap_or_default();
    let attached_by_me = bug_tags
        .iter()
        .any(|bt| bt.tag_id == tag_id && bt.user_id == ctx.user.id);

    if eff < 55 && !(eff >= 25 && attached_by_me) {
        return forbidden(&ctx, "Developer access required to detach tags.");
    }

    let _ = db.detach_tag_from_bug(bug_id, tag_id, ctx.user.id);
    Redirect::to(&format!("/view_bug_page/{}?msg=Tag+detached", bug_id)).into_response()
}
