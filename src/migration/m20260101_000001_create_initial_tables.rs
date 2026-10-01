use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Tabla partners
        manager
            .create_table(
                Table::create()
                    .table(Partners::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Partners::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Partners::Username).string_len(255).not_null().unique_key())
                    .col(ColumnDef::new(Partners::Role).string_len(20).not_null())
                    .col(ColumnDef::new(Partners::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Partners::UpdatedAt).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // 2. Tabla events
        manager
            .create_table(
                Table::create()
                    .table(Events::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Events::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Events::OrganizerId).uuid().not_null())
                    .col(ColumnDef::new(Events::VenueId).uuid().not_null())
                    .col(ColumnDef::new(Events::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Events::Description).text().null())
                    .col(ColumnDef::new(Events::EventType).string_len(50).not_null())
                    .col(ColumnDef::new(Events::AgePolicy).string_len(50).not_null())
                    .col(ColumnDef::new(Events::Status).string_len(50).not_null())
                    .col(ColumnDef::new(Events::Terms).text().null())
                    .col(ColumnDef::new(Events::Artist).string_len(255).null())
                    .col(ColumnDef::new(Events::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Events::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_events_organizer_id")
                            .from(Events::Table, Events::OrganizerId)
                            .to(Partners::Table, Partners::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Tabla event_schedules
        manager
            .create_table(
                Table::create()
                    .table(EventSchedules::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EventSchedules::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(EventSchedules::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventSchedules::StartsAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventSchedules::EndsAt).timestamp_with_time_zone().null())
                    .col(ColumnDef::new(EventSchedules::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventSchedules::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_schedules_event_id")
                            .from(EventSchedules::Table, EventSchedules::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 4. Tabla event_pricing_tiers
        manager
            .create_table(
                Table::create()
                    .table(EventPricingTiers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EventPricingTiers::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(EventPricingTiers::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventPricingTiers::Name).string_len(100).not_null())
                    .col(ColumnDef::new(EventPricingTiers::Description).text().null())
                    .col(ColumnDef::new(EventPricingTiers::Price).decimal_len(12, 2).not_null())
                    .col(ColumnDef::new(EventPricingTiers::Currency).string_len(10).not_null())
                    .col(ColumnDef::new(EventPricingTiers::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventPricingTiers::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_pricing_tiers_event_id")
                            .from(EventPricingTiers::Table, EventPricingTiers::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 5. Tabla event_sales
        manager
            .create_table(
                Table::create()
                    .table(EventSales::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EventSales::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(EventSales::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventSales::StartsAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventSales::EndsAt).timestamp_with_time_zone().null())
                    .col(ColumnDef::new(EventSales::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventSales::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_sales_event_id")
                            .from(EventSales::Table, EventSales::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 6. Tabla event_media
        manager
            .create_table(
                Table::create()
                    .table(EventMedia::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EventMedia::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(EventMedia::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventMedia::MediaType).string_len(50).not_null())
                    .col(ColumnDef::new(EventMedia::Url).string_len(1024).not_null())
                    .col(ColumnDef::new(EventMedia::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventMedia::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_media_event_id")
                            .from(EventMedia::Table, EventMedia::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 7. Tabla event_seats
        manager
            .create_table(
                Table::create()
                    .table(EventSeats::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EventSeats::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(EventSeats::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventSeats::VenueSeatId).uuid().not_null())
                    .col(ColumnDef::new(EventSeats::PricingTierId).uuid().not_null())
                    .col(ColumnDef::new(EventSeats::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(EventSeats::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_seats_event_id")
                            .from(EventSeats::Table, EventSeats::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_seats_pricing_tier_id")
                            .from(EventSeats::Table, EventSeats::PricingTierId)
                            .to(EventPricingTiers::Table, EventPricingTiers::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop en orden inverso para respetar las claves foráneas
        manager.drop_table(Table::drop().table(EventSeats::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(EventMedia::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(EventSales::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(EventPricingTiers::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(EventSchedules::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Events::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Partners::Table).if_exists().to_owned()).await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Partners {
    Table,
    Id,
    Username,
    Role,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Events {
    Table,
    Id,
    OrganizerId,
    VenueId,
    Name,
    Description,
    EventType,
    AgePolicy,
    Status,
    Terms,
    Artist,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum EventSchedules {
    Table,
    Id,
    EventId,
    StartsAt,
    EndsAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum EventPricingTiers {
    Table,
    Id,
    EventId,
    Name,
    Description,
    Price,
    Currency,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum EventSales {
    Table,
    Id,
    EventId,
    StartsAt,
    EndsAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum EventMedia {
    Table,
    Id,
    EventId,
    MediaType,
    Url,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum EventSeats {
    Table,
    Id,
    EventId,
    VenueSeatId,
    PricingTierId,
    CreatedAt,
    UpdatedAt,
}
