#![allow(non_snake_case)]

use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub mod api;
pub mod config;
pub mod core;
pub mod model;
pub mod openapi;
pub mod repository;
pub mod service;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub event_service: Arc<service::EventService>,
    pub auth_service: Arc<service::AuthService>,
    pub ticket_db: Option<DatabaseConnection>,
    pub search_db: Option<DatabaseConnection>,
}