use std::collections::HashSet;
use std::sync::Arc;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use eventManagement_api::{
    core::error::AppError,
    model::dtos::{
        CreateEventRequestDto, EventMediaDto, EventPaginationQueryDto, EventPricingTierDto,
        EventResponseDto, EventSaleDto, EventScheduleDto,
    },
    repository::MockEventRepository,
    service::{EventService, MockVenueClient},
};

fn create_sample_event(id: Uuid, name: &str, status: &str, venue_id: Uuid) -> EventResponseDto {
    EventResponseDto {
        id,
        organizer_id: Uuid::new_v4(),
        venue_id,
        name: name.to_string(),
        description: Some("Concierto épico en vivo".to_string()),
        event_type: "Concert".to_string(),
        age_policy: "18+".to_string(),
        status: status.to_string(),
        terms: Some("No reembolsos".to_string()),
        artist: Some("D-Saster Band".to_string()),
        schedules: vec![EventScheduleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now(),
            ends_at: Some(Utc::now()),
        }],
        pricing_tiers: vec![
            EventPricingTierDto {
                id: Uuid::new_v4(),
                name: "General".to_string(),
                description: Some("Acceso general".to_string()),
                price: Decimal::new(5000, 2), // 50.00
                currency: "USD".to_string(),
            },
            EventPricingTierDto {
                id: Uuid::new_v4(),
                name: "VIP".to_string(),
                description: Some("Zona VIP".to_string()),
                price: Decimal::new(15000, 2), // 150.00
                currency: "USD".to_string(),
            },
        ],
        sales: vec![EventSaleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now(),
            ends_at: Some(Utc::now()),
        }],
        media: vec![EventMediaDto {
            id: Uuid::new_v4(),
            media_type: "banner".to_string(),
            url: "https://cdn.dsaster.com/banner.png".to_string(),
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// ==================== Tests de Lectura (GET /events) ====================

#[tokio::test]
async fn test_get_event_by_id_success() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let event_id = Uuid::new_v4();
    let venue_id = Uuid::new_v4();
    let sample = create_sample_event(event_id, "Festival Metal", "Scheduled", venue_id);

    mock_repo.insert(sample.clone()).await;

    let service = EventService::new(mock_repo, venue_client);
    let result = service.get_event_by_id(event_id).await.unwrap();

    assert_eq!(result.id, event_id);
    assert_eq!(result.name, "Festival Metal");
    assert_eq!(result.status, "Scheduled");
    assert_eq!(result.pricing_tiers.len(), 2);
    assert_eq!(result.schedules.len(), 1);
    assert_eq!(result.media.len(), 1);
}

#[tokio::test]
async fn test_get_event_by_id_not_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client);
    let non_existent_id = Uuid::new_v4();

    let err = service.get_event_by_id(non_existent_id).await.unwrap_err();

    match err {
        AppError::NotFound(msg) => {
            assert!(msg.contains(&non_existent_id.to_string()));
        }
        _ => panic!("Se esperaba AppError::NotFound, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_get_events_pagination_defaults() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let venue_id = Uuid::new_v4();

    for i in 1..=15 {
        let ev = create_sample_event(
            Uuid::new_v4(),
            &format!("Show #{}", i),
            "Scheduled",
            venue_id,
        );
        mock_repo.insert(ev).await;
    }

    let service = EventService::new(mock_repo, venue_client);

    let query = EventPaginationQueryDto {
        page: None,
        per_page: None,
        status: None,
        venue_id: None,
        organizer_id: None,
        id: None,
    };

    let result = service.get_events(query).await.unwrap();

    assert_eq!(result.page, 1);
    assert_eq!(result.per_page, 10);
    assert_eq!(result.total_items, 15);
    assert_eq!(result.total_pages, 2);
    assert_eq!(result.items.len(), 10);
    assert_eq!(result.items[0].min_price, Some(Decimal::new(5000, 2)));
}

#[tokio::test]
async fn test_get_events_clamp_max_per_page() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client);

    let query = EventPaginationQueryDto {
        page: Some(1),
        per_page: Some(500),
        status: None,
        venue_id: None,
        organizer_id: None,
        id: None,
    };

    let result = service.get_events(query).await.unwrap();
    assert_eq!(result.per_page, 100);
}

#[tokio::test]
async fn test_get_events_filter_by_id_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let target_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let venue_id = Uuid::new_v4();

    mock_repo
        .insert(create_sample_event(target_id, "Target Show", "Scheduled", venue_id))
        .await;
    mock_repo
        .insert(create_sample_event(other_id, "Other Show", "Draft", venue_id))
        .await;

    let service = EventService::new(mock_repo, venue_client);

    let query = EventPaginationQueryDto {
        page: None,
        per_page: None,
        status: None,
        venue_id: None,
        organizer_id: None,
        id: Some(target_id),
    };

    let result = service.get_events(query).await.unwrap();

    assert_eq!(result.total_items, 1);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, target_id);
    assert_eq!(result.items[0].name, "Target Show");
}

#[tokio::test]
async fn test_get_events_filter_by_id_not_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let existing_id = Uuid::new_v4();
    let non_existing_id = Uuid::new_v4();

    mock_repo
        .insert(create_sample_event(existing_id, "Show", "Scheduled", Uuid::new_v4()))
        .await;

    let service = EventService::new(mock_repo, venue_client);

    let query = EventPaginationQueryDto {
        page: None,
        per_page: None,
        status: None,
        venue_id: None,
        organizer_id: None,
        id: Some(non_existing_id),
    };

    let result = service.get_events(query).await.unwrap();

    assert_eq!(result.total_items, 0);
    assert!(result.items.is_empty());
}

#[tokio::test]
async fn test_get_events_filter_by_status() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let venue_id = Uuid::new_v4();

    mock_repo
        .insert(create_sample_event(Uuid::new_v4(), "Show 1", "Draft", venue_id))
        .await;
    mock_repo
        .insert(create_sample_event(Uuid::new_v4(), "Show 2", "Scheduled", venue_id))
        .await;
    mock_repo
        .insert(create_sample_event(Uuid::new_v4(), "Show 3", "Scheduled", venue_id))
        .await;

    let service = EventService::new(mock_repo, venue_client);

    let query = EventPaginationQueryDto {
        page: None,
        per_page: None,
        status: Some("Scheduled".to_string()),
        venue_id: None,
        organizer_id: None,
        id: None,
    };

    let result = service.get_events(query).await.unwrap();

    assert_eq!(result.total_items, 2);
    assert_eq!(result.items.len(), 2);
    for item in result.items {
        assert_eq!(item.status, "Scheduled");
    }
}

// ==================== Tests de Registro (POST /events) ====================

#[tokio::test]
async fn test_create_event_success_minimal_fields() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo.clone(), venue_client);

    let venue_id = Uuid::new_v4();
    let future_date = Utc::now() + Duration::days(30);

    let request = CreateEventRequestDto {
        name: "Arctic Monkeys Live".to_string(),
        artist: "Arctic Monkeys".to_string(),
        date: future_date,
        venue_id,
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
        organizer_id: None,
    };

    let confirmation = service.create_event(request).await.unwrap();

    assert!(!confirmation.id.is_nil());
    assert_eq!(confirmation.name, "Arctic Monkeys Live");
    assert_eq!(confirmation.artist, "Arctic Monkeys");
    assert_eq!(confirmation.venue_id, venue_id);
    assert_eq!(confirmation.date, future_date);
    assert_eq!(confirmation.status, "Scheduled");
    assert_eq!(confirmation.message, "Event created successfully");

    // Verificar que el evento es inmediatamente consultable por ID
    let queried = service.get_event_by_id(confirmation.id).await.unwrap();
    assert_eq!(queried.name, "Arctic Monkeys Live");
    assert_eq!(queried.schedules[0].starts_at, future_date);
    assert_eq!(queried.age_policy, "All ages"); // Default aplicado
    assert_eq!(queried.event_type, "Concert");   // Default aplicado
}

#[tokio::test]
async fn test_create_event_empty_name() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client);

    let request = CreateEventRequestDto {
        name: "   ".to_string(), // Inválido: solo espacios
        artist: "Coldplay".to_string(),
        date: Utc::now() + Duration::days(10),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
        organizer_id: None,
    };

    let err = service.create_event(request).await.unwrap_err();
    match err {
        AppError::BadRequest(msg) => assert!(msg.contains("name cannot be empty")),
        _ => panic!("Se esperaba BadRequest, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_empty_artist() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client);

    let request = CreateEventRequestDto {
        name: "Lollapalooza 2026".to_string(),
        artist: "".to_string(), // Inválido: vacío
        date: Utc::now() + Duration::days(10),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
        organizer_id: None,
    };

    let err = service.create_event(request).await.unwrap_err();
    match err {
        AppError::BadRequest(msg) => assert!(msg.contains("Artist name cannot be empty")),
        _ => panic!("Se esperaba BadRequest, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_past_date() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client);

    let request = CreateEventRequestDto {
        name: "Concierto Antiguo".to_string(),
        artist: "The Beatles".to_string(),
        date: Utc::now() - Duration::days(1), // Inválido: fecha pasada
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
        organizer_id: None,
    };

    let err = service.create_event(request).await.unwrap_err();
    match err {
        AppError::BadRequest(msg) => assert!(msg.contains("valid future calendar date")),
        _ => panic!("Se esperaba BadRequest, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_venue_not_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    // Mock Venue Client estricto: solo conoce un conjunto específico de recintos
    let valid_venue_id = Uuid::new_v4();
    let mut allowed = HashSet::new();
    allowed.insert(valid_venue_id);

    let venue_client = Arc::new(MockVenueClient::new_with_venues(allowed));
    let service = EventService::new(mock_repo, venue_client);

    let unknown_venue = Uuid::new_v4();
    let request = CreateEventRequestDto {
        name: "Show en Recinto Desconocido".to_string(),
        artist: "Radiohead".to_string(),
        date: Utc::now() + Duration::days(15),
        venue_id: unknown_venue, // No existe en Venue Service
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
        organizer_id: None,
    };

    let err = service.create_event(request).await.unwrap_err();
    match err {
        AppError::UnprocessableEntity(msg) => assert!(msg.contains("Venue does not exist or is invalid")),
        _ => panic!("Se esperaba UnprocessableEntity, se obtuvo: {:?}", err),
    }
}
