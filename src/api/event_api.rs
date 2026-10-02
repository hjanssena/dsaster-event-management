use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use uuid::Uuid;

use crate::AppState;
use crate::api::auth::AuthenticatedPartner;
use crate::core::error::AppError;
#[allow(unused_imports)]
use crate::core::error::ErrorResponseDto;
use crate::model::dtos::{
    CreateEventRequestDto, EventConfirmationDto, EventPaginationQueryDto, EventResponseDto,
    PaginatedEventSummaryResponse,
};

/// Get the complete event detail.
///
/// No authentication is required. Copy an event ID from the list or creation response.
#[utoipa::path(
    get,
    path = "/api/v1/events/{id}",
    params(
        ("id" = Uuid, Path, description = "Identificador único del evento en formato UUID")
    ),
    responses(
        (status = 200, description = "Detalle completo del evento", body = EventResponseDto),
        (status = 404, description = "Evento no encontrado", body = ErrorResponseDto),
        (status = 400, description = "Identificador UUID inválido", body = ErrorResponseDto),
        (status = 500, description = "Error interno del servidor", body = ErrorResponseDto)
    ),
    tag = "events"
)]
pub async fn get_event_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EventResponseDto>, AppError> {
    let event = state.event_service.get_event_by_id(id).await?;
    Ok(Json(event))
}

/// List events with pagination and optional filters.
///
/// No authentication is required. Leave filters blank to list all events. Supplied filters are combined.
#[utoipa::path(
    get,
    path = "/api/v1/events",
    params(
        EventPaginationQueryDto
    ),
    responses(
        (status = 200, description = "Lista paginada de eventos", body = PaginatedEventSummaryResponse),
        (status = 500, description = "Error interno del servidor", body = ErrorResponseDto)
    ),
    tag = "events"
)]
pub async fn get_events(
    State(state): State<AppState>,
    Query(query): Query<EventPaginationQueryDto>,
) -> Result<Json<PaginatedEventSummaryResponse>, AppError> {
    let result = state.event_service.get_events(query).await?;
    Ok(Json(result))
}

/// Create an event for the authenticated organizer.
///
/// Use Authorize with an organizer token, then Try it out. The request requires name,
/// artist, a future date, and venue_id. The organizer and initial Scheduled status are set by the server.
#[utoipa::path(
    post,
    path = "/api/v1/events",
    request_body = CreateEventRequestDto,
    responses(
        (status = 201, description = "Evento creado exitosamente", body = EventConfirmationDto),
        (status = 400, description = "Nombre o artista vacío, o fecha que no está en el futuro", body = ErrorResponseDto),
        (status = 401, description = "Token ausente o inválido, o partner inexistente", body = ErrorResponseDto),
        (status = 403, description = "El partner autenticado no es organizador", body = ErrorResponseDto),
        (status = 422, description = "Recinto inexistente o inválido", body = ErrorResponseDto),
        (status = 500, description = "Error interno del servidor", body = ErrorResponseDto)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "events"
)]
pub async fn create_event(
    State(state): State<AppState>,
    AuthenticatedPartner(partner): AuthenticatedPartner,
    Json(payload): Json<CreateEventRequestDto>,
) -> Result<(StatusCode, Json<EventConfirmationDto>), AppError> {
    let confirmation = state.event_service.create_event(payload, &partner).await?;
    Ok((StatusCode::CREATED, Json(confirmation)))
}

/// Crea el sub-enrutador de Axum para las rutas de eventos
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_events).post(create_event))
        .route("/:id", get(get_event_by_id))
}
