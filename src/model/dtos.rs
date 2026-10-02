use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// --- Query DTOs ---

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct EventPaginationQueryDto {
    /// Page number, starting at 1. Defaults to 1; a value of 0 is treated as 1.
    pub page: Option<u64>,
    /// Events per page. Defaults to 10; values are clamped to the range 1 to 100.
    pub per_page: Option<u64>,
    /// Filter by exact event status, such as Scheduled, Draft, On Sale, Sold Out, Completed, or Cancelled.
    pub status: Option<String>,
    /// Return only events hosted at this venue UUID.
    pub venue_id: Option<Uuid>,
    /// Return only events owned by this organizer UUID.
    pub organizer_id: Option<Uuid>,
    /// Return only the event with this UUID, still inside a paginated response.
    pub id: Option<Uuid>,
}

// --- Request DTOs (Para recibir datos del cliente) ---

/// Request payload para la creación y registro de eventos
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventRequestDto {
    /// Required event name. Must contain at least one non-whitespace character.
    pub name: String,
    /// Required performing artist or group. Must contain at least one non-whitespace character.
    pub artist: String,
    /// Required event start date and time. Use RFC 3339 with a timezone, such as 2099-07-15T20:00:00Z. Must be in the future when submitted.
    pub date: DateTime<Utc>,
    /// Required UUID of the hosting venue. The JSON key venueId is also accepted. The default mock venue service accepts any nonzero UUID; the real venue service must confirm the venue exists.
    #[serde(alias = "venueId")]
    pub venue_id: Uuid,
    /// Optional event description. Omit or send null when not needed.
    pub description: Option<String>,
    /// Optional admission age policy, such as All ages or 18+. Defaults to All ages when omitted or null.
    pub age_policy: Option<String>,
    /// Optional event category, such as Concert or Festival. Defaults to Concert when omitted or null.
    pub event_type: Option<String>,
    /// Optional attendance terms and conditions. Omit or send null when not needed.
    pub terms: Option<String>,
}

/// Confirmación retornada tras la creación exitosa de un evento (HTTP 201)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventConfirmationDto {
    /// UUID assigned to the new event. Use it to fetch the event detail.
    pub id: Uuid,
    /// Confirmation message describing the result of the request.
    pub message: String,
    /// Display name of the event.
    pub name: String,
    /// Performing artist or group supplied in the creation request.
    pub artist: String,
    /// Start time of the initial event schedule, normalized to UTC.
    pub date: DateTime<Utc>,
    /// UUID of the venue hosting the event.
    pub venue_id: Uuid,
    /// Initial status assigned by the server: Scheduled.
    pub status: String,
    /// UTC timestamp when the record was created, in RFC 3339 format.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventDto {
    /// UUID of the organizer who owns the event. Event creation derives this value from the authenticated partner.
    pub organizer_id: Uuid,
    /// UUID of the venue hosting the event.
    pub venue_id: Uuid,
    /// Display name of the event.
    pub name: String,
    /// Optional description. Null when no description is recorded.
    pub description: Option<String>,
    /// Event category, such as Concert.
    pub event_type: String,
    /// Admission age policy, such as All ages or 18+.
    pub age_policy: String,
    /// Current event status. New events start as Scheduled.
    pub status: String,
    /// Optional terms and conditions for attending the event. Null when none are recorded.
    pub terms: Option<String>,
    /// Name of the performing artist or group. Null when no artist is recorded.
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEventPricingTierDto {
    /// Pricing category name, such as General, VIP, or Balcony.
    pub name: String,
    /// Optional description. Null when no description is recorded.
    pub description: Option<String>,
    /// Ticket price for this tier as a decimal amount in the specified currency.
    pub price: Decimal,
    /// Currency code for the price, such as USD or MXN.
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateEventStatusDto {
    /// Current event status. New events start as Scheduled.
    pub status: String,
}

// --- Response DTOs para Relaciones Hijas ---

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventScheduleDto {
    /// UUID identifying this performance schedule.
    pub id: Uuid,
    /// When the event performance starts, in RFC 3339 format.
    pub starts_at: DateTime<Utc>,
    /// Optional end date and time in RFC 3339 format. Null when no end time is recorded.
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventPricingTierDto {
    /// UUID identifying this ticket pricing tier.
    pub id: Uuid,
    /// Pricing category name, such as General, VIP, or Balcony.
    pub name: String,
    /// Optional description of this ticket category. Null when none is recorded.
    pub description: Option<String>,
    /// Ticket price for this tier as a decimal amount in the specified currency.
    pub price: Decimal,
    /// Currency code for the price, such as USD or MXN.
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventSaleDto {
    /// UUID identifying this ticket sale window.
    pub id: Uuid,
    /// When ticket sales open, in RFC 3339 format.
    pub starts_at: DateTime<Utc>,
    /// Optional time when ticket sales close. Null when no closing time is recorded.
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventMediaDto {
    /// UUID identifying this media resource.
    pub id: Uuid,
    /// Type of media, such as poster.
    pub media_type: String,
    /// URL of the media resource, such as a poster image.
    pub url: String,
}

// --- Response DTOs Principales ---

/// Detalle completo de un evento con todas sus colecciones anidadas
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventResponseDto {
    /// Unique UUID identifying this record.
    pub id: Uuid,
    /// UUID of the organizer who owns the event. Event creation derives this value from the authenticated partner.
    pub organizer_id: Uuid,
    /// UUID of the venue hosting the event.
    pub venue_id: Uuid,
    /// Display name of the event.
    pub name: String,
    /// Optional description. Null when no description is recorded.
    pub description: Option<String>,
    /// Event category, such as Concert.
    pub event_type: String,
    /// Admission age policy, such as All ages or 18+.
    pub age_policy: String,
    /// Current event status. New events start as Scheduled.
    pub status: String,
    /// Optional terms and conditions for attending the event. Null when none are recorded.
    pub terms: Option<String>,
    /// Name of the performing artist or group. Null when no artist is recorded.
    pub artist: Option<String>,
    /// Event performance schedules. An empty array means no schedules are recorded.
    pub schedules: Vec<EventScheduleDto>,
    /// Ticket pricing categories for this event. An empty array means no pricing tiers are recorded.
    pub pricing_tiers: Vec<EventPricingTierDto>,
    /// Ticket sale windows, separate from performance schedules. An empty array means no sale windows are recorded.
    pub sales: Vec<EventSaleDto>,
    /// Media resources associated with this event. An empty array means no media are recorded.
    pub media: Vec<EventMediaDto>,
    /// UTC timestamp when the record was created, in RFC 3339 format.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when the record was last updated, in RFC 3339 format.
    pub updated_at: DateTime<Utc>,
}

/// Proyección ligera y optimizada para listados paginados
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventSummaryDto {
    /// Unique UUID identifying this record.
    pub id: Uuid,
    /// UUID of the organizer who owns the event. Event creation derives this value from the authenticated partner.
    pub organizer_id: Uuid,
    /// UUID of the venue hosting the event.
    pub venue_id: Uuid,
    /// Display name of the event.
    pub name: String,
    /// Event category, such as Concert.
    pub event_type: String,
    /// Current event status. New events start as Scheduled.
    pub status: String,
    /// Name of the performing artist or group. Null when no artist is recorded.
    pub artist: Option<String>,
    /// Event performance schedules. An empty array means no schedules are recorded.
    pub schedules: Vec<EventScheduleDto>,
    /// Lowest price among the event pricing tiers. Null when there are no pricing tiers; currency is available in the event detail.
    pub min_price: Option<Decimal>,
}

/// Contenedor genérico para respuestas paginadas
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[aliases(PaginatedEventSummaryResponse = PaginatedResponseDto<EventSummaryDto>)]
pub struct PaginatedResponseDto<T> {
    /// Events matching the filters on the requested page. Empty when no events match or the page is beyond the last page.
    pub items: Vec<T>,
    /// Effective page number, starting at 1.
    pub page: u64,
    /// Effective maximum number of events per page, between 1 and 100.
    pub per_page: u64,
    /// Total number of events matching the filters across all pages.
    pub total_items: u64,
    /// Number of pages for the selected page size. Zero when no events match.
    pub total_pages: u64,
}
