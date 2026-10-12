pub mod api;
pub mod auth;
pub mod bugs;
pub mod custom_fields;
pub mod filters;
pub mod planning;
pub mod projects;
pub mod tags;
pub mod users;

use crate::auth::AuthContext;
use crate::db::Db;
use axum::extract::DefaultBodyLimit;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;

pub fn parse_bool(opt: Option<&str>) -> bool {
    match opt {
        Some(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "on" || v == "yes"
        }
        None => false,
    }
}

async fn root_handler(ctx: AuthContext) -> impl IntoResponse {
    if ctx.user.access_level >= 70 {
        Redirect::to("/manage_proj_page")
    } else {
        Redirect::to("/account_page")
    }
}

pub fn build_router(db: Db) -> Router {
    Router::new()
        .route("/", get(root_handler))
        // Auth routes
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route("/logout", get(auth::logout).post(auth::logout))
        .route("/set_project", post(auth::set_project))
        .route("/account_page", get(auth::account_page))
        .route("/account_update", post(auth::account_update))
        // User management routes
        .route("/manage_user_page", get(users::manage_user_page))
        .route("/manage_user_create_page", get(users::manage_user_create_page))
        .route("/manage_user_create", post(users::manage_user_create))
        .route("/manage_user_edit_page/:id", get(users::manage_user_edit_page))
        .route("/manage_user_update/:id", post(users::manage_user_update))
        .route("/manage_user_delete/:id", post(users::manage_user_delete))
        // Project management routes
        .route("/manage_proj_page", get(projects::manage_proj_page))
        .route("/manage_proj_create_page", get(projects::manage_proj_create_page))
        .route("/manage_proj_create", post(projects::manage_proj_create))
        .route("/manage_proj_edit_page/:id", get(projects::manage_proj_edit_page))
        .route("/manage_proj_update/:id", post(projects::manage_proj_update))
        .route("/manage_proj_delete/:id", post(projects::manage_proj_delete))
        .route("/manage_proj_subproj_add/:id", post(projects::manage_proj_subproj_add))
        .route("/manage_proj_subproj_delete/:id/:child_id", post(projects::manage_proj_subproj_delete))
        .route("/manage_proj_cat_add", post(projects::manage_proj_cat_add))
        .route("/manage_proj_cat_update/:cat_id", post(projects::manage_proj_cat_update))
        .route("/manage_proj_cat_delete/:cat_id", post(projects::manage_proj_cat_delete))
        .route("/manage_proj_ver_add/:id", post(projects::manage_proj_ver_add))
        .route("/manage_proj_ver_update/:ver_id", post(projects::manage_proj_ver_update))
        .route("/manage_proj_ver_delete/:ver_id", post(projects::manage_proj_ver_delete))
        .route("/manage_proj_user_add/:id", post(projects::manage_proj_user_add))
        .route("/manage_proj_user_remove/:id/:user_id", post(projects::manage_proj_user_remove))
        // Sub-project 2: Issue tracking & workflow routes
        .route("/view_all_bug_page", get(bugs::view_all_bug_page))
        .route("/view_bug_page/:id", get(bugs::view_bug_page))
        .route("/bug_report_page", get(bugs::bug_report_page))
        .route("/bug_report", post(bugs::bug_report))
        .route("/update_bug_page/:id", get(bugs::update_bug_page))
        .route("/update_bug/:id", post(bugs::update_bug))
        .route("/bug_delete/:id", post(bugs::bug_delete))
        .route("/bug_change_status/:id", post(bugs::bug_change_status))
        .route("/bug_assign/:id", post(bugs::bug_assign))
        .route("/bugnote_add/:bug_id", post(bugs::bugnote_add))
        .route("/bugnote_delete/:id", post(bugs::bugnote_delete))
        .route("/bug_relationship_add/:bug_id", post(bugs::bug_relationship_add))
        .route("/bug_relationship_delete/:id", post(bugs::bug_relationship_delete))
        .route(
            "/bug_file_add/:bug_id",
            post(bugs::bug_file_add).layer(DefaultBodyLimit::disable()),
        )
        .route("/file/bug/:id/:filename", get(bugs::bug_file_download))
        .route("/bug_file_delete/:id", post(bugs::bug_file_delete))
        // Sub-project 3: My View, Search, Saved Filters, Custom Fields, Tags, Monitors & CSV Export
        .route("/my_view_page", get(bugs::my_view_page))
        .route("/csv_export", get(bugs::csv_export))
        .route("/bug_monitor_add/:bug_id", post(bugs::bug_monitor_add))
        .route("/bug_monitor_delete/:bug_id", post(bugs::bug_monitor_delete))
        .route("/manage_tags_page", get(tags::manage_tags_page))
        .route("/tag_create", post(tags::tag_create))
        .route("/tag_update/:id", post(tags::tag_update))
        .route("/tag_delete/:id", post(tags::tag_delete))
        .route("/bug_tag_attach/:bug_id", post(tags::bug_tag_attach))
        .route("/bug_tag_detach/:bug_id/:tag_id", post(tags::bug_tag_detach))
        .route(
            "/manage_custom_field_page",
            get(custom_fields::manage_custom_field_page),
        )
        .route(
            "/manage_custom_field_create",
            post(custom_fields::manage_custom_field_create),
        )
        .route(
            "/manage_custom_field_edit_page/:id",
            get(custom_fields::manage_custom_field_edit_page),
        )
        .route(
            "/manage_custom_field_update/:id",
            post(custom_fields::manage_custom_field_update),
        )
        .route(
            "/manage_custom_field_delete/:id",
            post(custom_fields::manage_custom_field_delete),
        )
        .route(
            "/manage_custom_field_proj_add/:id",
            post(custom_fields::manage_custom_field_proj_add),
        )
        .route(
            "/manage_custom_field_proj_remove/:id/:project_id",
            post(custom_fields::manage_custom_field_proj_remove),
        )
        .route("/filter_save", post(filters::filter_save))
        .route("/filter_delete/:id", post(filters::filter_delete))
        // Sub-project 4: Roadmap, Changelog, Summary, Time Tracking, News & Email Queue
        .route("/roadmap_page", get(planning::roadmap_page))
        .route("/changelog_page", get(planning::changelog_page))
        .route("/summary_page", get(planning::summary_page))
        .route("/billing_page", get(planning::billing_page))
        .route("/news_page", get(planning::news_page))
        .route("/news_add", post(planning::news_add))
        .route("/news_delete/:id", post(planning::news_delete))
        .route(
            "/manage_email_queue_page",
            get(planning::manage_email_queue_page),
        )
        .route(
            "/manage_email_queue_flush",
            post(planning::manage_email_queue_flush),
        )
        .route("/manage_email_delete/:id", post(planning::manage_email_delete))
        // Sub-project 5: API Tokens, Plugin Management & MantisBT REST API
        .route("/api_tokens_page", get(api::api_tokens_page))
        .route("/api_token_create", post(api::api_token_create))
        .route("/api_token_revoke/:id", post(api::api_token_revoke))
        .route("/manage_plugin_page", get(api::manage_plugin_page))
        .route(
            "/manage_plugin_toggle/:basename",
            post(api::manage_plugin_toggle),
        )
        .route("/api/rest/users/me", get(api::rest_users_me))
        .route("/api/rest/projects", get(api::rest_list_projects))
        .route("/api/rest/projects/:id", get(api::rest_get_project))
        .route(
            "/api/rest/issues",
            get(api::rest_list_issues).post(api::rest_create_issue),
        )
        .route(
            "/api/rest/issues/:id",
            get(api::rest_get_issue)
                .patch(api::rest_update_issue)
                .delete(api::rest_delete_issue),
        )
        .route("/api/rest/issues/:id/notes", post(api::rest_add_issue_note))
        .with_state(db)
}
