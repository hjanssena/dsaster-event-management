use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// --- Query DTOs ---

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct EventPaginationQueryDto {
    /// Número de página (comienza en 1, por defecto 1)
    pub page: Option<u64>,
    /// Cantidad de elementos por página (por defecto 10, máx 100)
    pub per_page: Option<u64>,
    /// Filtrar por estado del evento (Draft, Scheduled, On Sale, Sold Out, Completed, Cancelled)
    pub status: Option<String>,
    /// Filtrar por identificador de recinto
    pub venue_id: Option<Uuid>,
    /// Filtrar por identificador de organizador
    pub organizer_id: Option<Uuid>,
    /// Filtrar por un identificador de evento específico
    pub id: Option<Uuid>,
}

// --- Request DTOs (Para recibir datos del cliente) ---

/// Request payload para la creación y registro de eventos
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventRequestDto {
    /// Nombre del evento
    pub name: String,
    /// Artista
    pub artist: String,
    /// Fecha y hora del evento
    pub date: DateTime<Utc>,
    /// Identificador del recinto
    #[serde(alias = "venueId")]
    pub venue_id: Uuid,
    /// Descripción opcional del evento
    pub description: Option<String>,
    /// Política de edad (por defecto "All ages")
    pub age_policy: Option<String>,
    /// Tipo de evento (por defecto "Concert")
    pub event_type: Option<String>,
    /// Términos y condiciones opcionales
    pub terms: Option<String>,
    /// Identificador del organizador
    #[serde(alias = "organizerId")]
    pub organizer_id: Option<Uuid>,
}

/// Confirmación retornada tras la creación exitosa de un evento (HTTP 201)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventConfirmationDto {
    pub id: Uuid,
    pub message: String,
    pub name: String,
    pub artist: String,
    pub date: DateTime<Utc>,
    pub venue_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventDto {
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub age_policy: String,
    pub status: String,
    pub terms: Option<String>,
    pub artist: Option<String>,
}


#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventPricingTierDto {
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateEventStatusDto {
    pub status: String,
}

// --- Response DTOs para Relaciones Hijas ---

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventScheduleDto {
    pub id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventPricingTierDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventSaleDto {
    pub id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventMediaDto {
    pub id: Uuid,
    pub media_type: String,
    pub url: String,
}

// --- Response DTOs Principales ---

/// Detalle completo de un evento con todas sus colecciones anidadas
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventResponseDto {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub age_policy: String,
    pub status: String,
    pub terms: Option<String>,
    pub artist: Option<String>,
    pub schedules: Vec<EventScheduleDto>,
    pub pricing_tiers: Vec<EventPricingTierDto>,
    pub sales: Vec<EventSaleDto>,
    pub media: Vec<EventMediaDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Proyección ligera y optimizada para listados paginados
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventSummaryDto {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub event_type: String,
    pub status: String,
    pub artist: Option<String>,
    pub schedules: Vec<EventScheduleDto>,
    pub min_price: Option<Decimal>,
}

/// Contenedor genérico para respuestas paginadas
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[aliases(PaginatedEventSummaryResponse = PaginatedResponseDto<EventSummaryDto>)]
pub struct PaginatedResponseDto<T> {
    pub items: Vec<T>,
    pub page: u64,
    pub per_page: u64,
    pub total_items: u64,
    pub total_pages: u64,
}

