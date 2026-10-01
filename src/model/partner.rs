use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Rol único de un partner (PA-04.3). El staff no es un rol: usa una credencial de configuración.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "snake_case")]
pub enum PartnerRole {
    #[sea_orm(string_value = "venue_owner")]
    VenueOwner,
    #[sea_orm(string_value = "organizer")]
    Organizer,
}

/// Partner (venue owner u organizador) conocido por este servicio.
/// Las cuentas, invitaciones y contraseñas viven en el servicio de autenticación;
/// aquí solo se guarda lo necesario para validar existencia y rol.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "partners")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid, // mismo id que emite el servicio de auth (JWT `sub`)
    #[sea_orm(unique)]
    pub username: String,
    pub role: PartnerRole,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::event::Entity")]
    Event,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
