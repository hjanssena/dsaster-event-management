use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use eventManagement_api::{
    AppState, api,
    config::{HttpConfig, PaginationConfig},
    model::dtos::{
        EventConfirmationDto, EventMediaDto, EventPricingTierDto, EventResponseDto, EventSaleDto,
        EventScheduleDto, PaginatedEventSummaryResponse,
    },
    repository::{
        DEV_ORGANIZER_ID, DEV_VENUE_OWNER_ID, MockEventRepository, MockPartnerRepository,
    },
    service::{AuthService, EventService, MockTokenVerifier, MockVenueClient, VenueClient},
};

fn create_test_app(
    mock_repo: Arc<MockEventRepository>,
    venue_client: Arc<dyn VenueClient>,
) -> Router {
    let service = Arc::new(EventService::new(
        mock_repo,
        venue_client,
        PaginationConfig {
            default_page_size: 10,
            max_page_size: 100,
        },
    ));
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

    api::build_router(
        state,
        &HttpConfig {
            request_timeout: std::time::Duration::from_secs(10),
            retry_after_seconds: 2,
        },
    )
}

fn bearer(partner_id: Uuid) -> String {
    format!("Bearer {}", MockTokenVerifier::token_for(partner_id))
}

#[tokio::test]
async fn test_http_openapi_spec_is_public() {
    let app = create_test_app(
        Arc::new(MockEventRepository::new()),
        Arc::new(MockVenueClient::new_permissive()),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let spec: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(spec["paths"]["/health"]["get"].is_object());
    assert!(spec["paths"]["/api/v1/events"]["get"].is_object());
    assert!(spec["paths"]["/api/v1/events/{id}"]["get"].is_object());
    assert_eq!(
        spec["paths"]["/api/v1/events"]["post"]["security"][0]["bearer_auth"],
        serde_json::json!([])
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["bearer_auth"]["scheme"],
        "bearer"
    );
}

#[tokio::test]
async fn test_http_swagger_ui_and_assets_are_public() {
    let app = create_test_app(
        Arc::new(MockEventRepository::new()),
        Arc::new(MockVenueClient::new_permissive()),
    );
    for (path, expected_content) in [
        ("/swagger-ui/", "Swagger UI"),
        (
            "/swagger-ui/swagger-initializer.js",
            "/api-docs/openapi.json",
        ),
        ("/swagger-ui/swagger-ui.css", ".swagger-ui"),
        ("/swagger-ui/swagger-ui-bundle.js", "SwaggerUIBundle"),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(
            String::from_utf8(body.to_vec())
                .unwrap()
                .contains(expected_content),
            "{path}"
        );
    }
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
    mock_repo
        .insert(create_sample_event(event_id, "Festival Primavera"))
        .await;

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
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains(&non_existent.to_string())
    );
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

    assert_json_400(response).await;
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
    mock_repo
        .insert(create_sample_event(Uuid::new_v4(), "Show"))
        .await;

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
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("valid future calendar date")
    );
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
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("Venue does not exist or is invalid")
    );
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
    assert!(
        error_message(response)
            .await
            .contains("Missing bearer token")
    );
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
    assert!(
        error_message(response)
            .await
            .contains("Missing bearer token")
    );
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
        .oneshot(post_event_request(
            &valid_event_payload(),
            Some(bearer(Uuid::new_v4())),
        ))
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
        .oneshot(post_event_request(
            &valid_event_payload(),
            Some(bearer(DEV_VENUE_OWNER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(error_message(response).await.contains("Only organizers"));
}

#[tokio::test]
async fn test_http_post_event_ignores_body_organizer_id() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(
        mock_repo.clone(),
        Arc::new(MockVenueClient::new_permissive()),
    );

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
    let stored = mock_repo
        .find_by_id(confirmation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.organizer_id, DEV_ORGANIZER_ID);
}

fn patch_event_request(uri: &str, body: Body, authorization: Option<String>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("PATCH")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder.body(body).unwrap()
}

fn create_owned_event(
    event_id: Uuid,
    organizer_id: Uuid,
    venue_id: Uuid,
    schedule_count: usize,
) -> EventResponseDto {
    let past_timestamp = Utc::now() - Duration::days(2);
    let schedules = (0..schedule_count)
        .map(|idx| EventScheduleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now() + Duration::days(15 + idx as i64),
            ends_at: None,
        })
        .collect();

    EventResponseDto {
        id: event_id,
        organizer_id,
        venue_id,
        name: "Original Concert".to_string(),
        description: Some("Original details".to_string()),
        event_type: "Concert".to_string(),
        age_policy: "All ages".to_string(),
        status: "Scheduled".to_string(),
        terms: None,
        artist: Some("Original Band".to_string()),
        schedules,
        pricing_tiers: vec![],
        sales: vec![],
        media: vec![],
        created_at: past_timestamp,
        updated_at: past_timestamp,
    }
}

#[tokio::test]
async fn test_http_patch_event_200_ok_and_persists_state() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let old_venue = Uuid::new_v4();
    let new_venue = Uuid::new_v4();
    let venue_client = Arc::new(MockVenueClient::new_with_venues(HashSet::from([new_venue])));

    let event_id = Uuid::new_v4();
    let original = create_owned_event(event_id, DEV_ORGANIZER_ID, old_venue, 1);
    mock_repo.insert(original.clone()).await;

    let app = create_test_app(mock_repo, venue_client);
    let next_date = Utc::now() + Duration::days(60);
    let payload = serde_json::json!({
        "name": "  Updated Concert  ",
        "artist": "Updated Band",
        "date": next_date.to_rfc3339(),
        "venueId": new_venue.to_string(),
    });

    let response = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(serde_json::to_vec(&payload).unwrap()),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated: EventResponseDto = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.organizer_id, DEV_ORGANIZER_ID);
    assert_eq!(updated.status, "Scheduled");
    assert_eq!(updated.created_at, original.created_at);
    assert!(updated.updated_at > original.updated_at);
    assert_eq!(updated.name, "Updated Concert");
    assert_eq!(updated.artist.as_deref(), Some("Updated Band"));
    assert_eq!(updated.venue_id, new_venue);
    assert_eq!(updated.schedules[0].starts_at, next_date);

    let detail_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/events/{}", event_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let persisted: EventResponseDto = serde_json::from_slice(&detail_body).unwrap();
    assert_eq!(persisted.name, "Updated Concert");
    assert_eq!(persisted.venue_id, new_venue);
}

#[tokio::test]
async fn test_http_patch_event_alias_route_200_ok() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let event_id = Uuid::new_v4();
    mock_repo
        .insert(create_owned_event(
            event_id,
            DEV_ORGANIZER_ID,
            Uuid::new_v4(),
            1,
        ))
        .await;

    let app = create_test_app(mock_repo, venue_client);
    let payload = serde_json::json!({ "name": "Alias Route Update" });

    let response = app
        .oneshot(patch_event_request(
            &format!("/events/{}", event_id),
            Body::from(serde_json::to_vec(&payload).unwrap()),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_patch_event_auth_401() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let event_id = Uuid::new_v4();
    mock_repo
        .insert(create_owned_event(
            event_id,
            DEV_ORGANIZER_ID,
            Uuid::new_v4(),
            1,
        ))
        .await;
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));
    let body = serde_json::to_vec(&serde_json::json!({ "name": "Changed" })).unwrap();

    for auth in [
        None,
        Some("Basic dXNlcjpwYXNz".to_string()),
        Some("Bearer invalid-token".to_string()),
        Some(bearer(Uuid::new_v4())),
    ] {
        let response = app
            .clone()
            .oneshot(patch_event_request(
                &format!("/api/v1/events/{}", event_id),
                Body::from(body.clone()),
                auth,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn test_http_patch_event_ownership_and_role_403() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let foreign_organizer_event_id = Uuid::new_v4();
    mock_repo
        .insert(create_owned_event(
            foreign_organizer_event_id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            1,
        ))
        .await;
    let owned_event_id = Uuid::new_v4();
    mock_repo
        .insert(create_owned_event(
            owned_event_id,
            DEV_ORGANIZER_ID,
            Uuid::new_v4(),
            1,
        ))
        .await;

    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));
    let payload = serde_json::to_vec(&serde_json::json!({ "name": "Forbidden Edit" })).unwrap();

    let wrong_role = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", owned_event_id),
            Body::from(payload.clone()),
            Some(bearer(DEV_VENUE_OWNER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_role.status(), StatusCode::FORBIDDEN);

    let wrong_owner = app
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", foreign_organizer_event_id),
            Body::from(payload),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_owner.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_http_patch_event_404_not_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));
    let payload = serde_json::to_vec(&serde_json::json!({ "name": "Ghost Event" })).unwrap();

    let response = app
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", Uuid::new_v4()),
            Body::from(payload),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_patch_event_rejects_server_owned_unknown_and_invalid_payloads_400() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let event_id = Uuid::new_v4();
    mock_repo
        .insert(create_owned_event(
            event_id,
            DEV_ORGANIZER_ID,
            Uuid::new_v4(),
            1,
        ))
        .await;
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let server_owned_payloads = [
        serde_json::json!({ "id": Uuid::new_v4() }),
        serde_json::json!({ "organizer_id": Uuid::new_v4() }),
        serde_json::json!({ "organizerId": Uuid::new_v4() }),
        serde_json::json!({ "status": "Cancelled" }),
        serde_json::json!({ "created_at": Utc::now().to_rfc3339() }),
        serde_json::json!({ "updated_at": Utc::now().to_rfc3339() }),
        serde_json::json!({ "version": 99 }),
        serde_json::json!({ "published_at": Utc::now().to_rfc3339() }),
        serde_json::json!({ "unexpected_field": "nope" }),
        serde_json::json!({}),
        serde_json::json!({ "name": "   " }),
        serde_json::json!({ "date": (Utc::now() - Duration::days(1)).to_rfc3339() }),
    ];

    for payload in server_owned_payloads {
        let response = app
            .clone()
            .oneshot(patch_event_request(
                &format!("/api/v1/events/{}", event_id),
                Body::from(serde_json::to_vec(&payload).unwrap()),
                Some(bearer(DEV_ORGANIZER_ID)),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{payload}");
    }

    let bad_json = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from("{not-json"),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(bad_json.status(), StatusCode::BAD_REQUEST);

    let bad_uuid = app
        .oneshot(patch_event_request(
            "/api/v1/events/not-a-uuid",
            Body::from(r#"{"name":"Test"}"#),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(bad_uuid.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_http_patch_event_schedule_targeting_and_venue_422() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let valid_venue = Uuid::new_v4();
    let venue_client = Arc::new(MockVenueClient::new_with_venues(HashSet::from([
        valid_venue,
    ])));

    let event_id = Uuid::new_v4();
    let multi = create_owned_event(event_id, DEV_ORGANIZER_ID, valid_venue, 2);
    let target_schedule_id = multi.schedules[0].id;
    let untouched_schedule_time = multi.schedules[1].starts_at;
    mock_repo.insert(multi).await;

    let app = create_test_app(mock_repo, venue_client);
    let next_date = Utc::now() + Duration::days(40);

    let missing_target = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(
                serde_json::to_vec(&serde_json::json!({ "date": next_date.to_rfc3339() })).unwrap(),
            ),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(missing_target.status(), StatusCode::BAD_REQUEST);

    let wrong_target = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "schedule_id": Uuid::new_v4(),
                    "date": next_date.to_rfc3339()
                }))
                .unwrap(),
            ),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_target.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let invalid_venue = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "venue_id": Uuid::new_v4()
                }))
                .unwrap(),
            ),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_venue.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let valid_targeted = app
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "schedule_id": target_schedule_id,
                    "date": next_date.to_rfc3339()
                }))
                .unwrap(),
            ),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();
    assert_eq!(valid_targeted.status(), StatusCode::OK);
    let body = to_bytes(valid_targeted.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: EventResponseDto = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.schedules[0].starts_at, next_date);
    assert_eq!(updated.schedules[1].starts_at, untouched_schedule_time);
}

#[tokio::test]
async fn test_http_patch_event_persistence_failure_500() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let event_id = Uuid::new_v4();
    let original = create_owned_event(event_id, DEV_ORGANIZER_ID, Uuid::new_v4(), 1);
    mock_repo.insert(original.clone()).await;
    mock_repo.set_fail_next_update(true);

    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));
    let response = app
        .clone()
        .oneshot(patch_event_request(
            &format!("/api/v1/events/{}", event_id),
            Body::from(r#"{"name":"Rolled Back"}"#),
            Some(bearer(DEV_ORGANIZER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error_message(response).await, "Internal server error");

    let detail = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/events/{}", event_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail_body = to_bytes(detail.into_body(), usize::MAX).await.unwrap();
    let persisted: EventResponseDto = serde_json::from_slice(&detail_body).unwrap();
    assert_eq!(persisted.name, original.name);
}

#[tokio::test]
async fn test_http_openapi_includes_patch_event() {
    let app = create_test_app(
        Arc::new(MockEventRepository::new()),
        Arc::new(MockVenueClient::new_permissive()),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let spec: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(spec["paths"]["/api/v1/events/{id}"]["patch"].is_object());
    assert_eq!(
        spec["paths"]["/api/v1/events/{id}"]["patch"]["security"][0]["bearer_auth"],
        serde_json::json!([])
    );
    assert!(spec["components"]["schemas"]["UpdateEventRequestDto"].is_object());
}

// ==================== Unparseable requests (ADR-010) ====================

/// Asserts a `400` with `Content-Type: application/json` and a non-empty `error` string.
async fn assert_json_400(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(!error_message(response).await.is_empty());
}

fn organizer_post(body: &str, content_type: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/events")
        .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID));
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn valid_create_body() -> serde_json::Value {
    serde_json::json!({
        "name": "Muse World Tour 2026",
        "artist": "Muse",
        "date": (Utc::now() + Duration::days(45)).to_rfc3339(),
        "venue_id": Uuid::new_v4().to_string(),
    })
}

fn test_app() -> Router {
    create_test_app(
        Arc::new(MockEventRepository::new()),
        Arc::new(MockVenueClient::new_permissive()),
    )
}

#[tokio::test]
async fn test_http_invalid_json_body_is_json_400() {
    let response = test_app()
        .oneshot(organizer_post("{not json", Some("application/json")))
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_wrong_type_in_body_is_json_400() {
    let mut body = valid_create_body();
    body["name"] = serde_json::json!(42);
    let response = test_app()
        .oneshot(organizer_post(&body.to_string(), Some("application/json")))
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_missing_required_field_is_json_400() {
    let mut body = valid_create_body();
    body.as_object_mut().unwrap().remove("name");
    let response = test_app()
        .oneshot(organizer_post(&body.to_string(), Some("application/json")))
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_missing_content_type_is_json_400() {
    let response = test_app()
        .oneshot(organizer_post(&valid_create_body().to_string(), None))
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_malformed_query_value_is_json_400() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/events?page=abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_post_date_without_offset_is_json_422() {
    let mut body = valid_create_body();
    body["date"] = serde_json::json!("2099-12-01T10:00:00");
    let response = test_app()
        .oneshot(organizer_post(&body.to_string(), Some("application/json")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(error_message(response).await.contains("timezone offset"));
}

#[tokio::test]
async fn test_http_post_date_as_number_is_json_400() {
    let mut body = valid_create_body();
    body["date"] = serde_json::json!(1700000000);
    let response = test_app()
        .oneshot(organizer_post(&body.to_string(), Some("application/json")))
        .await
        .unwrap();
    assert_json_400(response).await;
}

#[tokio::test]
async fn test_http_patch_date_without_offset_is_json_422() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let mut event = create_sample_event(Uuid::new_v4(), "Show");
    event.organizer_id = DEV_ORGANIZER_ID;
    let event_id = event.id;
    mock_repo.insert(event).await;
    let app = create_test_app(mock_repo, Arc::new(MockVenueClient::new_permissive()));

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/events/{event_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, bearer(DEV_ORGANIZER_ID))
                .body(Body::from(r#"{"date":"2099-12-01T10:00:00"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(error_message(response).await.contains("timezone offset"));
}
