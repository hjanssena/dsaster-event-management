# Design: TMP01 foundation

## Context

See proposal.md (Why) for the motivation; the specs give the required behavior. This section covers only what shapes the approach.

- `AppConfig::from_env()` (`src/config.rs`) reads 5 variables and panics on a missing `DATABASE_URL`.
- `main.rs` connects with `Database::connect(url)`, with default pool options.
- `main.rs` builds the router inline, and `tests/event_api_test.rs::create_test_app` rebuilds a copy without the middleware. Middleware added to `main.rs` alone would go untested.
- `AppError::into_response` maps every `DbErr` to `500`.
- Handlers use `axum::Json`, `Path` and `Query` directly.
- `EventService::get_events` and both repositories clamp pagination with `10`/`100`. `HttpVenueClient::new` hardcodes a 3 s timeout.
- Versions in use: axum 0.7.9, SeaORM 1.1.20 on sqlx-postgres 0.8.6, Postgres 15 (compose and CI).
- CI's `test` job already has a Postgres 15 service container (user `postgres`, superuser).

## Goals / Non-Goals

**Goals**
- Each later phase adds only its own behavior, on top of a configuration, error, parsing, timeout, database and test layer that already matches the ADRs.
- The tests exercise the router exactly as `main.rs` builds it.

**Non-Goals**
- JWT verification and the test-token helper (TMP02). `JWT_SECRET` is not read yet.
- Venue Service retries, circuit breaker and the `401`/`403` → `503` mapping (TMP06). The request deadline is made available, but nothing reads it yet.
- Moving the existing mock-based tests to the database. They move when their area is rewritten (ADR-011).
- Any change to endpoints, the schema or migrations.

## Decisions

### 1. `AppConfig::from_lookup` with typed fields and collected errors

`AppConfig::from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<AppConfig, ConfigError>` does all parsing. `from_env()` loads `.env`, calls it with `std::env::var`, and on `Err` exits with the `ConfigError` message.

- **Typed fields.** Each value is parsed into its unit: `Duration` for `_MS`, `_SECONDS` and `_HOURS`, `u32` for counts, sizes and page sizes. The unit lives in the type, so callers cannot mix milliseconds and seconds. `EVENT_VISIBILITY_LEAD_DAYS` stays a `u32` number of days, because the SQL multiplies it by 24 hours (ADR-003).
- **All errors at once.** Every variable is parsed; every failure is pushed to a list, and `ConfigError` names them all. The cross-field check (`EVENTS_PAGE_SIZE_DEFAULT <= EVENTS_PAGE_SIZE_MAX`) runs only when both parsed.
- **Booleans** (`MOCK_AUTH`, `MOCK_VENUE_SERVICE`) keep their current parsing rule ("false" or "0" is false).
- **Grouping.** Fields are grouped into small structs by consumer (`HttpConfig`, `DbConfig`, `PaginationConfig`, `VenueConfig`, `RetryConfig`, `BreakerConfig`, `PublishJobConfig`, `OutboxConfig`, `KafkaConfig`, `IdempotencyConfig`). Each component receives only its group. `EventService::new` takes `PaginationConfig`, and `HttpVenueClient::new` takes the Venue timeout.

*Alternatives:* reading `std::env` directly in tests. Rejected: tests run in parallel threads and share one process environment, so setting variables races. A config crate (`envy`, `config`) was rejected too: it needs a new crate and still lacks collected errors and the cross-field check.

### 2. `AppError` variants, and classifying `DbErr` at response time

- New variants:
  - `Conflict(String)` → `409`.
  - `ServiceUnavailable { message: String, retry_after: Option<u64> }` → `503`. It sets `Retry-After` only when `retry_after` is `Some`.
- `DatabaseError(DbErr)` keeps its `#[from]`. `into_response` classifies the error and logs it in full either way. The **transient** ones become `ServiceUnavailable` with `retry_after: None`, so the middleware adds the default. Everything else stays `500` with the generic message. Transient means:
  - `DbErr::ConnectionAcquire(_)`: pool timeout or connection closed.
  - `DbErr::Conn(_)`: connection failed.
  - `Query`/`Exec` errors whose sqlx database error has SQLSTATE `57014` (`query_canceled`, raised by `statement_timeout`).
- Classifying in `into_response`, not in `From`, keeps the hundreds of `?` call sites unchanged.

*Alternative:* map at each repository call. Rejected: easy to forget, and it duplicates one rule everywhere.

### 3. Extractors derived with Axum's macros (ADR-010)

`src/api/extract.rs` defines:

```rust
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct ApiJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct ApiPath<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct ApiQuery<T>(pub T);
```

`AppError` implements `From<JsonRejection>`, `From<PathRejection>` and `From<QueryRejection>`, each producing `BadRequest(rejection.body_text())`. That text describes the client's mistake ("missing field `name`", "expected a string") and contains no internal detail. `ApiJson<T: Serialize>` also implements `IntoResponse`. This needs Axum's `macros` feature.

### 4. `RequestTime` parsed in the service

`RequestTime` is a `#[serde(transparent)]` newtype over `String` in `src/model/dtos.rs`. In OpenAPI it is a string with `format: date-time`.

`src/service/request_time.rs` exposes `parse_request_time(value: &RequestTime, field: &str) -> Result<DateTime<Utc>, AppError>`:
1. `DateTime::parse_from_rfc3339` succeeds → convert to UTC.
2. Otherwise, a `NaiveDateTime` parse with `%Y-%m-%dT%H:%M:%S%.f` succeeds → `422`, "`<field>` must include a timezone offset".
3. Otherwise → `400`, "`<field>` is not a valid RFC 3339 time".

The current `CreateEventRequestDto.date` switches to `RequestTime`, and `EventService::create_event` parses it first. Its other behavior is untouched; TMP05 replaces that endpoint.

### 5. One router, one guard middleware

- `api::build_router(state: AppState, http: &HttpConfig) -> Router` builds the routes. They are the Swagger UI, `/health`, `/api/v1/events` and the `/events` alias. Then it applies `api::middleware::apply(router, http)`, `TraceLayer` and `CorsLayer`, in that order, so CORS wraps even a timeout response. `main.rs` and `create_test_app` both call it.
- `api::middleware::apply` is public, so tests can wrap a test-only router (a deliberately slow handler) in the same middleware.
- `request_guard` (`axum::middleware::from_fn_with_state`) works like this:
  1. Compute `deadline = Instant::now() + HTTP_REQUEST_TIMEOUT_MS`.
  2. Run `REQUEST_DEADLINE.scope(deadline, next.run(req))` (a `tokio::task_local!`) under `tokio::time::timeout`.
  3. On elapse, return `AppError::ServiceUnavailable { retry_after: None, .. }`. Dropping the inner future drops any open SeaORM transaction, which rolls back.
  4. On any response with status `503` and no `Retry-After`, insert `HTTP_RETRY_AFTER_SECONDS`.
- `api::middleware::request_deadline() -> Option<Instant>` reads the task-local value with `try_with`. It returns `None` outside a request, for background jobs.

*Alternatives:* `tower_http::timeout::TimeoutLayer` answers `408` with an empty body (ADR-008). `tower::timeout` with `HandleErrorLayer` needs a feature and an error adapter, and still cannot set the deadline or add the default header. One `from_fn` does all three.

### 6. Connection options in `src/db.rs`

`db::connect(db: &DbConfig, url: &str) -> Result<DatabaseConnection, DbErr>` builds `ConnectOptions`:
- `.acquire_timeout(DB_POOL_ACQUIRE_TIMEOUT_MS)`.
- `.map_sqlx_postgres_opts(|o| o.options([("TimeZone", "UTC"), ("statement_timeout", "<ms>")]))`. These are libpq startup parameters, so every pooled connection, including reconnections, has them from its first statement, with no `after_connect` hook.

`main.rs` and the test harness both connect through it.

### 7. Test harness: a template database, then one copy per test

The harness lives in `tests/common/db.rs` and is used as `let db = TestDb::new().await;`:
- Reads `TEST_DATABASE_URL` (a server URL whose path names any database, such as `/postgres`) as the admin connection. If it is missing, it panics with the message the spec requires.
- **Template.** Named `event_test_tpl_<last migration name>`.
  - If the template is missing, the harness takes `pg_advisory_lock` on an admin connection, so parallel test binaries don't race.
  - It creates `<template>_building`, runs `Migrator::up` through `db::connect`, closes that pool, and renames the database to the template name. A run that crashes halfway never leaves a template that looks finished.
  - A `tokio::sync::OnceCell` makes each test binary do this check once.
  - When a migration is added, the name changes and a new template is built. Old templates stay until someone drops them; they are small.
- **Per test.** `CREATE DATABASE event_test_<uuid> TEMPLATE <template>`, retried a few times on SQLSTATE `55006` ("source database is being accessed"), then `db::connect` to it with test `DbConfig` values.
- **Cleanup.** `Drop` for `TestDb` runs `DROP DATABASE ... WITH (FORCE)` on a short-lived thread with its own current-thread runtime. Async drop doesn't exist, and `Drop` also runs while a failing test unwinds. `WITH (FORCE)` needs Postgres 13 or later.
- **URL handling.** The URL is changed by replacing its path segment with string operations. The `url` crate is not added for this.

*Alternatives:* see ADR-011 (testcontainers, one shared database with rollback). `sqlx::test` was also rejected: it is tied to sqlx's migrator and pool, not SeaORM's.

### 8. CI

The `test` job of `.github/workflows/validation.yml` gets `TEST_DATABASE_URL: postgres://postgres:password@localhost:5432/postgres` next to its existing `DATABASE_URL`. No other workflow change.

## Risks / Trade-offs

- [Concurrent `CREATE DATABASE ... TEMPLATE` can fail with `55006`] → Bounded retry with a short sleep. Nothing connects to the template after it is built.
- [`Drop` cleanup fails, for example if the server goes away mid-run] → The database is left behind. The prefix `event_test_` makes leftovers easy to find; dropping them is a manual step.
- [The timeout cancels the handler between a successful `COMMIT` and the response] → The change is saved but the client gets `503`. This is inherent to any server-side timeout. Clients retry safely: `POST` with its `Idempotency-Key` (TMP05), and the other edits are idempotent (ADR-008).
- [Classifying `DbErr` too broadly could hide bugs as `503`] → Only connection, acquire and `57014` errors become `503`. Constraint violations and syntax errors stay `500` and are logged in full.
- [`cargo test` now needs Docker] → Accepted in ADR-011. The harness fails with instructions, not a cryptic connection error.
- [Breaking change to the current `POST`'s `date` and error formats] → The current API is replaced by TMP04–TMP07. No consumer depends on it yet (the contract tells consumers to build against the target).

## Migration Plan

No data or schema migration. Deploy as a normal release. New environment variables are optional because every one has a default. A deployment that sets an invalid value now fails at startup instead of running with a hardcoded value. Roll back by deploying the previous image.

## Open Questions

None.
