use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub venue_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub age_policy: String,
    pub status: String,
    pub terms: Option<String>,
    pub artist: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::event_schedule::Entity")]
    EventSchedule,
    #[sea_orm(has_many = "super::event_sale::Entity")]
    EventSale,
    #[sea_orm(has_many = "super::event_pricing_tier::Entity")]
    EventPricingTier,
    #[sea_orm(has_many = "super::event_media::Entity")]
    EventMedia,
    #[sea_orm(has_many = "super::event_seat::Entity")]
    EventSeat,
}

impl Related<super::event_schedule::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventSchedule.def()
    }
}

impl Related<super::event_sale::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventSale.def()
    }
}

impl Related<super::event_pricing_tier::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventPricingTier.def()
    }
}

impl Related<super::event_media::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventMedia.def()
    }
}

impl Related<super::event_seat::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventSeat.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
