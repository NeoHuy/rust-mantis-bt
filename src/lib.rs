pub mod auth;
pub mod db;
pub mod controllers;
pub mod handlers;
pub mod models;
pub mod ui;
pub mod views;

pub fn build_router(db: db::Db) -> axum::Router {
    handlers::build_router(db)
}
