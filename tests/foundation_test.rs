//! Tests for the TMP01 foundation: test harness, database sessions and request timeout.

mod common;

use std::sync::mpsc;
use std::time::Duration;

use axum::{http::StatusCode, response::IntoResponse};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};

use common::db::{TestDb, admin, admin_url_from, database_exists, url_for_database};
use eventManagement_api::{
    config::DbConfig,
    core::error::AppError,
    db,
    migration::{Migrator, MigratorTrait},
};

async fn count(db: &TestDb, sql: &str) -> i64 {
    db.conn
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap()
}

// ==================== Test infrastructure (ADR-011) ====================

#[tokio::test]
async fn test_db_has_every_migration_applied() {
    let db = TestDb::new().await;
    let applied = count(&db, "SELECT COUNT(*) FROM seaql_migrations").await;
    assert_eq!(applied, Migrator::migrations().len() as i64);
}

#[tokio::test]
async fn test_dbs_running_at_the_same_time_are_isolated() {
    async fn write_one_row() -> (String, i64) {
        let db = TestDb::new().await;
        db.conn
            .execute_unprepared("CREATE TABLE probe (id int); INSERT INTO probe VALUES (1);")
            .await
            .unwrap();
        (
            db.name().to_string(),
            count(&db, "SELECT COUNT(*) FROM probe").await,
        )
    }

    let ((name_a, rows_a), (name_b, rows_b)) = tokio::join!(write_one_row(), write_one_row());
    assert_ne!(name_a, name_b);
    assert_eq!(rows_a, 1);
    assert_eq!(rows_b, 1);
}

#[tokio::test]
async fn test_db_is_dropped_after_the_test() {
    let db = TestDb::new().await;
    let name = db.name().to_string();
    let admin = admin().await;
    assert!(database_exists(&admin, &name).await);

    drop(db);
    assert!(!database_exists(&admin, &name).await);
}

#[tokio::test]
async fn test_db_is_dropped_when_the_test_panics() {
    let (sender, receiver) = mpsc::channel();
    let failing_test = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let db = TestDb::new().await;
                sender.send(db.name().to_string()).unwrap();
                panic!("simulated test failure");
            })
    });

    assert!(
        failing_test.join().is_err(),
        "the simulated test should panic"
    );
    let name = receiver.recv().unwrap();
    assert!(!database_exists(&admin().await, &name).await);
}

#[test]
fn missing_test_database_url_explains_how_to_start_the_database() {
    let message = admin_url_from(|_| None).unwrap_err();
    assert!(message.contains("TEST_DATABASE_URL"), "{message}");
    assert!(
        message.contains("docker compose up -d event-db"),
        "{message}"
    );
}

#[test]
fn database_name_is_replaced_in_the_url() {
    assert_eq!(
        url_for_database("postgres://u:p@localhost:5432/postgres", "x"),
        "postgres://u:p@localhost:5432/x"
    );
    assert_eq!(
        url_for_database(
            "postgres://u:p@localhost:5432/postgres?sslmode=disable",
            "x"
        ),
        "postgres://u:p@localhost:5432/x?sslmode=disable"
    );
    assert_eq!(
        url_for_database("postgres://u:p@localhost:5432", "x"),
        "postgres://u:p@localhost:5432/x"
    );
}

// ==================== Database sessions ====================

fn db_config(acquire_ms: u64, statement_ms: u64) -> DbConfig {
    DbConfig {
        pool_acquire_timeout: Duration::from_millis(acquire_ms),
        statement_timeout: Duration::from_millis(statement_ms),
    }
}

async fn show(conn: &DatabaseConnection, setting: &str) -> String {
    conn.query_one(Statement::from_string(
        DbBackend::Postgres,
        format!("SHOW {setting}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<String>(0)
    .unwrap()
}

fn status_of(err: sea_orm::DbErr) -> StatusCode {
    AppError::DatabaseError(err).into_response().status()
}

#[tokio::test]
async fn sessions_run_in_utc_whatever_the_server_default() {
    let test_db = TestDb::new().await;
    admin()
        .await
        .execute_unprepared(&format!(
            "ALTER DATABASE \"{}\" SET timezone = 'America/Mexico_City'",
            test_db.name()
        ))
        .await
        .unwrap();

    // The database's own default is now not UTC...
    let defaults = admin()
        .await
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT array_to_string(setconfig, ',') FROM pg_db_role_setting \
             WHERE setdatabase = (SELECT oid FROM pg_database WHERE datname = $1)",
            [test_db.name().into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<String>(0)
        .unwrap();
    assert!(defaults.contains("America/Mexico_City"), "{defaults}");

    // ...but the service's connections always run in UTC.
    let conn = db::connect(test_db.url(), &db_config(3_000, 5_000))
        .await
        .unwrap();
    assert_eq!(show(&conn, "TimeZone").await, "UTC");
    conn.close().await.unwrap();
}

#[tokio::test]
async fn sessions_use_the_configured_statement_timeout() {
    let test_db = TestDb::new().await;
    let conn = db::connect(test_db.url(), &db_config(3_000, 1_234))
        .await
        .unwrap();
    assert_eq!(show(&conn, "statement_timeout").await, "1234ms");
    conn.close().await.unwrap();
}

#[tokio::test]
async fn statement_slower_than_the_timeout_is_503() {
    let test_db = TestDb::new().await;
    let conn = db::connect(test_db.url(), &db_config(3_000, 200))
        .await
        .unwrap();

    let err = conn
        .execute_unprepared("SELECT pg_sleep(2)")
        .await
        .expect_err("the statement should be cancelled");
    assert_eq!(status_of(err), StatusCode::SERVICE_UNAVAILABLE);
    conn.close().await.unwrap();
}

#[tokio::test]
async fn exhausted_pool_is_503_after_the_acquire_timeout() {
    let test_db = TestDb::new().await;
    let mut options = db::connect_options(test_db.url(), &db_config(200, 5_000));
    options.max_connections(1);
    let conn = Database::connect(options).await.unwrap();

    // The open transaction holds the only connection.
    let tx = conn.begin().await.unwrap();
    let started = std::time::Instant::now();
    let err = conn
        .execute_unprepared("SELECT 1")
        .await
        .expect_err("no connection should be available");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(status_of(err), StatusCode::SERVICE_UNAVAILABLE);

    tx.rollback().await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn unreachable_database_is_503() {
    // A port that was just free: nothing listens on it, so the connection is refused.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("postgres://usuario:password@127.0.0.1:{port}/event_store");

    let err = db::connect(&url, &db_config(1_000, 5_000))
        .await
        .expect_err("nothing listens on that port");
    assert_eq!(status_of(err), StatusCode::SERVICE_UNAVAILABLE);
}
