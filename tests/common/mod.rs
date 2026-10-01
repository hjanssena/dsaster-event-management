use eventManagement_api::migration::{Migrator, MigratorTrait};
use sea_orm::{DatabaseConnection, DbErr};

/// Ejecuta las migraciones de SeaORM sobre la base de datos indicada para pruebas de integración
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    Migrator::up(db, None).await
}

