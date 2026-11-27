use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, ModelTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::model::{
    dtos::{
        CreateEventRequestDto, EventConfirmationDto, EventMediaDto, EventPaginationQueryDto,
        EventPricingTierDto, EventResponseDto, EventSaleDto, EventScheduleDto, EventSummaryDto,
        EventUpdateChanges, PaginatedEventSummaryResponse, PaginatedResponseDto,
    },
    event, event_media, event_pricing_tier, event_sale, event_schedule,
};

/// Contrato para el acceso a datos de eventos
#[async_trait]
pub trait EventRepository: Send + Sync {
    /// Busca un evento por su ID con todas sus relaciones cargadas
    async fn find_by_id(&self, id: Uuid) -> Result<Option<EventResponseDto>, AppError>;

    /// Busca eventos de forma paginada aplicando filtros opcionales.
    /// `page` (from 1) and `per_page` come already validated from the service.
    async fn find_paginated(
        &self,
        query: &EventPaginationQueryDto,
        page: u64,
        per_page: u64,
    ) -> Result<PaginatedEventSummaryResponse, AppError>;

    /// Crea y persiste un evento de forma atómica en base de datos
    async fn create_event(
        &self,
        dto: &CreateEventRequestDto,
        date: DateTime<Utc>,
        event_id: Uuid,
        organizer_id: Uuid,
    ) -> Result<EventConfirmationDto, AppError>;

    async fn update_event(
        &self,
        id: Uuid,
        changes: &EventUpdateChanges,
    ) -> Result<Option<EventResponseDto>, AppError>;
}

async fn load_event_aggregate<C: ConnectionTrait>(
    conn: &C,
    event_model: event::Model,
) -> Result<EventResponseDto, AppError> {
    let schedules = event_model
        .find_related(event_schedule::Entity)
        .order_by_asc(event_schedule::Column::CreatedAt)
        .all(conn)
        .await?
        .into_iter()
        .map(|s| EventScheduleDto {
            id: s.id,
            starts_at: s.starts_at,
            ends_at: s.ends_at,
        })
        .collect();

    let pricing_tiers = event_model
        .find_related(event_pricing_tier::Entity)
        .all(conn)
        .await?
        .into_iter()
        .map(|p| EventPricingTierDto {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
            currency: p.currency,
        })
        .collect();

    let sales = event_model
        .find_related(event_sale::Entity)
        .all(conn)
        .await?
        .into_iter()
        .map(|s| EventSaleDto {
            id: s.id,
            starts_at: s.starts_at,
            ends_at: s.ends_at,
        })
        .collect();

    let media = event_model
        .find_related(event_media::Entity)
        .all(conn)
        .await?
        .into_iter()
        .map(|m| EventMediaDto {
            id: m.id,
            media_type: m.media_type,
            url: m.url,
        })
        .collect();

    Ok(EventResponseDto {
        id: event_model.id,
        organizer_id: event_model.organizer_id,
        venue_id: event_model.venue_id,
        name: event_model.name,
        description: event_model.description,
        event_type: event_model.event_type,
        age_policy: event_model.age_policy,
        status: event_model.status,
        terms: event_model.terms,
        artist: event_model.artist,
        schedules,
        pricing_tiers,
        sales,
        media,
        created_at: event_model.created_at,
        updated_at: event_model.updated_at,
    })
}

/// Implementación de producción respaldada por SeaORM y PostgreSQL/MySQL
pub struct SeaOrmEventRepository {
    db: DatabaseConnection,
}

impl SeaOrmEventRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventRepository for SeaOrmEventRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<EventResponseDto>, AppError> {
        let event_model = match event::Entity::find_by_id(id).one(&self.db).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        load_event_aggregate(&self.db, event_model).await.map(Some)
    }

    async fn find_paginated(
        &self,
        query: &EventPaginationQueryDto,
        page: u64,
        per_page: u64,
    ) -> Result<PaginatedEventSummaryResponse, AppError> {
        let mut select = event::Entity::find();

        if let Some(id) = query.id {
            select = select.filter(event::Column::Id.eq(id));
        }
        if let Some(ref status) = query.status {
            select = select.filter(event::Column::Status.eq(status));
        }
        if let Some(venue_id) = query.venue_id {
            select = select.filter(event::Column::VenueId.eq(venue_id));
        }
        if let Some(organizer_id) = query.organizer_id {
            select = select.filter(event::Column::OrganizerId.eq(organizer_id));
        }

        select = select.order_by_desc(event::Column::CreatedAt);

        let paginator = select.paginate(&self.db, per_page);
        let total_items = paginator.num_items().await?;
        let total_pages = paginator.num_pages().await?;

        let events_page = if total_items == 0 || (page - 1) >= total_pages {
            Vec::new()
        } else {
            paginator.fetch_page(page - 1).await?
        };

        let mut items = Vec::with_capacity(events_page.len());
        for ev in events_page {
            let schedules = ev
                .find_related(event_schedule::Entity)
                .all(&self.db)
                .await?
                .into_iter()
                .map(|s| EventScheduleDto {
                    id: s.id,
                    starts_at: s.starts_at,
                    ends_at: s.ends_at,
                })
                .collect();

            let pricing_tiers = ev
                .find_related(event_pricing_tier::Entity)
                .all(&self.db)
                .await?;

            let min_price = pricing_tiers.iter().map(|t| t.price).min();

            items.push(EventSummaryDto {
                id: ev.id,
                organizer_id: ev.organizer_id,
                venue_id: ev.venue_id,
                name: ev.name,
                event_type: ev.event_type,
                status: ev.status,
                artist: ev.artist,
                schedules,
                min_price,
            });
        }

        Ok(PaginatedResponseDto {
            items,
            page,
            per_page,
            total_items,
            total_pages,
        })
    }

    async fn create_event(
        &self,
        dto: &CreateEventRequestDto,
        date: DateTime<Utc>,
        event_id: Uuid,
        organizer_id: Uuid,
    ) -> Result<EventConfirmationDto, AppError> {
        let now = chrono::Utc::now();
        let status = "Scheduled".to_string();

        let tx = self.db.begin().await?;

        let event_active = event::ActiveModel {
            id: Set(event_id),
            organizer_id: Set(organizer_id),
            venue_id: Set(dto.venue_id),
            name: Set(dto.name.clone()),
            description: Set(dto.description.clone()),
            event_type: Set(dto
                .event_type
                .clone()
                .unwrap_or_else(|| "Concert".to_string())),
            age_policy: Set(dto
                .age_policy
                .clone()
                .unwrap_or_else(|| "All ages".to_string())),
            status: Set(status.clone()),
            terms: Set(dto.terms.clone()),
            artist: Set(Some(dto.artist.clone())),
            created_at: Set(now),
            updated_at: Set(now),
        };
        event_active.insert(&tx).await?;

        let schedule_active = event_schedule::ActiveModel {
            id: Set(Uuid::new_v4()),
            event_id: Set(event_id),
            starts_at: Set(date),
            ends_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        schedule_active.insert(&tx).await?;

        tx.commit().await?;

        Ok(EventConfirmationDto {
            id: event_id,
            message: "Event created successfully".to_string(),
            name: dto.name.clone(),
            artist: dto.artist.clone(),
            date,
            venue_id: dto.venue_id,
            status,
            created_at: now,
        })
    }

    async fn update_event(
        &self,
        id: Uuid,
        changes: &EventUpdateChanges,
    ) -> Result<Option<EventResponseDto>, AppError> {
        let now = chrono::Utc::now();
        let tx = self.db.begin().await?;

        let event_model = match event::Entity::find_by_id(id).one(&tx).await? {
            Some(model) => model,
            None => return Ok(None),
        };

        let mut event_active: event::ActiveModel = event_model.into();
        if let Some(ref name) = changes.name {
            event_active.name = Set(name.clone());
        }
        if let Some(ref artist) = changes.artist {
            event_active.artist = Set(Some(artist.clone()));
        }
        if let Some(venue_id) = changes.venue_id {
            event_active.venue_id = Set(venue_id);
        }
        event_active.updated_at = Set(now);

        let updated_event = event_active.update(&tx).await?;

        if let Some((schedule_id, new_starts_at)) = changes.schedule_update {
            let schedule_model = event_schedule::Entity::find_by_id(schedule_id)
                .filter(event_schedule::Column::EventId.eq(id))
                .one(&tx)
                .await?
                .ok_or_else(|| {
                    AppError::UnprocessableEntity(
                        "Target schedule does not belong to this event".to_string(),
                    )
                })?;

            let mut schedule_active: event_schedule::ActiveModel = schedule_model.into();
            schedule_active.starts_at = Set(new_starts_at);
            schedule_active.updated_at = Set(now);
            schedule_active.update(&tx).await?;
        }

        let aggregate = load_event_aggregate(&tx, updated_event).await?;
        tx.commit().await?;

        Ok(Some(aggregate))
    }
}

/// Implementación en memoria para pruebas unitarias sin dependencias externas
#[derive(Clone, Default)]
pub struct MockEventRepository {
    storage: Arc<RwLock<HashMap<Uuid, EventResponseDto>>>,
    fail_next_update: Arc<AtomicBool>,
}

impl MockEventRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            fail_next_update: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn insert(&self, event: EventResponseDto) {
        let mut lock = self.storage.write().await;
        lock.insert(event.id, event);
    }

    pub fn set_fail_next_update(&self, fail: bool) {
        self.fail_next_update.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl EventRepository for MockEventRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<EventResponseDto>, AppError> {
        let lock = self.storage.read().await;
        Ok(lock.get(&id).cloned())
    }

    async fn find_paginated(
        &self,
        query: &EventPaginationQueryDto,
        page: u64,
        per_page: u64,
    ) -> Result<PaginatedEventSummaryResponse, AppError> {
        let lock = self.storage.read().await;

        let mut filtered: Vec<EventResponseDto> = lock
            .values()
            .filter(|ev| {
                if query.id.is_some_and(|id| ev.id != id) {
                    return false;
                }
                if query
                    .status
                    .as_ref()
                    .is_some_and(|status| &ev.status != status)
                {
                    return false;
                }
                if query
                    .venue_id
                    .is_some_and(|venue_id| ev.venue_id != venue_id)
                {
                    return false;
                }
                if query
                    .organizer_id
                    .is_some_and(|organizer_id| ev.organizer_id != organizer_id)
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        // Ordenar por created_at descendente
        filtered.sort_by_key(|a| std::cmp::Reverse(a.created_at));

        let total_items = filtered.len() as u64;
        let total_pages = if total_items == 0 {
            0
        } else {
            total_items.div_ceil(per_page)
        };

        let start = ((page - 1) * per_page) as usize;
        let page_items = if start >= filtered.len() {
            Vec::new()
        } else {
            let end = (start + per_page as usize).min(filtered.len());
            filtered[start..end].to_vec()
        };

        let items = page_items
            .into_iter()
            .map(|ev| {
                let min_price = ev.pricing_tiers.iter().map(|p| p.price).min();
                EventSummaryDto {
                    id: ev.id,
                    organizer_id: ev.organizer_id,
                    venue_id: ev.venue_id,
                    name: ev.name,
                    event_type: ev.event_type,
                    status: ev.status,
                    artist: ev.artist,
                    schedules: ev.schedules,
                    min_price,
                }
            })
            .collect();

        Ok(PaginatedResponseDto {
            items,
            page,
            per_page,
            total_items,
            total_pages,
        })
    }

    async fn create_event(
        &self,
        dto: &CreateEventRequestDto,
        date: DateTime<Utc>,
        event_id: Uuid,
        organizer_id: Uuid,
    ) -> Result<EventConfirmationDto, AppError> {
        let now = chrono::Utc::now();
        let status = "Scheduled".to_string();

        let full_event = EventResponseDto {
            id: event_id,
            organizer_id,
            venue_id: dto.venue_id,
            name: dto.name.clone(),
            description: dto.description.clone(),
            event_type: dto
                .event_type
                .clone()
                .unwrap_or_else(|| "Concert".to_string()),
            age_policy: dto
                .age_policy
                .clone()
                .unwrap_or_else(|| "All ages".to_string()),
            status: status.clone(),
            terms: dto.terms.clone(),
            artist: Some(dto.artist.clone()),
            schedules: vec![EventScheduleDto {
                id: Uuid::new_v4(),
                starts_at: date,
                ends_at: None,
            }],
            pricing_tiers: vec![],
            sales: vec![],
            media: vec![],
            created_at: now,
            updated_at: now,
        };

        self.insert(full_event).await;

        Ok(EventConfirmationDto {
            id: event_id,
            message: "Event created successfully".to_string(),
            name: dto.name.clone(),
            artist: dto.artist.clone(),
            date,
            venue_id: dto.venue_id,
            status,
            created_at: now,
        })
    }

    async fn update_event(
        &self,
        id: Uuid,
        changes: &EventUpdateChanges,
    ) -> Result<Option<EventResponseDto>, AppError> {
        if self.fail_next_update.swap(false, Ordering::SeqCst) {
            return Err(AppError::InternalServerError(
                "Simulated persistence failure".to_string(),
            ));
        }

        let mut lock = self.storage.write().await;
        let Some(mut updated) = lock.get(&id).cloned() else {
            return Ok(None);
        };

        if let Some(ref name) = changes.name {
            updated.name = name.clone();
        }
        if let Some(ref artist) = changes.artist {
            updated.artist = Some(artist.clone());
        }
        if let Some(venue_id) = changes.venue_id {
            updated.venue_id = venue_id;
        }
        if let Some((schedule_id, new_starts_at)) = changes.schedule_update {
            let Some(schedule) = updated
                .schedules
                .iter_mut()
                .find(|item| item.id == schedule_id)
            else {
                return Err(AppError::UnprocessableEntity(
                    "Target schedule does not belong to this event".to_string(),
                ));
            };
            schedule.starts_at = new_starts_at;
        }

        updated.updated_at = chrono::Utc::now();
        lock.insert(id, updated.clone());
        Ok(Some(updated))
    }
}
