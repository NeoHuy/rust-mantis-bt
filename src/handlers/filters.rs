use crate::auth::AuthContext;
use crate::db::Db;
use crate::handlers::parse_bool;
use crate::models::BugFilter;
use crate::ui::{html_escape, render_layout};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

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

#[derive(Deserialize)]
pub struct SaveFilterForm {
    pub name: String,
    pub is_public: Option<String>,
    pub search: Option<String>,
    pub status: Option<i32>,
    pub hide_status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub category_id: Option<i64>,
    pub reporter_id: Option<i64>,
    pub handler_id: Option<i64>,
    pub tag_id: Option<i64>,
}

pub async fn filter_save(
    State(db): State<Db>,
    ctx: AuthContext,
    axum::Form(form): axum::Form<SaveFilterForm>,
) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to save filters.");
    }
    let is_public = parse_bool(form.is_public.as_deref());
    if is_public && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to save public filters.");
    }

    let filter = BugFilter {
        project_id: if ctx.active_project_id > 0 {
            Some(ctx.active_project_id)
        } else {
            None
        },
        search: form.search.filter(|s| !s.trim().is_empty()),
        status: form.status.filter(|&v| v > 0),
        hide_status: form.hide_status.filter(|&v| v > 0),
        priority: form.priority.filter(|&v| v > 0),
        severity: form.severity.filter(|&v| v > 0),
        category_id: form.category_id.filter(|&v| v > 0),
        reporter_id: form.reporter_id.filter(|&v| v > 0),
        handler_id: form.handler_id.filter(|&v| v != 0),
        tag_id: form.tag_id.filter(|&v| v > 0),
        monitored_by: None,
    };

    match db.save_filter(
        ctx.user.id,
        ctx.active_project_id,
        is_public,
        &form.name,
        &filter,
    ) {
        Ok(fid) => Redirect::to(&format!(
            "/view_all_bug_page?filter_id={}&msg=Filter+saved",
            fid
        ))
        .into_response(),
        Err(_) => Redirect::to("/view_all_bug_page?err=Failed+to+save+filter").into_response(),
    }
}

pub async fn filter_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
) -> Response {
    let (sf, _) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        _ => return Redirect::to("/view_all_bug_page?err=Filter+not+found").into_response(),
    };

    if sf.user_id != ctx.user.id && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Cannot delete another user's filter.");
    }

    let _ = db.delete_filter(id);
    Redirect::to("/view_all_bug_page?msg=Filter+deleted").into_response()
}
