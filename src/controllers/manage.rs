use crate::auth::AuthContext;
use crate::db::Db;
use crate::models::{env_flag_enabled, SiteOverviewStats, SiteRuntimeConfig};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

pub async fn manage_overview_page(State(db): State<Db>, ctx: AuthContext) -> Response {
    if ctx.user.access_level < 90 {
        return crate::views::render_access_denied_page(&ctx);
    }

    let (stats, stats_err) = match db.get_site_overview_stats() {
        Ok(s) => (s, None),
        Err(e) => (
            SiteOverviewStats::default(),
            Some(format!("Failed to load database overview counts: {}", e)),
        ),
    };

    let runtime = SiteRuntimeConfig {
        cookie_secure: env_flag_enabled("MANTIS_COOKIE_SECURE"),
        upload_fs: std::env::var("MANTIS_UPLOAD_DIR")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
        sendgrid_enabled: std::env::var("SENDGRID_API_KEY")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
        jira_enabled: db.get_jira_config(0).map(|c| c.enabled).unwrap_or(false),
        query_token_enabled: env_flag_enabled("MANTIS_ALLOW_QUERY_TOKEN"),
    };

    crate::views::manage::render_manage_overview_page(&ctx, &stats, &runtime, stats_err.as_deref())
        .into_response()
}

pub fn router() -> Router<Db> {
    Router::new().route("/manage_overview_page", get(manage_overview_page))
}

#[cfg(test)]
#[path = "manage_test.rs"]
mod tests;
