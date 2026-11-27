//! Tests for the TMP01 foundation: test harness, database sessions and request timeout.

mod common;

use std::sync::mpsc;
use std::time::Duration;

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use tower::ServiceExt;

use common::db::{TestDb, admin, admin_url_from, database_exists, url_for_database};
use eventManagement_api::{
    api::middleware::{apply, request_deadline},
    config::{DbConfig, HttpConfig},
    core::error::AppError,
    db,
    migration::{Migrator, MigratorTrait},
    service::{HttpVenueClient, VenueClient},
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

// ==================== Request timeout (ADR-008) ====================

const TIMEOUT_MS: u64 = 150;
const DEFAULT_RETRY_AFTER: u64 = 7;

fn guard_config() -> HttpConfig {
    HttpConfig {
        request_timeout: Duration::from_millis(TIMEOUT_MS),
        retry_after_seconds: DEFAULT_RETRY_AFTER,
    }
}

async fn call(router: Router, uri: &str) -> Response {
    router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn retry_after(response: &Response) -> Option<String> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_string())
}

async fn json_body(response: Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn guarded(router: Router) -> Router {
    apply(router, &guard_config())
}

#[tokio::test]
async fn slow_request_is_cut_with_503_json_and_retry_after() {
    let router = guarded(Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            "too late"
        }),
    ));

    let started = std::time::Instant::now();
    let response = call(router, "/slow").await;
    let elapsed = started.elapsed();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        retry_after(&response).as_deref(),
        Some(DEFAULT_RETRY_AFTER.to_string().as_str())
    );
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(json_body(response).await["error"].is_string());
    assert!(
        elapsed < Duration::from_millis(TIMEOUT_MS + 500),
        "answered after {elapsed:?}"
    );
}

#[tokio::test]
async fn fast_request_is_unchanged() {
    let router = guarded(Router::new().route("/fast", get(|| async { "ok" })));
    let response = call(router, "/fast").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(retry_after(&response), None);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn timed_out_request_saves_nothing_from_its_transaction() {
    let test_db = TestDb::new().await;
    test_db
        .conn
        .execute_unprepared("CREATE TABLE probe (id int)")
        .await
        .unwrap();

    async fn insert_then_stall(State(conn): State<DatabaseConnection>) -> &'static str {
        let tx = conn.begin().await.unwrap();
        tx.execute_unprepared("INSERT INTO probe VALUES (1)")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
        tx.commit().await.unwrap();
        "committed"
    }

    let router = guarded(
        Router::new()
            .route("/write", get(insert_then_stall))
            .with_state(test_db.conn.clone()),
    );
    let response = call(router, "/write").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    // Wait past the point where the handler would have committed.
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_eq!(count(&test_db, "SELECT COUNT(*) FROM probe").await, 0);
}

#[tokio::test]
async fn service_unavailable_without_retry_after_gets_the_default() {
    let router = guarded(Router::new().route(
        "/down",
        get(|| async { AppError::service_unavailable("dependency down") }),
    ));
    let response = call(router, "/down").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        retry_after(&response).as_deref(),
        Some(DEFAULT_RETRY_AFTER.to_string().as_str())
    );
}

#[tokio::test]
async fn service_unavailable_keeps_its_own_retry_after() {
    let router = guarded(Router::new().route(
        "/breaker",
        get(|| async {
            AppError::ServiceUnavailable {
                message: "breaker open".into(),
                retry_after: Some(12),
            }
        }),
    ));
    let response = call(router, "/breaker").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(retry_after(&response).as_deref(), Some("12"));
}

#[tokio::test]
async fn request_deadline_is_available_inside_a_request_only() {
    assert_eq!(request_deadline(), None);

    let router = guarded(Router::new().route(
        "/deadline",
        get(|| async {
            let deadline = request_deadline().expect("deadline inside a request");
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            left.as_millis().to_string()
        }),
    ));
    let response = call(router, "/deadline").await;
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let left_ms: u64 = std::str::from_utf8(&body).unwrap().parse().unwrap();

    assert!(left_ms > 0 && left_ms <= TIMEOUT_MS, "{left_ms} ms left");
}

#[tokio::test]
async fn database_unavailable_reaches_the_client_as_503_with_default_retry_after() {
    // http-errors "Unknown wait" and database-sessions "Statement too slow", end to end.
    let test_db = TestDb::new().await;
    let conn = db::connect(test_db.url(), &db_config(3_000, 100))
        .await
        .unwrap();

    async fn slow_query(State(conn): State<DatabaseConnection>) -> Result<&'static str, AppError> {
        conn.execute_unprepared("SELECT pg_sleep(2)").await?;
        Ok("done")
    }

    let router = apply(
        Router::new()
            .route("/query", get(slow_query))
            .with_state(conn.clone()),
        &HttpConfig {
            request_timeout: Duration::from_secs(5),
            retry_after_seconds: DEFAULT_RETRY_AFTER,
        },
    );
    let response = call(router, "/query").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        retry_after(&response).as_deref(),
        Some(DEFAULT_RETRY_AFTER.to_string().as_str())
    );
    assert_eq!(
        json_body(response).await["error"],
        "Service temporarily unavailable"
    );
    conn.close().await.unwrap();
}

// ==================== Configured Venue Service timeout ====================

#[tokio::test]
async fn venue_service_call_stops_at_the_configured_timeout() {
    // A server that accepts connections and never answers.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            held.push(socket);
        }
    });

    let client = HttpVenueClient::new(format!("http://{address}"), Duration::from_millis(200));
    let started = std::time::Instant::now();
    let result = client.verify_venue_exists(uuid::Uuid::new_v4()).await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "a call past the timeout must fail");
    assert!(
        elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(1),
        "stopped after {elapsed:?}"
    );
    server.abort();
}
