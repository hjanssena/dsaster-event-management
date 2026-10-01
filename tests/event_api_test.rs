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
    repository::{MockEventRepository, MockPartnerRepository, DEV_ORGANIZER_ID, DEV_VENUE_OWNER_ID},
    service::{AuthService, EventService, MockTokenVerifier, MockVenueClient, VenueClient},
    AppState,
};

fn create_test_app(
    mock_repo: Arc<MockEventRepository>,
    venue_client: Arc<dyn VenueClient>,
) -> Router {
    let service = Arc::new(EventService::new(mock_repo, venue_client));
    let auth_service = Arc::new(AuthService::new(
        Arc::new(MockTokenVerifier::new()),
        Arc::new(MockPartnerRepository::with_dev_partners()),
    ));
    let state = AppState {
        db: sea_orm::DatabaseConnection::Disconnected,
        event_service: service,
        auth_service,
        ticket_db: None,
        search_db: None,
    };

    Router::new()
        .route("/health", get(api::health_api::health_check))
        .nest("/api/v1/events", api::event_api::routes())
        .nest("/events", api::event_api::routes())
        .with_state(state)
}

fn bearer(partner_id: Uuid) -> String {
    format!("Bearer {}", MockTokenVerifier::token_for(partner_id))
}

fn post_event_request(payload: &serde_json::Value, authorization: Option<String>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/events")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder
        .body(Body::from(serde_json::to_vec(payload).unwrap()))
        .unwrap()
}

fn valid_event_payload() -> serde_json::Value {
    serde_json::json!({
        "name": "Evento Autenticado",
        "artist": "Artista",
        "date": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "venueId": Uuid::new_v4().to_string()
    })
}

async fn error_message(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["error"].as_str().unwrap().to_string()
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
                .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID))
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
                .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID))
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
                .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID))
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
                .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID))
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

// ==================== Tests de Autenticación y Rol (POST /events) ====================

#[tokio::test]
async fn test_http_post_event_without_token_401() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(post_event_request(&valid_event_payload(), None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(error_message(response).await.contains("Missing bearer token"));
}

#[tokio::test]
async fn test_http_post_event_non_bearer_scheme_401() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(post_event_request(
            &valid_event_payload(),
            Some("Basic dXN1YXJpbzpwYXNzd29yZA==".to_string()),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(error_message(response).await.contains("Missing bearer token"));
}

#[tokio::test]
async fn test_http_post_event_invalid_token_401() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(post_event_request(
            &valid_event_payload(),
            Some("Bearer no-es-un-token-valido".to_string()),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(error_message(response).await.contains("Invalid token"));
}

#[tokio::test]
async fn test_http_post_event_unknown_partner_401() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(post_event_request(&valid_event_payload(), Some(bearer(Uuid::new_v4()))))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(error_message(response).await.contains("Partner not found"));
}

#[tokio::test]
async fn test_http_post_event_venue_owner_403() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(post_event_request(&valid_event_payload(), Some(bearer(DEV_VENUE_OWNER_ID))))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(error_message(response).await.contains("Only organizers"));
}

#[tokio::test]
async fn test_http_post_event_ignores_body_organizer_id() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo.clone(), Arc::new(MockVenueClient::new_permissive()));

    // Un organizer_id enviado en el body nunca se usa para atribuir el evento (VE-06)
    let mut payload = valid_event_payload();
    payload["organizer_id"] = serde_json::json!(Uuid::new_v4().to_string());
    payload["organizerId"] = serde_json::json!(Uuid::new_v4().to_string());

    let response = app
        .oneshot(post_event_request(&payload, Some(bearer(DEV_ORGANIZER_ID))))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let confirmation: EventConfirmationDto = serde_json::from_slice(&body).unwrap();

    use eventManagement_api::repository::EventRepository;
    let stored = mock_repo.find_by_id(confirmation.id).await.unwrap().unwrap();
    assert_eq!(stored.organizer_id, DEV_ORGANIZER_ID);
}
