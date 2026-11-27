# Tasks

Each task is one commit, in the message given. Each task's tests must pass before its commit. The OpenSpec change folder is committed with task 1.1.

## 1. Configuration

- [x] 1.1 Implement `AppConfig::from_lookup` with typed, grouped fields for every ADR-006/ADR-008 variable except `JWT_SECRET`. It collects all invalid values and checks `EVENTS_PAGE_SIZE_DEFAULT <= EVENTS_PAGE_SIZE_MAX`. `from_env()` exits with the full error message (design §1).
  - Update `main.rs` to the new fields.
  - In `AGENTS.md`, mark the variables that are now read as Implemented.
  - Verify with unit tests built from explicit maps, covering every `service-configuration` scenario of the requirements "Documented variables with defaults", "Numeric values must be positive integers" and "Default page size within the maximum".
  - Commit `feat(TMP01): read and validate every configured variable`.

## 2. Errors and request parsing

- [x] 2.1 Add `AppError::Conflict` and `AppError::ServiceUnavailable { message, retry_after }`, and classify `DbErr` in `into_response` (design §2).
  - Verify with tests that a conflict gives `409`, a known wait gives `503` with `Retry-After: 12`, and a `503` without a wait sets no header (the middleware adds it in 5.1).
  - Also test that `ConnectionAcquire(Timeout)`, `Conn` and a SQLSTATE `57014` error give `503`, while a generic `DbErr` gives `500` with `Internal server error`.
  - Commit `feat(TMP01): add conflict and service unavailable errors`.
- [x] 2.2 Enable Axum's `macros` feature, add `ApiJson`/`ApiPath`/`ApiQuery` in `src/api/extract.rs` with the `From<*Rejection> for AppError` impls, and switch every handler in `src/api/event_api.rs` to them (design §3).
  - Add an `AGENTS.md` layout line for `extract.rs`.
  - Verify with API tests for each "Unparseable requests get a JSON 400" scenario: invalid JSON, wrong type, missing field, missing `Content-Type`, malformed path UUID and `page=abc`. Each must give `400` with a JSON `error` body.
  - Extend `test_http_get_event_by_id_400_bad_uuid` to assert the JSON body.
  - Commit `feat(TMP01): return a JSON 400 for unparseable requests`.
- [ ] 2.3 Add `RequestTime` and `service::request_time::parse_request_time` (design §4). Switch `CreateEventRequestDto.date` to `RequestTime`, parsed first in `EventService::create_event`.
  - Update its utoipa schema (string, `date-time`) and regenerate `openapi.yml` with `cargo run --bin generate_openapi`.
  - Verify with service tests for the four "Request times need an offset" scenarios: offset converted to UTC, no offset gives `422`, `tomorrow` gives `400`, a number gives `400`. Add an API test that a `date` without an offset gives `422` with a JSON body.
  - Check that `git diff openapi.yml` changes only the `date` schema.
  - Commit `feat(TMP01): parse request times in the service`.

## 3. Test harness

- [ ] 3.1 Add `TestDb` in `tests/common/db.rs` (design §7): template built once under an advisory lock, a copy per test with a retry on `55006`, and `Drop` cleanup with `WITH (FORCE)`. It panics with instructions when `TEST_DATABASE_URL` is missing.
  - Add `TEST_DATABASE_URL` to the `test` job in `.github/workflows/validation.yml`, the one workflow edit this ticket allows.
  - Update the `AGENTS.md` Testing section with how to run the tests.
  - Verify with harness tests for every `test-infrastructure` scenario:
    - migrations applied (the `seaql_migrations` row count matches `Migrator::migrations()`)
    - two concurrent tests each insert a row and see only their own
    - the database is gone after `TestDb` is dropped, including from a test that panics inside `catch_unwind`
    - missing variable panics with the message
  - Commit `test(TMP01): run tests against per-test PostgreSQL databases`.

## 4. Database sessions

- [ ] 4.1 Add `db::connect` in `src/db.rs` with the acquire timeout and the `TimeZone`/`statement_timeout` startup options (design §6), and use it in `main.rs` and in `TestDb`.
  - Verify with `TestDb` tests for every `database-sessions` scenario:
    - `SHOW TimeZone` is `UTC` after `ALTER DATABASE ... SET timezone = 'America/Mexico_City'`
    - `SHOW statement_timeout` matches the config
    - `pg_sleep` beyond the limit gives `503` with `Retry-After`
    - holding the only connection of a size-1 pool past the acquire timeout gives `503` with `Retry-After`
    - a refused connection (unused local port) gives `503` with `Retry-After`
  - Commit `feat(TMP01): limit database waits and run sessions in UTC`.

## 5. Request timeout

- [ ] 5.1 Add `request_guard`, `apply` and `request_deadline` in `src/api/middleware.rs`, and `api::build_router` (design §5). Use `build_router` in `main.rs` and in `create_test_app`.
  - Add `AGENTS.md` layout lines for `middleware.rs` and `db.rs`, and move `HTTP_REQUEST_TIMEOUT_MS`/`HTTP_RETRY_AFTER_SECONDS` to Implemented.
  - Verify with tests for every `request-timeout` scenario, using a test router wrapped by `apply`:
    - a slow handler gives `503` with the JSON body and `Retry-After` set to the configured value
    - a fast handler's response is unchanged
    - a handler that inserts in a transaction and then sleeps past the timeout leaves no row (`TestDb`)
    - a handler returning `503` without `Retry-After` gets the default; one with `Retry-After: 12` keeps it
    - `request_deadline()` is about arrival plus the timeout inside a request, and `None` outside one
  - Also verify the http-errors "Unknown wait" scenario end to end.
  - Commit `feat(TMP01): cut requests at the request timeout`.

## 6. Configuration in use

- [ ] 6.1 Pass `PaginationConfig` to `EventService::new` and remove the hardcoded `10`/`100` from the service and both repositories. Pass `VENUE_SERVICE_TIMEOUT_MS` to `HttpVenueClient::new`. Update the existing tests to the new constructors.
  - Verify with tests for the `service-configuration` requirements "Pagination uses the configured page sizes" (default 7, max 50 clamps 500, page 0 becomes 1) and "Venue Service calls use the configured timeout". For the timeout, use a local TCP listener that never answers: the call returns an error within about the configured timeout.
  - Commit `refactor(TMP01): use configured page sizes and venue timeout`.

## 7. Integration checks

- [ ] 7.1 Run `cargo fmt --all -- --check`, `cargo lint` and `cargo test` with `event-db` running and `TEST_DATABASE_URL` set. Run `openspec validate tmp01-foundation --strict`. Confirm `openapi.yml` matches `cargo run --bin generate_openapi` output with no diff, and that only the files listed in proposal.md (Impact) changed.
  - Record the results in this change folder and mark the tasks done.
  - Commit `chore(TMP01): record foundation validation`.
