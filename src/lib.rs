use sea_orm::{Database, DatabaseConnection};

pub mod api;
pub mod config;
pub mod core;
pub mod model;
pub mod repository;
pub mod service;
pub mod openapi;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection, // obligatorio para la conexión principal a la base de datos
    pub ticket_db: Option<DatabaseConnection>, // Opcional por ahora
    pub search_db: Option<DatabaseConnection>, // Opcional por ahora
}