use std::sync::Arc;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::model::dtos::{
    EventPaginationQueryDto, EventResponseDto, PaginatedEventSummaryResponse,
};
use crate::repository::EventRepository;

/// Capa de lógica de negocio y casos de uso para eventos
#[derive(Clone)]
pub struct EventService {
    repo: Arc<dyn EventRepository>,
}

impl EventService {
    pub fn new(repo: Arc<dyn EventRepository>) -> Self {
        Self { repo }
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
}
