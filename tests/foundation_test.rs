//! Tests for the TMP01 foundation: test harness, database sessions and request timeout.

mod common;

use std::sync::mpsc;

use sea_orm::{ConnectionTrait, DbBackend, Statement};

use common::db::{TestDb, admin, admin_url_from, database_exists, url_for_database};
use eventManagement_api::migration::{Migrator, MigratorTrait};

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
