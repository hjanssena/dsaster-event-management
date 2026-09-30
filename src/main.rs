use std::sync::Arc;
use axum::{
    routing::get,
    Router,
};
use sea_orm::Database;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use eventManagement_api::{
    api,
    config::AppConfig,
    repository::SeaOrmEventRepository,
    service::{EventService, HttpVenueClient, MockVenueClient, VenueClient},
    AppState,
};

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

    // 3. Inicializar Repositorios y Servicios (Inyección de Dependencias)
    let repo = Arc::new(SeaOrmEventRepository::new(event_db.clone()));

    let venue_client: Arc<dyn VenueClient> = if config.mock_venue_service {
        info!("Iniciando VenueClient en modo MOCK (simulado en memoria)");
        Arc::new(MockVenueClient::new_permissive())
    } else {
        info!("Iniciando HttpVenueClient conectado a {}", config.venue_service_url);
        Arc::new(HttpVenueClient::new(config.venue_service_url.clone()))
    };

    let event_service = Arc::new(EventService::new(repo, venue_client));

    let state = AppState {
        db: event_db,
        event_service,
        ticket_db: None,
        search_db: None,
    };

    // 4. Configurar CORS
    let cors = CorsLayer::permissive();

    // 5. Configurar el Router de Axum
    let app = Router::new()
        .route("/health", get(api::health_api::health_check))
        .nest("/api/v1/events", api::event_api::routes())
        .nest("/events", api::event_api::routes()) // Alias compatible
        .layer(TraceLayer::new_for_http()) // Middleware para logs HTTP
        .layer(cors) // Middleware para CORS
        .with_state(state);

    info!("EventAPI escuchando peticiones en {}", config.server_addr);
    let listener = tokio::net::TcpListener::bind(&config.server_addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
