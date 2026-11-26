use crate::auth::AuthContext;
use crate::controllers::{parse_bool, sanitize_error_message, url_encode};
use crate::db::Db;
use crate::models::{parse_query_pairs, ListBugsQuery, FILTER_MATCH_ALL, FILTER_MATCH_ANY};
use crate::views;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::HashMap;

fn forbidden(ctx: &AuthContext, msg: &str) -> Response {
    let body = format!(
        r#"<div class="flash-error">Access Denied: {}</div>"#,
        views::html_escape(msg)
    );
    (
        StatusCode::FORBIDDEN,
        views::layout::render_layout("Access Denied", ctx, None, &body),
    )
        .into_response()
}

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateFilterForm {
    pub name: String,
    pub project_id: Option<i64>,
    pub is_public: Option<String>,
    pub match_type: Option<i32>,
}

pub async fn filter_save(State(db): State<Db>, ctx: AuthContext, body: Bytes) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to save filters.");
    }

    let pairs = parse_query_pairs(&String::from_utf8_lossy(&body));
    let mut name = String::new();
    let mut is_public_raw: Option<String> = None;
    for (k, v) in &pairs {
        match k.as_str() {
            "name" => name = v.clone(),
            "is_public" => is_public_raw = Some(v.clone()),
            _ => {}
        }
    }

    let is_public = parse_bool(is_public_raw.as_deref());
    if is_public && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Manager access required to save public filters.");
    }

    let q = ListBugsQuery::from_urlencoded_pairs(&pairs);
    if let Some(pid) = q.project_id.filter(|&v| v > 0) {
        let eff = db.effective_access_level(&ctx.user, pid).unwrap_or(0);
        if eff < 10 {
            return forbidden(&ctx, "You do not have access to this project.");
        }
    }
    let save_project_id = q
        .project_id
        .filter(|&v| v > 0)
        .unwrap_or(ctx.active_project_id);
    let filter =
        crate::controllers::bugs::filter_from_query(&db, &ctx.user, save_project_id, &q, true);

    match db.save_filter(ctx.user.id, save_project_id, is_public, &name, &filter) {
        Ok(fid) => Redirect::to(&format!(
            "/view_all_bug_page?filter_id={}&msg=Filter+saved",
            fid
        ))
        .into_response(),
        Err(e) => Redirect::to(&format!(
            "/view_all_bug_page?err={}",
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response(),
    }
}

pub async fn manage_filter_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let filters = db
        .list_manageable_filters(&ctx.user, ctx.active_project_id)
        .unwrap_or_default();

    let users_map: HashMap<i64, String> = db
        .list_users(None, false)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();
    let projects_map: HashMap<i64, String> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();

    views::filters::render_manage_filter_page(
        &ctx,
        &filters,
        &users_map,
        &projects_map,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn manage_filter_edit_page(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    Query(q): Query<FlashQuery>,
) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let (sf, bf) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        _ => return views::render_not_found_page(&ctx, "Saved filter not found"),
    };

    if sf.user_id != ctx.user.id && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Cannot edit another user's filter.");
    }
    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }

    let projects: Vec<(i64, String)> = ctx
        .visible_projects
        .iter()
        .map(|(p, _)| (p.id, p.name.clone()))
        .collect();

    views::filters::render_manage_filter_edit_page(
        &ctx,
        &sf,
        &bf,
        &projects,
        q.msg.as_deref(),
        q.err.as_deref(),
    )
    .into_response()
}

pub async fn manage_filter_update(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<UpdateFilterForm>,
) -> Response {
    if ctx.user.access_level < 25 {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let (sf, mut bf) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        _ => return views::render_not_found_page(&ctx, "Saved filter not found"),
    };

    if sf.user_id != ctx.user.id && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Cannot edit another user's filter.");
    }
    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }

    let new_is_public = if ctx.user.access_level >= 70 {
        match form.is_public.as_deref() {
            Some(v) => parse_bool(Some(v)),
            None => sf.is_public != 0,
        }
    } else {
        if parse_bool(form.is_public.as_deref()) && sf.is_public == 0 {
            return forbidden(&ctx, "Manager access required to publish filters.");
        }
        sf.is_public != 0
    };

    let new_project_id = form.project_id.filter(|&p| p >= 0).unwrap_or(sf.project_id);
    if new_project_id > 0
        && db
            .effective_access_level(&ctx.user, new_project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to the selected project.");
    }

    bf.project_id = if new_project_id > 0 {
        Some(new_project_id)
    } else {
        None
    };
    if let Some(mt) = form.match_type {
        bf.match_type = if mt == FILTER_MATCH_ANY {
            FILTER_MATCH_ANY
        } else {
            FILTER_MATCH_ALL
        };
    }

    match db.update_filter(id, &form.name, new_is_public, new_project_id, Some(&bf)) {
        Ok(_) => Redirect::to("/manage_filter_page?msg=Filter+updated").into_response(),
        Err(e) => Redirect::to(&format!(
            "/manage_filter_edit_page/{}?err={}",
            id,
            url_encode(&sanitize_error_message(&e))
        ))
        .into_response(),
    }
}

pub async fn filter_delete(
    State(db): State<Db>,
    ctx: AuthContext,
    Path(id): Path<i64>,
    body: Bytes,
) -> Response {
    let pairs = parse_query_pairs(&String::from_utf8_lossy(&body));
    let return_to_manage = pairs
        .iter()
        .any(|(k, v)| k == "return_to" && v.trim() == "manage");

    let (sf, _) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        _ => {
            let target = if return_to_manage {
                "/manage_filter_page?err=Filter+not+found"
            } else {
                "/view_all_bug_page?err=Filter+not+found"
            };
            return Redirect::to(target).into_response();
        }
    };

    if sf.user_id != ctx.user.id && ctx.user.access_level < 70 {
        return forbidden(&ctx, "Cannot delete another user's filter.");
    }
    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }

    let _ = db.delete_filter(id);
    if return_to_manage {
        Redirect::to("/manage_filter_page?msg=Filter+deleted").into_response()
    } else {
        Redirect::to("/view_all_bug_page?msg=Filter+deleted").into_response()
    }
}

pub fn router() -> Router<Db> {
    Router::new()
        .route("/filter_save", post(filter_save))
        .route("/filter_delete/:id", post(filter_delete))
        .route("/manage_filter_page", get(manage_filter_page))
        .route("/manage_filter_edit_page/:id", get(manage_filter_edit_page))
        .route("/manage_filter_update/:id", post(manage_filter_update))
}

#[cfg(test)]
#[path = "filters_test.rs"]
mod tests;
