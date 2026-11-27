use axum::{
    Json, Router,
    body::Bytes,
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
    PaginatedEventSummaryResponse, UpdateEventRequestDto,
};

const SERVER_OWNED_EVENT_FIELDS: &[&str] = &[
    "id",
    "organizer_id",
    "organizerId",
    "status",
    "created_at",
    "createdAt",
    "updated_at",
    "updatedAt",
    "version",
    "published_at",
    "publishedAt",
];

fn parse_update_event_payload(body: &[u8]) -> Result<UpdateEventRequestDto, AppError> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("Invalid JSON request body".to_string()))?;

    let object = value
        .as_object()
        .ok_or_else(|| AppError::BadRequest("Request body must be a JSON object".to_string()))?;

    for field in SERVER_OWNED_EVENT_FIELDS {
        if object.contains_key(*field) {
            return Err(AppError::BadRequest(format!(
                "Server-owned field '{}' cannot be modified",
                field
            )));
        }
    }

    serde_json::from_value::<UpdateEventRequestDto>(value)
        .map_err(|err| AppError::BadRequest(format!("Invalid update payload: {}", err)))
}

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

#[utoipa::path(
    patch,
    path = "/api/v1/events/{id}",
    params(
        ("id" = Uuid, Path, description = "Unique event UUID")
    ),
    request_body = UpdateEventRequestDto,
    responses(
        (status = 200, description = "Updated event detail", body = EventResponseDto),
        (status = 400, description = "Invalid payload, server-owned field, empty text, past date, or missing schedule_id", body = ErrorResponseDto),
        (status = 401, description = "Missing, invalid, or unknown partner bearer token", body = ErrorResponseDto),
        (status = 403, description = "Authenticated partner is not the owning organizer", body = ErrorResponseDto),
        (status = 404, description = "Event not found", body = ErrorResponseDto),
        (status = 422, description = "Target venue or schedule_id is invalid or does not exist", body = ErrorResponseDto),
        (status = 500, description = "Internal server or persistence error", body = ErrorResponseDto)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "events"
)]
pub async fn update_event(
    State(state): State<AppState>,
    AuthenticatedPartner(partner): AuthenticatedPartner,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Result<Json<EventResponseDto>, AppError> {
    let payload = parse_update_event_payload(&body)?;
    let updated = state
        .event_service
        .update_event(id, payload, &partner)
        .await?;
    Ok(Json(updated))
}

/// Crea el sub-enrutador de Axum para las rutas de eventos
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_events).post(create_event))
        .route("/:id", get(get_event_by_id).patch(update_event))
}
