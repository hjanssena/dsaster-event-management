use sea_orm_migration::prelude::*;
use uuid::Uuid;

use super::m20260101_000001_create_initial_tables::Partners;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DEV_ORGANIZER_ID: &str = "11111111-1111-1111-1111-111111111111";
const DEV_VENUE_OWNER_ID: &str = "22222222-2222-2222-2222-222222222222";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let now = chrono::Utc::now();
        let organizer_uuid = Uuid::parse_str(DEV_ORGANIZER_ID).unwrap();
        let venue_owner_uuid = Uuid::parse_str(DEV_VENUE_OWNER_ID).unwrap();

        let insert = Query::insert()
            .into_table(Partners::Table)
            .columns([
                Partners::Id,
                Partners::Username,
                Partners::Role,
                Partners::CreatedAt,
                Partners::UpdatedAt,
            ])
            .values_panic([
                organizer_uuid.into(),
                "dev_organizer".into(),
                "organizer".into(),
                now.into(),
                now.into(),
            ])
            .values_panic([
                venue_owner_uuid.into(),
                "dev_venue_owner".into(),
                "venue_owner".into(),
                now.into(),
                now.into(),
            ])
            .to_owned();

        manager.exec_stmt(insert).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let organizer_uuid = Uuid::parse_str(DEV_ORGANIZER_ID).unwrap();
        let venue_owner_uuid = Uuid::parse_str(DEV_VENUE_OWNER_ID).unwrap();

        let delete = Query::delete()
            .from_table(Partners::Table)
            .and_where(
                Expr::col(Partners::Id).is_in([organizer_uuid, venue_owner_uuid]),
            )
            .to_owned();

        manager.exec_stmt(delete).await?;
        Ok(())
    }
}
