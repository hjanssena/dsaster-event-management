use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        // Aseguramos que se intente cargar el archivo .env, aunque si no existe, no falla.
        dotenvy::dotenv().ok();

        let database_url = env::var("DATABASE_URL").expect("Falta la variable DATABASE_URL en el .env");
        let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        Self {
            database_url,
            server_addr,
        }
    }
}

