use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

use eventManagement_api::{
    config::PaginationConfig,
    core::error::AppError,
    model::dtos::{
        CreateEventRequestDto, EventMediaDto, EventPaginationQueryDto, EventPricingTierDto,
        EventResponseDto, EventSaleDto, EventScheduleDto, RequestTime, UpdateEventRequestDto,
    },
    model::partner::{self, PartnerRole},
    repository::MockEventRepository,
    service::{EventService, MockVenueClient, request_time::parse_request_time},
};

/// Page sizes passed explicitly, never relying on defaults (ADR-006).
fn pagination(default_page_size: u64, max_page_size: u64) -> PaginationConfig {
    PaginationConfig {
        default_page_size,
        max_page_size,
    }
}

fn create_partner(role: PartnerRole) -> partner::Model {
    partner::Model {
        id: Uuid::new_v4(),
        username: format!("{:?}_test", role),
        role,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

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

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));
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
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));
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

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

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
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

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
        .insert(create_sample_event(
            target_id,
            "Target Show",
            "Scheduled",
            venue_id,
        ))
        .await;
    mock_repo
        .insert(create_sample_event(
            other_id,
            "Other Show",
            "Draft",
            venue_id,
        ))
        .await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

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
        .insert(create_sample_event(
            existing_id,
            "Show",
            "Scheduled",
            Uuid::new_v4(),
        ))
        .await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

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
        .insert(create_sample_event(
            Uuid::new_v4(),
            "Show 1",
            "Draft",
            venue_id,
        ))
        .await;
    mock_repo
        .insert(create_sample_event(
            Uuid::new_v4(),
            "Show 2",
            "Scheduled",
            venue_id,
        ))
        .await;
    mock_repo
        .insert(create_sample_event(
            Uuid::new_v4(),
            "Show 3",
            "Scheduled",
            venue_id,
        ))
        .await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

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
    let service = EventService::new(mock_repo.clone(), venue_client, pagination(10, 100));

    let venue_id = Uuid::new_v4();
    let future_date = Utc::now() + Duration::days(30);

    let organizer = create_partner(PartnerRole::Organizer);
    let request = CreateEventRequestDto {
        name: "Arctic Monkeys Live".to_string(),
        artist: "Arctic Monkeys".to_string(),
        date: (future_date).into(),
        venue_id,
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let confirmation = service.create_event(request, &organizer).await.unwrap();

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
    assert_eq!(queried.event_type, "Concert"); // Default aplicado
    assert_eq!(queried.organizer_id, organizer.id); // Atribuido al organizador autenticado
}

#[tokio::test]
async fn test_create_event_empty_name() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let organizer = create_partner(PartnerRole::Organizer);
    let request = CreateEventRequestDto {
        name: "   ".to_string(), // Inválido: solo espacios
        artist: "Coldplay".to_string(),
        date: (Utc::now() + Duration::days(10)).into(),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service.create_event(request, &organizer).await.unwrap_err();
    match err {
        AppError::BadRequest(msg) => assert!(msg.contains("name cannot be empty")),
        _ => panic!("Se esperaba BadRequest, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_empty_artist() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let organizer = create_partner(PartnerRole::Organizer);
    let request = CreateEventRequestDto {
        name: "Lollapalooza 2026".to_string(),
        artist: "".to_string(), // Inválido: vacío
        date: (Utc::now() + Duration::days(10)).into(),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service.create_event(request, &organizer).await.unwrap_err();
    match err {
        AppError::BadRequest(msg) => assert!(msg.contains("Artist name cannot be empty")),
        _ => panic!("Se esperaba BadRequest, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_past_date() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let organizer = create_partner(PartnerRole::Organizer);
    let request = CreateEventRequestDto {
        name: "Concierto Antiguo".to_string(),
        artist: "The Beatles".to_string(),
        date: (Utc::now() - Duration::days(1)).into(), // Inválido: fecha pasada
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service.create_event(request, &organizer).await.unwrap_err();
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
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let unknown_venue = Uuid::new_v4();
    let organizer = create_partner(PartnerRole::Organizer);
    let request = CreateEventRequestDto {
        name: "Show en Recinto Desconocido".to_string(),
        artist: "Radiohead".to_string(),
        date: (Utc::now() + Duration::days(15)).into(),
        venue_id: unknown_venue, // No existe en Venue Service
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service.create_event(request, &organizer).await.unwrap_err();
    match err {
        AppError::UnprocessableEntity(msg) => {
            assert!(msg.contains("Venue does not exist or is invalid"))
        }
        _ => panic!("Se esperaba UnprocessableEntity, se obtuvo: {:?}", err),
    }
}

#[tokio::test]
async fn test_create_event_venue_owner_forbidden() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let service = EventService::new(mock_repo.clone(), venue_client, pagination(10, 100));

    let venue_owner = create_partner(PartnerRole::VenueOwner);
    let request = CreateEventRequestDto {
        name: "Evento de Venue Owner".to_string(),
        artist: "Artista".to_string(),
        date: (Utc::now() + Duration::days(10)).into(),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service
        .create_event(request, &venue_owner)
        .await
        .unwrap_err();
    match err {
        AppError::Forbidden(msg) => assert!(msg.contains("Only organizers")),
        _ => panic!("Se esperaba Forbidden, se obtuvo: {:?}", err),
    }

    // No se persistió ningún evento
    let page = service
        .get_events(EventPaginationQueryDto {
            page: None,
            per_page: None,
            status: None,
            venue_id: None,
            organizer_id: None,
            id: None,
        })
        .await
        .unwrap();
    assert_eq!(page.total_items, 0);
}

fn create_owned_event_with_schedules(
    id: Uuid,
    organizer_id: Uuid,
    venue_id: Uuid,
    schedule_count: usize,
) -> EventResponseDto {
    let base_time = Utc::now() - Duration::days(1);
    let schedules = (0..schedule_count)
        .map(|idx| EventScheduleDto {
            id: Uuid::new_v4(),
            starts_at: Utc::now() + Duration::days(10 + idx as i64),
            ends_at: None,
        })
        .collect();

    EventResponseDto {
        id,
        organizer_id,
        venue_id,
        name: "Original Event".to_string(),
        description: Some("Initial description".to_string()),
        event_type: "Concert".to_string(),
        age_policy: "All ages".to_string(),
        status: "Scheduled".to_string(),
        terms: None,
        artist: Some("Original Artist".to_string()),
        schedules,
        pricing_tiers: vec![],
        sales: vec![],
        media: vec![],
        created_at: base_time,
        updated_at: base_time,
    }
}

#[tokio::test]
async fn test_update_event_full_success_preserves_server_fields() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let old_venue = Uuid::new_v4();
    let new_venue = Uuid::new_v4();
    let allowed = HashSet::from([new_venue]);
    let venue_client = Arc::new(MockVenueClient::new_with_venues(allowed));

    let organizer = create_partner(PartnerRole::Organizer);
    let event_id = Uuid::new_v4();
    let original = create_owned_event_with_schedules(event_id, organizer.id, old_venue, 1);
    mock_repo.insert(original.clone()).await;

    let new_date = Utc::now() + Duration::days(60);
    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let updated = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: Some("  Updated Event  ".to_string()),
                artist: Some("  Updated Artist  ".to_string()),
                date: Some((new_date).into()),
                schedule_id: None,
                venue_id: Some(new_venue),
            },
            &organizer,
        )
        .await
        .unwrap();

    assert_eq!(updated.id, original.id);
    assert_eq!(updated.organizer_id, original.organizer_id);
    assert_eq!(updated.status, original.status);
    assert_eq!(updated.created_at, original.created_at);
    assert!(updated.updated_at > original.updated_at);
    assert_eq!(updated.name, "Updated Event");
    assert_eq!(updated.artist.as_deref(), Some("Updated Artist"));
    assert_eq!(updated.venue_id, new_venue);
    assert_eq!(updated.schedules.len(), 1);
    assert_eq!(updated.schedules[0].id, original.schedules[0].id);
    assert_eq!(updated.schedules[0].starts_at, new_date);
}

#[tokio::test]
async fn test_update_event_partial_metadata_skips_venue_verification() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let unverified_venue = Uuid::new_v4();
    let venue_client = Arc::new(MockVenueClient::new_with_venues(HashSet::new()));

    let organizer = create_partner(PartnerRole::Organizer);
    let event_id = Uuid::new_v4();
    let original = create_owned_event_with_schedules(event_id, organizer.id, unverified_venue, 1);
    mock_repo.insert(original.clone()).await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let updated = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: Some("Renamed Only".to_string()),
                artist: None,
                date: None,
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "Renamed Only");
    assert_eq!(updated.artist, original.artist);
    assert_eq!(updated.venue_id, unverified_venue);
    assert_eq!(updated.schedules, original.schedules);

    let same_venue_updated = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: None,
                schedule_id: None,
                venue_id: Some(unverified_venue),
            },
            &organizer,
        )
        .await
        .unwrap();
    assert_eq!(same_venue_updated.venue_id, unverified_venue);
}

#[tokio::test]
async fn test_update_event_role_and_ownership_forbidden() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());

    let owner_organizer = create_partner(PartnerRole::Organizer);
    let other_organizer = create_partner(PartnerRole::Organizer);
    let venue_owner = create_partner(PartnerRole::VenueOwner);
    let event_id = Uuid::new_v4();
    let original =
        create_owned_event_with_schedules(event_id, owner_organizer.id, Uuid::new_v4(), 1);
    mock_repo.insert(original.clone()).await;

    let service = EventService::new(mock_repo.clone(), venue_client, pagination(10, 100));
    let request = UpdateEventRequestDto {
        name: Some("Hijacked".to_string()),
        artist: None,
        date: None,
        schedule_id: None,
        venue_id: None,
    };

    let err_role = service
        .update_event(event_id, request.clone(), &venue_owner)
        .await
        .unwrap_err();
    assert!(matches!(err_role, AppError::Forbidden(_)));

    let err_owner = service
        .update_event(event_id, request, &other_organizer)
        .await
        .unwrap_err();
    assert!(matches!(err_owner, AppError::Forbidden(_)));

    let persisted = service.get_event_by_id(event_id).await.unwrap();
    assert_eq!(persisted.name, original.name);
}

#[tokio::test]
async fn test_update_event_not_found_and_validation_errors() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let organizer = create_partner(PartnerRole::Organizer);
    let event_id = Uuid::new_v4();
    let original = create_owned_event_with_schedules(event_id, organizer.id, Uuid::new_v4(), 1);
    mock_repo.insert(original).await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));

    let missing_err = service
        .update_event(
            Uuid::new_v4(),
            UpdateEventRequestDto {
                name: Some("Valid".to_string()),
                artist: None,
                date: None,
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(missing_err, AppError::NotFound(_)));

    let invalid_payloads = [
        UpdateEventRequestDto {
            name: None,
            artist: None,
            date: None,
            schedule_id: None,
            venue_id: None,
        },
        UpdateEventRequestDto {
            name: Some("   ".to_string()),
            artist: None,
            date: None,
            schedule_id: None,
            venue_id: None,
        },
        UpdateEventRequestDto {
            name: Some("a".repeat(256)),
            artist: None,
            date: None,
            schedule_id: None,
            venue_id: None,
        },
        UpdateEventRequestDto {
            name: None,
            artist: Some("".to_string()),
            date: None,
            schedule_id: None,
            venue_id: None,
        },
        UpdateEventRequestDto {
            name: None,
            artist: None,
            date: Some((Utc::now() - Duration::hours(1)).into()),
            schedule_id: None,
            venue_id: None,
        },
        UpdateEventRequestDto {
            name: None,
            artist: None,
            date: None,
            schedule_id: Some(Uuid::new_v4()),
            venue_id: None,
        },
    ];

    for payload in invalid_payloads {
        let err = service
            .update_event(event_id, payload, &organizer)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }
}

#[tokio::test]
async fn test_update_event_explicit_schedule_targeting() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_client = Arc::new(MockVenueClient::new_permissive());
    let organizer = create_partner(PartnerRole::Organizer);
    let multi_id = Uuid::new_v4();
    let multi_event = create_owned_event_with_schedules(multi_id, organizer.id, Uuid::new_v4(), 2);
    let target_schedule_id = multi_event.schedules[1].id;
    let sibling_starts_at = multi_event.schedules[0].starts_at;
    mock_repo.insert(multi_event).await;

    let empty_sched_id = Uuid::new_v4();
    let empty_sched_event =
        create_owned_event_with_schedules(empty_sched_id, organizer.id, Uuid::new_v4(), 0);
    mock_repo.insert(empty_sched_event).await;

    let service = EventService::new(mock_repo, venue_client, pagination(10, 100));
    let future_date = Utc::now() + Duration::days(90);

    let ambiguous_err = service
        .update_event(
            multi_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: Some((future_date).into()),
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(ambiguous_err, AppError::BadRequest(_)));

    let foreign_err = service
        .update_event(
            multi_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: Some((future_date).into()),
                schedule_id: Some(Uuid::new_v4()),
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(foreign_err, AppError::UnprocessableEntity(_)));

    let zero_err = service
        .update_event(
            empty_sched_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: Some((future_date).into()),
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(zero_err, AppError::UnprocessableEntity(_)));

    let updated = service
        .update_event(
            multi_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: Some((future_date).into()),
                schedule_id: Some(target_schedule_id),
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap();

    assert_eq!(updated.schedules[0].starts_at, sibling_starts_at);
    assert_eq!(updated.schedules[1].starts_at, future_date);
}

#[tokio::test]
async fn test_update_event_invalid_changed_venue_and_persistence_failure() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let valid_venue = Uuid::new_v4();
    let venue_client = Arc::new(MockVenueClient::new_with_venues(HashSet::from([
        valid_venue,
    ])));
    let organizer = create_partner(PartnerRole::Organizer);
    let event_id = Uuid::new_v4();
    let original = create_owned_event_with_schedules(event_id, organizer.id, valid_venue, 1);
    mock_repo.insert(original.clone()).await;

    let service = EventService::new(mock_repo.clone(), venue_client, pagination(10, 100));

    let venue_err = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: Some("Will Fail Venue".to_string()),
                artist: None,
                date: None,
                schedule_id: None,
                venue_id: Some(Uuid::new_v4()),
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(venue_err, AppError::UnprocessableEntity(_)));

    mock_repo.set_fail_next_update(true);
    let persistence_err = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: Some("Will Fail Persistence".to_string()),
                artist: None,
                date: Some((Utc::now() + Duration::days(30)).into()),
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(persistence_err, AppError::InternalServerError(_)));

    let stored = service.get_event_by_id(event_id).await.unwrap();
    assert_eq!(stored.name, original.name);
    assert_eq!(stored.venue_id, original.venue_id);
    assert_eq!(
        stored.schedules[0].starts_at,
        original.schedules[0].starts_at
    );
}

// ==================== Request times (ADR-010) ====================

#[test]
fn test_request_time_with_offset_is_converted_to_utc() {
    let parsed =
        parse_request_time(&RequestTime("2099-12-01T10:00:00-06:00".into()), "date").unwrap();
    assert_eq!(parsed.to_rfc3339(), "2099-12-01T16:00:00+00:00");

    let zulu = parse_request_time(&RequestTime("2099-12-01T16:00:00Z".into()), "date").unwrap();
    assert_eq!(parsed, zulu);
}

#[test]
fn test_request_time_without_offset_is_422() {
    let err = parse_request_time(&RequestTime("2099-12-01T10:00:00".into()), "date").unwrap_err();
    match err {
        AppError::UnprocessableEntity(msg) => {
            assert!(msg.contains("date"), "{msg}");
            assert!(msg.contains("timezone offset"), "{msg}");
        }
        other => panic!("expected 422, got {other:?}"),
    }
}

#[test]
fn test_request_time_that_is_not_a_time_is_400() {
    let err = parse_request_time(&RequestTime("tomorrow".into()), "date").unwrap_err();
    assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
}

#[test]
fn test_request_time_that_is_not_a_string_is_rejected_by_parsing() {
    // A JSON number never becomes a RequestTime; the body extractor turns this into a 400.
    let parsed: Result<RequestTime, _> = serde_json::from_str("1700000000");
    assert!(parsed.is_err());
}

#[tokio::test]
async fn test_create_event_date_without_offset_is_422() {
    let service = EventService::new(
        Arc::new(MockEventRepository::new()),
        Arc::new(MockVenueClient::new_permissive()),
        pagination(10, 100),
    );
    let request = CreateEventRequestDto {
        name: "Arctic Monkeys Live".to_string(),
        artist: "Arctic Monkeys".to_string(),
        date: RequestTime("2099-12-01T10:00:00".into()),
        venue_id: Uuid::new_v4(),
        description: None,
        age_policy: None,
        event_type: None,
        terms: None,
    };

    let err = service
        .create_event(request, &create_partner(PartnerRole::Organizer))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::UnprocessableEntity(_)), "{err:?}");
}

#[tokio::test]
async fn test_update_event_date_without_offset_is_422() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let organizer = create_partner(PartnerRole::Organizer);
    let mut event = create_sample_event(Uuid::new_v4(), "Show", "Scheduled", Uuid::new_v4());
    event.organizer_id = organizer.id;
    let event_id = event.id;
    mock_repo.insert(event).await;
    let service = EventService::new(
        mock_repo,
        Arc::new(MockVenueClient::new_permissive()),
        pagination(10, 100),
    );

    let err = service
        .update_event(
            event_id,
            UpdateEventRequestDto {
                name: None,
                artist: None,
                date: Some(RequestTime("2099-12-01T10:00:00".into())),
                schedule_id: None,
                venue_id: None,
            },
            &organizer,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::UnprocessableEntity(_)), "{err:?}");
}

// ==================== Configured page sizes (ADR-006) ====================

async fn service_with_events(count: usize, page_sizes: PaginationConfig) -> EventService {
    let mock_repo = Arc::new(MockEventRepository::new());
    let venue_id = Uuid::new_v4();
    for i in 1..=count {
        mock_repo
            .insert(create_sample_event(
                Uuid::new_v4(),
                &format!("Show #{i}"),
                "Scheduled",
                venue_id,
            ))
            .await;
    }
    EventService::new(
        mock_repo,
        Arc::new(MockVenueClient::new_permissive()),
        page_sizes,
    )
}

fn page_query(page: Option<u64>, per_page: Option<u64>) -> EventPaginationQueryDto {
    EventPaginationQueryDto {
        page,
        per_page,
        status: None,
        venue_id: None,
        organizer_id: None,
        id: None,
    }
}

#[tokio::test]
async fn test_get_events_uses_configured_default_page_size() {
    let service = service_with_events(20, pagination(7, 50)).await;
    let result = service.get_events(page_query(None, None)).await.unwrap();

    assert_eq!(result.per_page, 7);
    assert_eq!(result.items.len(), 7);
    assert_eq!(result.total_pages, 3);
}

#[tokio::test]
async fn test_get_events_clamps_to_configured_maximum() {
    let service = service_with_events(60, pagination(7, 50)).await;
    let result = service
        .get_events(page_query(None, Some(500)))
        .await
        .unwrap();

    assert_eq!(result.per_page, 50);
    assert_eq!(result.items.len(), 50);
}

#[tokio::test]
async fn test_get_events_page_zero_is_page_one() {
    let service = service_with_events(3, pagination(7, 50)).await;
    let result = service.get_events(page_query(Some(0), None)).await.unwrap();

    assert_eq!(result.page, 1);
    assert_eq!(result.items.len(), 3);
}
