use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::core::error::AppError;

/// Trait para la comunicación con el microservicio VenueManagement
#[async_trait]
pub trait VenueClient: Send + Sync {
    /// Verifica si un recinto existe y está activo para hospedar eventos
    async fn verify_venue_exists(&self, venue_id: Uuid) -> Result<bool, AppError>;
}

/// Implementación simulada en memoria mientras el equipo de Venue construye su servicio
#[derive(Clone, Default)]
pub struct MockVenueClient {
    /// Conjunto de recintos conocidos (None significa modo permisivo para aceptar cualquier UUID)
    allowed_venues: Option<Arc<RwLock<HashSet<Uuid>>>>,
}

impl MockVenueClient {
    /// Crea un mock permisivo que valida cualquier UUID no nulo (ideal para desarrollo y tests generales)
    pub fn new_permissive() -> Self {
        Self {
            allowed_venues: None,
        }
    }

    /// Crea un mock estricto con un conjunto definido de recintos
    pub fn new_with_venues(venues: HashSet<Uuid>) -> Self {
        Self {
            allowed_venues: Some(Arc::new(RwLock::new(venues))),
        }
    }

    /// Agrega un recinto a la lista de recintos permitidos
    pub async fn add_venue(&self, venue_id: Uuid) {
        if let Some(ref lock) = self.allowed_venues {
            let mut set = lock.write().await;
            set.insert(venue_id);
        }
    }
}

#[async_trait]
impl VenueClient for MockVenueClient {
    async fn verify_venue_exists(&self, venue_id: Uuid) -> Result<bool, AppError> {
        info!("(MOCK) Verificando existencia de recinto: {}", venue_id);

        if venue_id.is_nil() {
            return Ok(false);
        }

        if let Some(ref lock) = self.allowed_venues {
            let set = lock.read().await;
            Ok(set.contains(&venue_id))
        } else {
            // Modo permisivo: cualquier UUID válido se considera existente
            Ok(true)
        }
    }
}

/// Comunicacion por HTTP con la VenueAPI
pub struct HttpVenueClient {
    base_url: String,
    client: reqwest::Client,
}

impl HttpVenueClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        Self { base_url, client }
    }
}

#[async_trait]
impl VenueClient for HttpVenueClient {
    async fn verify_venue_exists(&self, venue_id: Uuid) -> Result<bool, AppError> {
        let url = format!(
            "{}/api/v1/venues/{}",
            self.base_url.trim_end_matches('/'),
            venue_id
        );
        info!(
            "Consultando existencia de recinto en Venue Service: {}",
            url
        );

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(true)
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    info!("Recinto {} no encontrado en Venue Service (404)", venue_id);
                    Ok(false)
                } else {
                    warn!("Venue Service respondió con status: {}", response.status());
                    Ok(false)
                }
            }
            Err(err) => {
                error!("Error de conexión al consultar Venue Service: {:?}", err);
                Err(AppError::InternalServerError(format!(
                    "Failed to communicate with Venue Service: {}",
                    err
                )))
            }
        }
    }
}
