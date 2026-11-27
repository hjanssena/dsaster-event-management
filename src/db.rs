//! Database connections (ADR-002, ADR-003, ADR-008).
//!
//! Every pooled connection, including reconnections, starts with the session
//! settings below as libpq startup parameters, so no statement ever runs
//! without them:
//!
//! - `TimeZone = 'UTC'`: no expression depends on a server's local time zone
//!   or its daylight saving changes (ADR-003). sqlx 0.8 already sends this by
//!   default; it is set here explicitly so the guarantee does not depend on a
//!   driver default.
//! - `statement_timeout = DB_STATEMENT_TIMEOUT_MS`: a slow statement fails
//!   instead of holding the request (ADR-008).
//!
//! Waiting for a pooled connection is limited by `DB_POOL_ACQUIRE_TIMEOUT_MS`.
//! Both timeouts surface as `503` (see `core::error`).

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

use crate::config::DbConfig;

/// Connection options with the pool limit and session settings applied.
/// Callers may adjust pool size or logging before connecting.
pub fn connect_options(url: &str, db: &DbConfig) -> ConnectOptions {
    let statement_timeout_ms = db.statement_timeout.as_millis().to_string();
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .acquire_timeout(db.pool_acquire_timeout)
        .map_sqlx_postgres_opts(move |pg| {
            pg.options([
                ("TimeZone", "UTC".to_string()),
                ("statement_timeout", statement_timeout_ms.clone()),
            ])
        });
    options
}

/// Opens the connection pool with the configured limits and session settings.
pub async fn connect(url: &str, db: &DbConfig) -> Result<DatabaseConnection, DbErr> {
    Database::connect(connect_options(url, db)).await
}
