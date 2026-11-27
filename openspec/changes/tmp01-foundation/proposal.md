# TMP01: Foundation for the target design

> `TMP01` is a temporary ticket ID while the project board is down. Replace it with the real ID in the branch, commits and PR title once the board is back.

## Why

Every later phase (auth, schema, endpoints, propagation) depends on cross-cutting behavior that ADR-006, ADR-008, ADR-010 and ADR-011 define and that the code does not have yet:

- Only 5 of the documented environment variables are read. Pagination (`10`/`100`) and the Venue Service timeout (3 s) are hardcoded.
- `AppError` has no `409` or `503`. Database timeouts become `500`.
- Axum's extractors answer plain-text `400`, `415` or `422`.
- Incoming requests have no timeout.
- Tests cannot run against PostgreSQL.

Building this first means each later phase only adds its own behavior.

## What Changes

- **Configuration (ADR-006, ADR-008):** `AppConfig` reads every documented variable, with its default. Startup is refused, listing every bad variable, when a value is not a positive integer or `EVENTS_PAGE_SIZE_DEFAULT` is greater than `EVENTS_PAGE_SIZE_MAX`. The variables for later phases (Kafka, outbox, publish job, idempotency, retries, breaker) are read and validated now, so their names and defaults are fixed in one place. `JWT_SECRET` is left to TMP02.
- **Errors:** `AppError` gains `Conflict` (`409`) and `ServiceUnavailable` (`503`, with an optional `Retry-After`).
  - These database errors become `503`: pool acquire timeout, statement timeout, and connection failures.
  - Other database errors stay `500` with a generic message.
- **Request parsing (ADR-010):** handlers use new `ApiJson`, `ApiPath` and `ApiQuery` extractors, so every rejection is a JSON `400`. A `RequestTime` type and a service-level parser return `422` for a time without an offset and `400` for a value that is not a time.
  - **BREAKING (current API):** the `date` field of the current `POST /api/v1/events` becomes a `RequestTime`. A date without an offset now gets `422` instead of Axum's plain-text `422`. A missing `Content-Type` gets `400` instead of `415`. Wrong types and missing fields get `400` instead of `422`.
- **Request timeout (ADR-008):** an Axum middleware cuts each request at `HTTP_REQUEST_TIMEOUT_MS` with `503`, `Retry-After` and a JSON body. It adds `Retry-After: HTTP_RETRY_AFTER_SECONDS` to any `503` that has none, and stores the request deadline in a task-local value for the Venue Service retries of TMP06.
- **Database sessions (ADR-003, ADR-008):** the connection pool waits at most `DB_POOL_ACQUIRE_TIMEOUT_MS` for a connection. Every connection sets `TimeZone = 'UTC'` and `statement_timeout = DB_STATEMENT_TIMEOUT_MS`.
- **Config instead of constants:** `EventService` pagination uses `EVENTS_PAGE_SIZE_DEFAULT`/`EVENTS_PAGE_SIZE_MAX`, and `HttpVenueClient` uses `VENUE_SERVICE_TIMEOUT_MS`.
- **One router:** `main.rs` and the tests build the router with the same `build_router`, so the tests exercise the real middleware.
- **Test harness (ADR-011):** `tests/common` gets a real-PostgreSQL harness. It migrates a template database once and gives each test its own copy, dropped at the end. The `HS256` test-token helper is left to TMP02, where the verifier exists.
- **CI:** `.github/workflows/validation.yml` gets `TEST_DATABASE_URL` for the existing Postgres service container. This ticket explicitly asks for that one edit to the protected workflow.

## Capabilities

### New Capabilities

- `service-configuration`: how Event Service reads, defaults and validates its environment variables, and refuses to start on bad values.
- `http-errors`: error status codes and the JSON error body, including request parsing rejections, request time parsing, `409` and `503` with `Retry-After`.
- `request-timeout`: the incoming request time limit, its `503` response, the default `Retry-After`, and the request deadline.
- `database-sessions`: connection pool limits and per-connection session settings, and how database unavailability reaches the client.
- `test-infrastructure`: tests running against real PostgreSQL with one isolated database per test.

### Modified Capabilities

None. `openspec/specs/` contains no capability specs yet.

## Impact

- **Code:**
  - Changed: `src/config.rs`, `src/core/error.rs`, `src/api/event_api.rs`, `src/api/mod.rs`, `src/main.rs`, `src/lib.rs`, `src/service/event_srv.rs`, `src/service/venue_client.rs`, `src/repository/event_repo.rs` (its duplicate clamping is removed), `src/model/dtos.rs`.
  - New: `src/api/extract.rs`, `src/api/middleware.rs`, `src/service/request_time.rs`, `src/db.rs`.
- **Tests:** `tests/common/mod.rs` (harness), new `tests/foundation_test.rs`. `tests/event_api_test.rs` uses `build_router`. The existing mock-based tests keep working; they move to the database when their area is rewritten (ADR-011).
- **API contract:** `openapi.yml` is regenerated, because `date` becomes a string in the schema. No endpoint is added or removed.
- **Dependencies:** no new crate. Axum's `macros` feature is enabled for the extractor derives (ADR-010). SeaORM 1.1's `ConnectOptions::map_sqlx_postgres_opts` sets the session options.
- **Environment:** `.env.example` and the `AGENTS.md` table already list every variable. Their status moves from Planned to Implemented. Running `cargo test` now needs `event-db` (`docker compose up -d event-db`) and `TEST_DATABASE_URL`.
- **CI:** one `env` line in `.github/workflows/validation.yml`.
