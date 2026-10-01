use std::collections::HashSet;
use std::sync::Arc;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    routing::get,
    Router,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use tower::ServiceExt;
use uuid::Uuid;

use eventManagement_api::{
    api,
    model::dtos::{
        EventConfirmationDto, EventMediaDto, EventPricingTierDto, EventResponseDto, EventSaleDto,
        EventScheduleDto, PaginatedEventSummaryResponse,
    },
    repository::MockEventRepository,
    service::{EventService, MockVenueClient, VenueClient},
    AppState,
};

fn create_test_app(
    mock_repo: Arc<MockEventRepository>,
    venue_client: Arc<dyn VenueClient>,
) -> Router {
    let service = Arc::new(EventService::new(mock_repo, venue_client));
    let state = AppState {
        db: sea_orm::DatabaseConnection::Disconnected,
        event_service: service,
        ticket_db: None,
        search_db: None,
    };

    Router::new()
        .route("/health", get(api::health_api::health_check))
        .nest("/api/v1/events", api::event_api::routes())
        .nest("/events", api::event_api::routes())
        .with_state(state)
}

fn create_sample_event(id: Uuid, name: &str) -> EventResponseDto {
    EventResponseDto {
        id,
        organizer_id: Uuid::new_v4(),
        venue_id: Uuid::new_v4(),
        name: name.to_string(),
        description: Some("Descripción de prueba".to_string()),
        event_type: "Concert".to_string(),
        age_policy: "All ages".to_string(),
        status: "Scheduled".to_string(),
        terms: None,
        artist: Some("Artista Test".to_string()),
        schedules: vec![EventScheduleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now(),
            ends_at: None,
        }],
        pricing_tiers: vec![EventPricingTierDto {
            id: Uuid::new_v4(),
            name: "General".to_string(),
            description: None,
            price: Decimal::new(2500, 2),
            currency: "USD".to_string(),
        }],
        sales: vec![EventSaleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now(),
            ends_at: None,
        }],
        media: vec![EventMediaDto {
            id: Uuid::new_v4(),
            media_type: "poster".to_string(),
            url: "https://test.com/poster.jpg".to_string(),
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// ==================== Tests de Endpoints GET ====================

#[tokio::test]
async fn test_http_get_event_by_id_200_ok() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let event_id = Uuid::new_v4();
    mock_repo.insert(create_sample_event(event_id, "Festival Primavera")).await;

    let app = create_test_app(mock_repo, venue_client);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/events/{}", event_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let event: EventResponseDto = serde_json::from_slice(&body).unwrap();
    assert_eq!(event.id, event_id);
    assert_eq!(event.name, "Festival Primavera");
}

#[tokio::test]
async fn test_http_get_event_by_id_404_not_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let app = create_test_app(mock_repo, venue_client);
    let non_existent = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/events/{}", non_existent))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains(&non_existent.to_string()));
}

#[tokio::test]
async fn test_http_get_event_by_id_400_bad_uuid() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let app = create_test_app(mock_repo, venue_client);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events/not-a-valid-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_http_get_events_paginated_200_ok() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    for i in 1..=5 {
        mock_repo
            .insert(create_sample_event(Uuid::new_v4(), &format!("Show {}", i)))
            .await;
    }

    let app = create_test_app(mock_repo, venue_client);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events?page=1&per_page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let paginated: PaginatedEventSummaryResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(paginated.page, 1);
    assert_eq!(paginated.per_page, 2);
    assert_eq!(paginated.total_items, 5);
    assert_eq!(paginated.total_pages, 3);
    assert_eq!(paginated.items.len(), 2);
}

#[tokio::test]
async fn test_http_get_events_alias_route() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    mock_repo.insert(create_sample_event(Uuid::new_v4(), "Show")).await;

    let app = create_test_app(mock_repo, venue_client);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/events?page=1&per_page=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ==================== Tests de Endpoints POST ====================

#[tokio::test]
async fn test_http_post_event_201_created() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let app = create_test_app(mock_repo, venue_client);

    let venue_id = Uuid::new_v4();
    let future_date = (Utc::now() + Duration::days(45)).to_rfc3339();

    // Probamos usando "venueId" en camelCase
    let payload = serde_json::json!({
        "name": "Muse World Tour 2026",
        "artist": "Muse",
        "date": future_date,
        "venueId": venue_id.to_string(),
        "description": "Gran concierto en estadio"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let confirmation: EventConfirmationDto = serde_json::from_slice(&body).unwrap();

    assert_eq!(confirmation.name, "Muse World Tour 2026");
    assert_eq!(confirmation.artist, "Muse");
    assert_eq!(confirmation.venue_id, venue_id);
    assert_eq!(confirmation.status, "Scheduled");
    assert_eq!(confirmation.message, "Event created successfully");
}

#[tokio::test]
async fn test_http_post_event_alias_route() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let app = create_test_app(mock_repo, venue_client);

    let venue_id = Uuid::new_v4();
    let future_date = (Utc::now() + Duration::days(20)).to_rfc3339();

    let payload = serde_json::json!({
        "name": "Indie Festival",
        "artist": "Indie Band",
        "date": future_date,
        "venue_id": venue_id.to_string()
    });

    // Petición a la ruta alias /events
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_http_post_event_past_date_400() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let app = create_test_app(mock_repo, venue_client);

    let past_date = (Utc::now() - Duration::days(5)).to_rfc3339();

    let payload = serde_json::json!({
        "name": "Evento con fecha pasada",
        "artist": "Artista",
        "date": past_date,
        "venueId": Uuid::new_v4().to_string()
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("valid future calendar date"));
}

#[tokio::test]
async fn test_http_post_event_venue_not_found_422() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let mut allowed_venues = HashSet::new();
    let registered_venue = Uuid::new_v4();
    allowed_venues.insert(registered_venue);

    let venue_client = Arc::new(MockVenueClient::new_with_venues(allowed_venues));
    let app = create_test_app(mock_repo, venue_client);

    let unknown_venue = Uuid::new_v4();
    let future_date = (Utc::now() + Duration::days(10)).to_rfc3339();

    let payload = serde_json::json!({
        "name": "Concierto en Recinto Desconocido",
        "artist": "Artista",
        "date": future_date,
        "venueId": unknown_venue.to_string()
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("Venue does not exist or is invalid"));
}
