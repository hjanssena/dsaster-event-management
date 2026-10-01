use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::model::partner::{self, PartnerRole};

/// Partners fijos para desarrollo local con autenticación simulada (MOCK_AUTH)
pub const DEV_ORGANIZER_ID: Uuid = Uuid::from_u128(0x11111111_1111_1111_1111_111111111111);
pub const DEV_VENUE_OWNER_ID: Uuid = Uuid::from_u128(0x22222222_2222_2222_2222_222222222222);

/// Contrato para el acceso a datos de partners
#[async_trait]
pub trait PartnerRepository: Send + Sync {
    /// Busca un partner por su ID (el `sub` del token)
    async fn find_by_id(&self, id: Uuid) -> Result<Option<partner::Model>, AppError>;
}

/// Implementación de producción respaldada por SeaORM
pub struct SeaOrmPartnerRepository {
    db: DatabaseConnection,
}

impl SeaOrmPartnerRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PartnerRepository for SeaOrmPartnerRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<partner::Model>, AppError> {
        Ok(partner::Entity::find_by_id(id).one(&self.db).await?)
    }
}

/// Implementación en memoria para pruebas y para el modo de autenticación simulada
#[derive(Clone, Default)]
pub struct MockPartnerRepository {
    storage: Arc<RwLock<HashMap<Uuid, partner::Model>>>,
}

impl MockPartnerRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_partners(partners: Vec<partner::Model>) -> Self {
        let storage = partners.into_iter().map(|p| (p.id, p)).collect();
        Self {
            storage: Arc::new(RwLock::new(storage)),
        }
    }

    /// Mock precargado con un organizador y un venue owner conocidos
    pub fn with_dev_partners() -> Self {
        let now = chrono::Utc::now();
        Self::with_partners(vec![
            partner::Model {
                id: DEV_ORGANIZER_ID,
                username: "dev_organizer".to_string(),
                role: PartnerRole::Organizer,
                created_at: now,
                updated_at: now,
            },
            partner::Model {
                id: DEV_VENUE_OWNER_ID,
                username: "dev_venue_owner".to_string(),
                role: PartnerRole::VenueOwner,
                created_at: now,
                updated_at: now,
            },
        ])
    }

    pub async fn insert(&self, partner: partner::Model) {
        let mut lock = self.storage.write().await;
        lock.insert(partner.id, partner);
    }
}

#[async_trait]
impl PartnerRepository for MockPartnerRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<partner::Model>, AppError> {
        let lock = self.storage.read().await;
        Ok(lock.get(&id).cloned())
    }
}
