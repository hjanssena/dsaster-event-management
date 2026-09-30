use std::sync::Arc;
use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use eventManagement_api::{
    core::error::AppError,
    model::dtos::{
        EventMediaDto, EventPaginationQueryDto, EventPricingTierDto, EventResponseDto,
        EventSaleDto, EventScheduleDto,
    },
    repository::MockEventRepository,
    service::EventService,
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

#[tokio::test]
async fn test_get_event_by_id_success() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let event_id = Uuid::new_v4();
    let venue_id = Uuid::new_v4();
    let sample = create_sample_event(event_id, "Festival Metal", "Scheduled", venue_id);

    mock_repo.insert(sample.clone()).await;

    let service = EventService::new(mock_repo);
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
    let service = EventService::new(mock_repo);
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
    let venue_id = Uuid::new_v4();

    // Insertar 15 eventos
    for i in 1..=15 {
        let ev = create_sample_event(
            Uuid::new_v4(),
            &format!("Show #{}", i),
            "Scheduled",
            venue_id,
        );
        mock_repo.insert(ev).await;
    }

    let service = EventService::new(mock_repo);

    // Consulta sin parámetros explícitos (debe aplicar page=1 y per_page=10)
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
    let service = EventService::new(mock_repo);

    let query = EventPaginationQueryDto {
        page: Some(1),
        per_page: Some(500), // Excede el máximo permitido de 100
        status: None,
        venue_id: None,
        organizer_id: None,
        id: None,
    };

    let result = service.get_events(query).await.unwrap();

    // El servicio debe acotar per_page a 100
    assert_eq!(result.per_page, 100);
}

#[tokio::test]
async fn test_get_events_filter_by_id_found() {
    let mock_repo = Arc::new(MockEventRepository::new());
    let target_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let venue_id = Uuid::new_v4();

    mock_repo
        .insert(create_sample_event(target_id, "Target Show", "Scheduled", venue_id))
        .await;
    mock_repo
        .insert(create_sample_event(other_id, "Other Show", "Draft", venue_id))
        .await;

    let service = EventService::new(mock_repo);

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
    let existing_id = Uuid::new_v4();
    let non_existing_id = Uuid::new_v4();

    mock_repo
        .insert(create_sample_event(existing_id, "Show", "Scheduled", Uuid::new_v4()))
        .await;

    let service = EventService::new(mock_repo);

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

    let service = EventService::new(mock_repo);

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
