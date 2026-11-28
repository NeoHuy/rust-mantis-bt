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
use std::collections::{HashMap, HashSet};

fn filter_manager_access(db: &Db, user: &crate::models::User, project_id: i64) -> i32 {
    if project_id > 0 {
        db.effective_access_level(user, project_id).unwrap_or(0)
    } else {
        user.access_level
    }
}

fn has_filter_reporter_access(db: &Db, ctx: &AuthContext, project_id: i64) -> bool {
    if project_id > 0 {
        db.effective_access_level(&ctx.user, project_id)
            .unwrap_or(0)
            >= 25
    } else if ctx.user.access_level >= 25 {
        true
    } else {
        ctx.visible_projects
            .iter()
            .filter(|(p, _)| p.enabled != 0)
            .any(|(p, _)| db.effective_access_level(&ctx.user, p.id).unwrap_or(0) >= 25)
    }
}

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
    if !has_filter_reporter_access(&db, &ctx, save_project_id) {
        return forbidden(&ctx, "Reporter access required to save filters.");
    }
    if is_public && filter_manager_access(&db, &ctx.user, save_project_id) < 70 {
        return forbidden(&ctx, "Manager access required to save public filters.");
    }
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
    if !has_filter_reporter_access(&db, &ctx, 0) {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let filters = db
        .list_manageable_filters(&ctx.user, ctx.active_project_id)
        .unwrap_or_default();

    let mut proj_eff_cache: HashMap<i64, i32> = HashMap::new();
    let mut manageable_ids = HashSet::new();
    for sf in &filters {
        let can_manage = if sf.user_id == ctx.user.id {
            true
        } else if sf.is_public != 0 {
            let eff = *proj_eff_cache
                .entry(sf.project_id)
                .or_insert_with(|| filter_manager_access(&db, &ctx.user, sf.project_id));
            eff >= 70
        } else {
            false
        };
        if can_manage {
            manageable_ids.insert(sf.id);
        }
    }

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
        &manageable_ids,
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
    let (sf, mut bf) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        Ok(None) => return views::render_not_found_page(&ctx, "Saved filter not found"),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Database Error",
                    Some("Failed to load saved filter."),
                    "",
                ),
            )
                .into_response()
        }
    };

    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }
    if !has_filter_reporter_access(&db, &ctx, sf.project_id) {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let proj_mgr = filter_manager_access(&db, &ctx.user, sf.project_id) >= 70;
    if sf.is_public == 0 {
        if sf.user_id != ctx.user.id {
            return forbidden(
                &ctx,
                "Private saved filters can only be viewed or edited by their owner.",
            );
        }
    } else if sf.user_id != ctx.user.id && !proj_mgr {
        return forbidden(&ctx, "Cannot edit another user's public filter.");
    }

    let allowed_cf_ids: HashSet<i64> = db
        .list_accessible_custom_fields_for_user(&ctx.user, sf.project_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|cf| cf.filter_by != 0)
        .map(|cf| cf.id)
        .collect();
    bf.retain_custom_fields(&allowed_cf_ids);

    let can_toggle_public = sf.user_id == ctx.user.id && proj_mgr;

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
        can_toggle_public,
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
    let (sf, mut bf) = match db.get_filter(id) {
        Ok(Some(v)) => v,
        Ok(None) => return views::render_not_found_page(&ctx, "Saved filter not found"),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Database Error",
                    Some("Failed to load saved filter."),
                    "",
                ),
            )
                .into_response()
        }
    };

    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }
    if !has_filter_reporter_access(&db, &ctx, sf.project_id) {
        return forbidden(&ctx, "Reporter access required to manage saved filters.");
    }

    let cur_proj_mgr = filter_manager_access(&db, &ctx.user, sf.project_id) >= 70;
    if sf.is_public == 0 {
        if sf.user_id != ctx.user.id {
            return forbidden(
                &ctx,
                "Private saved filters can only be edited by their owner.",
            );
        }
    } else if sf.user_id != ctx.user.id && !cur_proj_mgr {
        return forbidden(&ctx, "Cannot edit another user's public filter.");
    }

    let new_project_id = form.project_id.filter(|&p| p >= 0).unwrap_or(sf.project_id);
    if new_project_id > 0
        && db
            .effective_access_level(&ctx.user, new_project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to the selected project.");
    }
    if !has_filter_reporter_access(&db, &ctx, new_project_id) {
        return forbidden(&ctx, "Reporter access required on the destination project.");
    }

    let can_toggle_public =
        sf.user_id == ctx.user.id && filter_manager_access(&db, &ctx.user, new_project_id) >= 70;
    let new_is_public = if can_toggle_public {
        match form.is_public.as_deref() {
            Some(v) => parse_bool(Some(v)),
            None => sf.is_public != 0,
        }
    } else {
        if let Some(raw_pub) = form.is_public.as_deref() {
            let requested = parse_bool(Some(raw_pub));
            if requested != (sf.is_public != 0) {
                return forbidden(
                    &ctx,
                    "Only the filter owner with Manager privileges may change filter visibility.",
                );
            }
        }
        sf.is_public != 0
    };
    if new_is_public
        && new_project_id != sf.project_id
        && filter_manager_access(&db, &ctx.user, new_project_id) < 70
    {
        return forbidden(
            &ctx,
            "Manager access required on the destination project to keep a filter public.",
        );
    }

    bf.project_id = if new_project_id > 0 {
        Some(new_project_id)
    } else {
        None
    };
    if new_project_id != sf.project_id {
        let allowed_cf_ids: HashSet<i64> = db
            .list_accessible_custom_fields_for_user(&ctx.user, new_project_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|cf| cf.filter_by != 0)
            .map(|cf| cf.id)
            .collect();
        bf.retain_custom_fields(&allowed_cf_ids);
    }
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
        Ok(None) => {
            let target = if return_to_manage {
                "/manage_filter_page?err=Filter+not+found"
            } else {
                "/view_all_bug_page?err=Filter+not+found"
            };
            return Redirect::to(target).into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                views::render_error_page(
                    &ctx,
                    "Database Error",
                    Some("Failed to load saved filter."),
                    "",
                ),
            )
                .into_response()
        }
    };

    if sf.project_id > 0
        && db
            .effective_access_level(&ctx.user, sf.project_id)
            .unwrap_or(0)
            < 10
    {
        return forbidden(&ctx, "You do not have access to this filter's project.");
    }

    if sf.is_public == 0 {
        if sf.user_id != ctx.user.id && ctx.user.access_level < 90 {
            return forbidden(
                &ctx,
                "Private saved filters can only be deleted by their owner or an Administrator.",
            );
        }
    } else if sf.user_id != ctx.user.id && filter_manager_access(&db, &ctx.user, sf.project_id) < 70
    {
        return forbidden(&ctx, "Cannot delete another user's public filter.");
    }

    let base = if return_to_manage {
        "/manage_filter_page"
    } else {
        "/view_all_bug_page"
    };
    match db.delete_filter(id) {
        Ok(()) => Redirect::to(&format!("{}?msg=Filter+deleted", base)).into_response(),
        Err(e) => Redirect::to(&format!(
            "{}?err={}",
            base,
            url_encode(&sanitize_error_message(&e.to_string()))
        ))
        .into_response(),
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
