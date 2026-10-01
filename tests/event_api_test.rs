use std::sync::Arc;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use chrono::Utc;
use rust_decimal::Decimal;
use tower::ServiceExt;
use uuid::Uuid;

use eventManagement_api::{
    api,
    model::dtos::{
        EventMediaDto, EventPricingTierDto, EventResponseDto, EventSaleDto, EventScheduleDto,
        PaginatedEventSummaryResponse,
    },
    repository::MockEventRepository,
    service::EventService,
    AppState,
};

fn create_test_app(mock_repo: Arc<MockEventRepository>) -> Router {
    let service = Arc::new(EventService::new(mock_repo));
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

#[tokio::test]
async fn test_http_get_event_by_id_200_ok() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let event_id = Uuid::new_v4();
    mock_repo.insert(create_sample_event(event_id, "Festival Primavera")).await;

    let app = create_test_app(mock_repo);

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
    let app = create_test_app(mock_repo);
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
    let app = create_test_app(mock_repo);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events/not-a-valid-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Axum Path extractor retorna 400 Bad Request cuando el Path no coincide con el tipo Uuid
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_http_get_events_paginated_200_ok() {
    let mock_repo = Arc::new(MockEventRepository::new());
    for i in 1..=5 {
        mock_repo
            .insert(create_sample_event(Uuid::new_v4(), &format!("Show {}", i)))
            .await;
    }

    let app = create_test_app(mock_repo);

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
    mock_repo.insert(create_sample_event(Uuid::new_v4(), "Show")).await;

    let app = create_test_app(mock_repo);

    // Consulta al alias sin "/api/v1"
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
