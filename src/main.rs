use axum::{
    routing::{get, post, put},
    Router,
};
use sea_orm::{Database, DatabaseConnection};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Declaración de los módulos (las capas de tu arquitectura)
mod api;
mod config;
mod core;
mod model;
mod repository;
mod service;

use config::AppConfig;

// El estado que Axum compartirá con todos los controladores
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection, // obligatorio para la conexión principal a la base de datos
    pub ticket_db: Option<DatabaseConnection>, // Opcional por ahora
    pub search_db: Option<DatabaseConnection>, // Opcional por ahora
}

#[tokio::main]
async fn main() {
    // 1. Inicializar Logs (Tracing)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Falló al configurar tracing");

    // 2. Cargar Configuración (.env)
    let config = AppConfig::from_env();

    info!("Conectando a la base de datos principal...");
    let event_db = Database::connect(&config.database_url)
        .await
        .expect("Error al conectar con la base de datos principal");

    let state = AppState {
        db: event_db,
        ticket_db: None,
        search_db: None,
    };

    // 3. Configurar CORS
    // Nota: CorsLayer::permissive() permite llamadas desde cualquier origen (bueno para desarrollo local).
    // Para producción, deberías restringir los orígenes permitidos.
    let cors = CorsLayer::permissive();

    // 4. Configurar el Router de Axum
    let app = Router::new()
        .route("/health", get(api::health_api::health_check))
        .layer(TraceLayer::new_for_http()) // Middleware para logs HTTP
        .layer(cors) // Middleware para CORS
        .with_state(state);

    info!("EventAPI escuchando peticiones en {}", config.server_addr);
    let listener = tokio::net::TcpListener::bind(&config.server_addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
