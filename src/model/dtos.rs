use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- Request DTOs (Para recibir datos del cliente) ---

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEventDto {
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub age_policy: String,
    pub status: String,
    pub terms: Option<String>,
    pub artist: Option<String>, // Campo para el artista
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEventPricingTierDto {
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEventStatusDto {
    pub status: String,
}

// --- Response DTOs (Para enviar datos al cliente) ---

#[derive(Debug, Clone, Serialize)]
pub struct EventResponseDto {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub age_policy: String,
    pub status: String,
    pub artist: Option<String>,
    pub schedules: Vec<EventScheduleDto>,
    pub pricing_tiers: Vec<EventPricingTierDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventScheduleDto {
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPricingTierDto {
    pub id: Uuid,
    pub name: String,
    pub price: Decimal,
    pub currency: String,
}

