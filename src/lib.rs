pub mod config;
pub mod routes;
pub mod handlers;
pub mod schemas;
pub mod utils;
pub mod middlewares;
pub mod models;

pub mod app;

pub use app::create_app;
