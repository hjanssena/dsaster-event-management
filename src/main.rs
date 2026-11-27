use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use eventManagement_api::{
    AppState, api,
    config::AppConfig,
    db,
    migration::{Migrator, MigratorTrait},
    repository::{
        MockPartnerRepository, PartnerRepository, SeaOrmEventRepository, SeaOrmPartnerRepository,
    },
    service::{
        AuthService, EventService, HttpVenueClient, MockTokenVerifier, MockVenueClient, VenueClient,
    },
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
    let event_db = db::connect(&config.database_url, &config.db)
        .await
        .expect("Error al conectar con la base de datos principal");

    // 3. Ejecutar migraciones automáticas de SeaORM
    info!("Ejecutando migraciones automáticas de SeaORM en la base de datos...");
    Migrator::up(&event_db, None)
        .await
        .expect("Error al ejecutar las migraciones de SeaORM");
    info!("Migraciones de SeaORM completadas con éxito.");

    // 4. Inicializar Repositorios y Servicios (Inyección de Dependencias)
    let repo = Arc::new(SeaOrmEventRepository::new(event_db.clone()));

    let venue_client: Arc<dyn VenueClient> = if config.venue.use_mock {
        info!("Iniciando VenueClient en modo MOCK (simulado en memoria)");
        Arc::new(MockVenueClient::new_permissive())
    } else {
        info!(
            "Iniciando HttpVenueClient conectado a {}",
            config.venue.base_url
        );
        Arc::new(HttpVenueClient::with_token(
            config.venue.base_url.clone(),
            config.venue.token.clone(),
            config.venue.timeout,
        ))
    };

    let event_service = Arc::new(EventService::new(repo, venue_client, config.pagination));

    // Partners: en modo MOCK se precargan un organizador y un venue owner de desarrollo
    let partner_repo: Arc<dyn PartnerRepository> = if config.mock_auth {
        info!("Iniciando PartnerRepository en modo MOCK (partners de desarrollo en memoria)");
        Arc::new(MockPartnerRepository::with_dev_partners())
    } else {
        Arc::new(SeaOrmPartnerRepository::new(event_db.clone()))
    };

    // Único verificador disponible hasta que el servicio de autenticación defina el JWT
    info!("Iniciando TokenVerifier en modo MOCK (tokens `mock:<uuid>`)");
    let auth_service = Arc::new(AuthService::new(
        Arc::new(MockTokenVerifier::new()),
        partner_repo,
    ));

    let state = AppState {
        db: event_db,
        event_service,
        auth_service,
        ticket_db: None,
        search_db: None,
    };

    // 5. Router with the request guard, tracing and CORS (shared with the tests)
    let app = api::build_router(state, &config.http);

    info!("EventAPI escuchando peticiones en {}", config.server_addr);
    let listener = tokio::net::TcpListener::bind(&config.server_addr)
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
