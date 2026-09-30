use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::core::error::AppError;
#[allow(unused_imports)]
use crate::core::error::ErrorResponseDto;
use crate::model::dtos::{
    CreateEventRequestDto, EventConfirmationDto, EventPaginationQueryDto, EventResponseDto,
    PaginatedEventSummaryResponse,
};
use crate::AppState;

/// Obtiene los detalles completos de un evento específico por su ID.
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

/// Obtiene una lista paginada de eventos con filtros opcionales (o un evento específico vía ?id=...).
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

/// Registra y crea un nuevo evento en el sistema.
#[utoipa::path(
    post,
    path = "/api/v1/events",
    request_body = CreateEventRequestDto,
    responses(
        (status = 201, description = "Evento creado exitosamente", body = EventConfirmationDto),
        (status = 400, description = "Datos de entrada inválidos o recinto inexistente", body = ErrorResponseDto),
        (status = 500, description = "Error interno del servidor", body = ErrorResponseDto)
    ),
    tag = "events"
)]
pub async fn create_event(
    State(state): State<AppState>,
    Json(payload): Json<CreateEventRequestDto>,
) -> Result<(StatusCode, Json<EventConfirmationDto>), AppError> {
    let confirmation = state.event_service.create_event(payload).await?;
    Ok((StatusCode::CREATED, Json(confirmation)))
}

/// Crea el sub-enrutador de Axum para las rutas de eventos
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_events).post(create_event))
        .route("/:id", get(get_event_by_id))
}
