use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::PaginationConfig;
use crate::core::error::AppError;
use crate::model::dtos::{
    CreateEventRequestDto, EventConfirmationDto, EventPaginationQueryDto, EventResponseDto,
    EventScheduleDto, EventUpdateChanges, PaginatedEventSummaryResponse, UpdateEventRequestDto,
};
use crate::model::partner::{self, PartnerRole};
use crate::repository::EventRepository;
use crate::service::VenueClient;
use crate::service::request_time::parse_request_time;

fn validate_non_empty_text(value: String, field_label: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!(
            "{} cannot be empty",
            field_label
        )));
    }
    if trimmed.chars().count() > 255 {
        return Err(AppError::BadRequest(format!(
            "{} cannot exceed 255 characters",
            field_label
        )));
    }
    Ok(trimmed.to_string())
}

fn resolve_target_schedule(
    schedules: &[EventScheduleDto],
    requested_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    match requested_id {
        Some(target_id) => {
            if schedules.iter().any(|schedule| schedule.id == target_id) {
                Ok(target_id)
            } else {
                Err(AppError::UnprocessableEntity(
                    "Target schedule does not belong to this event".to_string(),
                ))
            }
        }
        None => match schedules {
            [single] => Ok(single.id),
            [] => Err(AppError::UnprocessableEntity(
                "Event has no schedule to update".to_string(),
            )),
            _ => Err(AppError::BadRequest(
                "Multiple schedules exist; schedule_id is required to update date".to_string(),
            )),
        },
    }
}

/// Capa de lógica de negocio y casos de uso para eventos
#[derive(Clone)]
pub struct EventService {
    repo: Arc<dyn EventRepository>,
    venue_client: Arc<dyn VenueClient>,
    pagination: PaginationConfig,
}

impl EventService {
    pub fn new(
        repo: Arc<dyn EventRepository>,
        venue_client: Arc<dyn VenueClient>,
        pagination: PaginationConfig,
    ) -> Self {
        Self {
            repo,
            venue_client,
            pagination,
        }
    }

    /// Obtiene los detalles completos de un evento específico por su ID.
    /// Si no existe, retorna AppError::NotFound.
    pub async fn get_event_by_id(&self, id: Uuid) -> Result<EventResponseDto, AppError> {
        match self.repo.find_by_id(id).await? {
            Some(event) => Ok(event),
            None => Err(AppError::NotFound(format!(
                "Event with ID {} not found",
                id
            ))),
        }
    }

    /// Obtiene eventos de forma paginada aplicando filtros opcionales (incluyendo id específico).
    /// `page` defaults to 1 (0 counts as 1); `per_page` defaults to `EVENTS_PAGE_SIZE_DEFAULT`
    /// and is clamped to between 1 and `EVENTS_PAGE_SIZE_MAX` (ADR-006).
    pub async fn get_events(
        &self,
        mut query: EventPaginationQueryDto,
    ) -> Result<PaginatedEventSummaryResponse, AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query
            .per_page
            .unwrap_or(self.pagination.default_page_size)
            .clamp(1, self.pagination.max_page_size);

        query.page = Some(page);
        query.per_page = Some(per_page);

        self.repo.find_paginated(&query, page, per_page).await
    }

    /// Registra y crea un nuevo evento en el sistema a nombre del organizador autenticado.
    /// Valida rol, nombre, artista, fecha de calendario futura y existencia del recinto con Venue Service.
    pub async fn create_event(
        &self,
        dto: CreateEventRequestDto,
        organizer: &partner::Model,
    ) -> Result<EventConfirmationDto, AppError> {
        // 0. Solo los organizadores pueden registrar eventos (VE-07)
        if organizer.role != PartnerRole::Organizer {
            return Err(AppError::Forbidden(
                "Only organizers can register events".to_string(),
            ));
        }

        // Times are parsed before any other check: no offset is 422, not a time is 400 (ADR-010)
        let date = parse_request_time(&dto.date, "date")?;

        // 1. Validar nombre no vacío
        if dto.name.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Event name cannot be empty".to_string(),
            ));
        }

        // 2. Validar artista no vacío
        if dto.artist.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Artist name cannot be empty".to_string(),
            ));
        }

        // 3. Validar fecha válida en calendario (debe ser futura)
        let now = Utc::now();
        if date <= now {
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

        // 5. El organizador se obtiene de la sesión, nunca del formulario (VE-06)
        let organizer_id = organizer.id;
        let event_id = Uuid::new_v4();

        // 6. Persistir atómicamente en base de datos
        self.repo
            .create_event(&dto, date, event_id, organizer_id)
            .await
    }

    pub async fn update_event(
        &self,
        id: Uuid,
        dto: UpdateEventRequestDto,
        organizer: &partner::Model,
    ) -> Result<EventResponseDto, AppError> {
        if organizer.role != PartnerRole::Organizer {
            return Err(AppError::Forbidden(
                "Only organizers can update events".to_string(),
            ));
        }

        let existing = match self.repo.find_by_id(id).await? {
            Some(event) => event,
            None => {
                return Err(AppError::NotFound(format!(
                    "Event with ID {} not found",
                    id
                )));
            }
        };

        if existing.organizer_id != organizer.id {
            return Err(AppError::Forbidden(
                "Only the owning organizer can update this event".to_string(),
            ));
        }

        if dto.name.is_none()
            && dto.artist.is_none()
            && dto.date.is_none()
            && dto.schedule_id.is_none()
            && dto.venue_id.is_none()
        {
            return Err(AppError::BadRequest(
                "At least one updatable field must be provided".to_string(),
            ));
        }

        if dto.schedule_id.is_some() && dto.date.is_none() {
            return Err(AppError::BadRequest(
                "schedule_id requires date to be provided".to_string(),
            ));
        }

        let name = dto
            .name
            .map(|value| validate_non_empty_text(value, "Event name"))
            .transpose()?;

        let artist = dto
            .artist
            .map(|value| validate_non_empty_text(value, "Artist name"))
            .transpose()?;

        // No offset is 422, not a time is 400 (ADR-010)
        let new_date = dto
            .date
            .as_ref()
            .map(|date| parse_request_time(date, "date"))
            .transpose()?;

        let schedule_update = match new_date {
            Some(new_date) => {
                if new_date <= Utc::now() {
                    return Err(AppError::BadRequest(
                        "Event date must be a valid future calendar date".to_string(),
                    ));
                }
                let target_id = resolve_target_schedule(&existing.schedules, dto.schedule_id)?;
                Some((target_id, new_date))
            }
            None => None,
        };

        if let Some(new_venue_id) = dto
            .venue_id
            .filter(|candidate| *candidate != existing.venue_id)
        {
            let venue_exists = self.venue_client.verify_venue_exists(new_venue_id).await?;
            if !venue_exists {
                return Err(AppError::UnprocessableEntity(
                    "Venue does not exist or is invalid".to_string(),
                ));
            }
        }

        let changes = EventUpdateChanges {
            name,
            artist,
            venue_id: dto.venue_id,
            schedule_update,
        };

        self.repo
            .update_event(id, &changes)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Event with ID {} not found", id)))
    }
}
