pub mod auth;
pub mod controllers;
pub mod db;
pub mod jira;
pub mod models;
pub mod views;

pub fn build_router(db: db::Db) -> axum::Router {
    controllers::build_router(db)
}
