use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub venue_service_url: String,
    pub mock_venue_service: bool,
    pub mock_auth: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        // Aseguramos que se intente cargar el archivo .env, aunque si no existe, no falla.
        dotenvy::dotenv().ok();

        let database_url =
            env::var("DATABASE_URL").expect("Falta la variable DATABASE_URL en el .env");
        let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let venue_service_url =
            env::var("VENUE_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
        let mock_venue_service = env::var("MOCK_VENUE_SERVICE")
            .map(|val| val.to_lowercase() != "false" && val != "0")
            .unwrap_or(true); // Activo por defecto mientras el equipo de Venue construye su servicio
        let mock_auth = env::var("MOCK_AUTH")
            .map(|val| val.to_lowercase() != "false" && val != "0")
            .unwrap_or(true); // Activo por defecto mientras el servicio de autenticación define el JWT

        Self {
            database_url,
            server_addr,
            venue_service_url,
            mock_venue_service,
            mock_auth,
        }
    }
}
