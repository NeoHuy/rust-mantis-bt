pub mod auth;
pub mod db;
pub mod handlers;
pub mod jira;
pub mod models;
pub mod ui;

pub fn build_router(db: db::Db) -> axum::Router {
    handlers::build_router(db)
}
