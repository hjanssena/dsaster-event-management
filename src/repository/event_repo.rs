use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::model::{
    dtos::{
        EventMediaDto, EventPaginationQueryDto, EventPricingTierDto, EventResponseDto,
        EventSaleDto, EventScheduleDto, EventSummaryDto, PaginatedEventSummaryResponse,
        PaginatedResponseDto,
    },
    event, event_media, event_pricing_tier, event_sale, event_schedule,
};

/// Contrato para el acceso a datos de eventos
#[async_trait]
pub trait EventRepository: Send + Sync {
    /// Busca un evento por su ID con todas sus relaciones cargadas
    async fn find_by_id(&self, id: Uuid) -> Result<Option<EventResponseDto>, AppError>;

    /// Busca eventos de forma paginada aplicando filtros opcionales
    async fn find_paginated(
        &self,
        query: &EventPaginationQueryDto,
    ) -> Result<PaginatedEventSummaryResponse, AppError>;
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

        // Cargar colecciones anidadas del agregado
        let schedules = event_model
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

        let pricing_tiers = event_model
            .find_related(event_pricing_tier::Entity)
            .all(&self.db)
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
            .all(&self.db)
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
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| EventMediaDto {
                id: m.id,
                media_type: m.media_type,
                url: m.url,
            })
            .collect();

        Ok(Some(EventResponseDto {
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
        }))
    }

    async fn find_paginated(
        &self,
        query: &EventPaginationQueryDto,
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

        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

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
}

/// Implementación en memoria para pruebas unitarias sin dependencias externas
#[derive(Clone, Default)]
pub struct MockEventRepository {
    storage: Arc<RwLock<HashMap<Uuid, EventResponseDto>>>,
}

impl MockEventRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, event: EventResponseDto) {
        let mut lock = self.storage.write().await;
        lock.insert(event.id, event);
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
    ) -> Result<PaginatedEventSummaryResponse, AppError> {
        let lock = self.storage.read().await;
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

        let mut filtered: Vec<EventResponseDto> = lock
            .values()
            .filter(|ev| {
                if query.id.is_some_and(|id| ev.id != id) {
                    return false;
                }
                if query.status.as_ref().is_some_and(|status| &ev.status != status) {
                    return false;
                }
                if query.venue_id.is_some_and(|venue_id| ev.venue_id != venue_id) {
                    return false;
                }
                if query.organizer_id.is_some_and(|organizer_id| ev.organizer_id != organizer_id) {
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
}
