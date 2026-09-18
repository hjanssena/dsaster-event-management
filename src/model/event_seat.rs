use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "event_seats")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub event_id: Uuid,
    pub venue_seat_id: Uuid,
    pub pricing_tier_id: Uuid,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::event::Entity",
        from = "Column::EventId",
        to = "super::event::Column::Id"
    )]
    Event,
    #[sea_orm(
        belongs_to = "super::event_pricing_tier::Entity",
        from = "Column::PricingTierId",
        to = "super::event_pricing_tier::Column::Id"
    )]
    EventPricingTier,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl Related<super::event_pricing_tier::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventPricingTier.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

