use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::model::dtos::{
    CreateEventRequestDto, EventConfirmationDto, EventPaginationQueryDto, EventResponseDto,
    PaginatedEventSummaryResponse,
};
use crate::repository::EventRepository;
use crate::service::VenueClient;

/// Capa de lógica de negocio y casos de uso para eventos
#[derive(Clone)]
pub struct EventService {
    repo: Arc<dyn EventRepository>,
    venue_client: Arc<dyn VenueClient>,
}

impl EventService {
    pub fn new(repo: Arc<dyn EventRepository>, venue_client: Arc<dyn VenueClient>) -> Self {
        Self { repo, venue_client }
    }

    /// Obtiene los detalles completos de un evento específico por su ID.
    /// Si no existe, retorna AppError::NotFound.
    pub async fn get_event_by_id(&self, id: Uuid) -> Result<EventResponseDto, AppError> {
        match self.repo.find_by_id(id).await? {
            Some(event) => Ok(event),
            None => Err(AppError::NotFound(format!("Event with ID {} not found", id))),
        }
    }

    /// Obtiene eventos de forma paginada aplicando filtros opcionales (incluyendo id específico).
    /// Aplica validaciones defensivas: page >= 1 y per_page acotado entre 1 y 100.
    pub async fn get_events(
        &self,
        mut query: EventPaginationQueryDto,
    ) -> Result<PaginatedEventSummaryResponse, AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

        query.page = Some(page);
        query.per_page = Some(per_page);

        self.repo.find_paginated(&query).await
    }

    /// Registra y crea un nuevo evento en el sistema.
    /// Valida nombre, artista, fecha de calendario futura y existencia del recinto con Venue Service.
    pub async fn create_event(
        &self,
        dto: CreateEventRequestDto,
    ) -> Result<EventConfirmationDto, AppError> {
        // 1. Validar nombre no vacío
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest("Event name cannot be empty".to_string()));
        }

        // 2. Validar artista no vacío
        if dto.artist.trim().is_empty() {
            return Err(AppError::BadRequest("Artist name cannot be empty".to_string()));
        }

        // 3. Validar fecha válida en calendario (debe ser futura)
        let now = Utc::now();
        if dto.date <= now {
            return Err(AppError::BadRequest(
                "Event date must be a valid future calendar date".to_string(),
            ));
        }

        // 4. Validar existencia del recinto en Venue Service
        let venue_exists = self.venue_client.verify_venue_exists(dto.venue_id).await?;
        if !venue_exists {
            return Err(AppError::UnprocessableEntity(
                "Venue does not exist or is invalid".to_string(),
            ));
        }

        // 5. Asignar organizer_id por defecto si no viene provisto (backstage default)
        let organizer_id = dto.organizer_id.unwrap_or_else(|| Uuid::from_u128(1));
        let event_id = Uuid::new_v4();

        // 6. Persistir atómicamente en base de datos
        self.repo.create_event(&dto, event_id, organizer_id).await
    }
}
