# Validation: TMP01 foundation

Run on 2026-11-27, on branch `feat_TMP01_foundation`, after rebasing onto `testing` (`535b8a4`, which includes #34, #35 and #36). The checks below are from after the rebase.

## Environment

- PostgreSQL 15 (`postgres:15-alpine`), the same image as `event-db` in `docker-compose.yml`. It ran as a temporary Podman container on port `55433`, because the local `event-db` container could not start: the machine's `containers.conf` selects `slirp4netns`, which Podman 6 no longer supports, and a native Postgres already held port `5432`.
- `TEST_DATABASE_URL=postgres://usuario:password@localhost:55433/postgres`.

## Checks

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `cargo lint` (`clippy --locked --all-targets -- -D warnings`) | Pass, no warnings |
| `cargo test` | Pass: 103 tests (20 lib unit, 34 API, 28 service, 19 foundation, 2 Venue client) |
| Each of the 9 commits during the rebase | `cargo build --tests` passed |
| Upstream tests after the rebase | All 47 test functions from `testing` are still present (19 service, 26 API, 2 Venue client) |
| `cargo test --test foundation_test`, 5 extra runs | Pass every run, 1.4–1.6 s each, no flaky timing |
| `openspec validate tmp01-foundation --strict` | Valid |
| `cargo run --bin generate_openapi` then `git diff openapi.yml` | No diff |
| Leftover test databases after the runs | None; only the template `event_test_tpl_<hash>`, closed to connections |
| Binary with `RETRY_MAX_ATTEMPTS=0`, `EVENTS_PAGE_SIZE_DEFAULT=500`, no `DATABASE_URL` | Refused to start, exit code 1, named all three problems |
| Server against a real database (`/health`, bad UUID, `page=abc`, date without offset, `per_page=500`) | `200`; JSON `400`; JSON `400`; JSON `422`; `per_page` clamped to `100` |

## Scenario coverage

Every scenario in `specs/` maps to at least one test:

- **service-configuration:** `src/config.rs` unit tests (defaults, override, missing `DATABASE_URL`, zero, non-numeric, several invalid, default above maximum). Also `test_get_events_uses_configured_default_page_size`, `test_get_events_clamps_to_configured_maximum`, `test_get_events_page_zero_is_page_one` and `venue_service_call_stops_at_the_configured_timeout`.
- **http-errors:**
  - `src/core/error.rs` unit tests: JSON body, `409`, `503` with and without a wait, database errors.
  - `tests/event_api_test.rs`: invalid JSON, wrong type, missing field, missing `Content-Type`, bad UUID, bad query, date without offset (`422`), date as a number (`400`).
  - `tests/event_service_test.rs`: the four request-time cases.
  - `database_unavailable_reaches_the_client_as_503_with_default_retry_after`: unknown wait, end to end.
- **request-timeout:** `slow_request_is_cut_with_503_json_and_retry_after`, `fast_request_is_unchanged`, `timed_out_request_saves_nothing_from_its_transaction`, `service_unavailable_without_retry_after_gets_the_default`, `service_unavailable_keeps_its_own_retry_after` and `request_deadline_is_available_inside_a_request_only`.
- **database-sessions:** `sessions_run_in_utc_whatever_the_server_default`, `sessions_use_the_configured_statement_timeout`, `statement_slower_than_the_timeout_is_503`, `exhausted_pool_is_503_after_the_acquire_timeout` and `unreachable_database_is_503`. Retry-After end to end is checked by `database_unavailable_reaches_the_client_as_503_with_default_retry_after`.
- **test-infrastructure:** `test_db_has_every_migration_applied`, `test_dbs_running_at_the_same_time_are_isolated`, `test_db_is_dropped_after_the_test`, `test_db_is_dropped_when_the_test_panics` and `missing_test_database_url_explains_how_to_start_the_database`.

## Rebase onto `testing`

While this change was in progress, #34 (`PATCH /api/v1/events/{id}`) and #35 (`VENUE_SERVICE_TOKEN`, `HttpVenueClient::with_token`) merged into `testing`. Integrating them followed this change's own rules:

- **1.1:** `VENUE_SERVICE_TOKEN` moved into `AppConfig` as `venue.token`. It is optional: unset or blank means no token. It has a test and a row in the `AGENTS.md` variable table.
- **2.2:** the new `PATCH` handler takes its path with `ApiPath`. It already parsed its body into JSON `400`s itself.
- **2.3:** `UpdateEventRequestDto.date` became `Option<RequestTime>` and is parsed in `EventService::update_event`. A `PATCH` date without an offset therefore gets `422`, like every request time. Added a service test and an API test for it. Its utoipa description and `openapi.yml` were updated.
- **6.1:** `HttpVenueClient::new` and `with_token` take `VENUE_SERVICE_TIMEOUT_MS`. The #34 tests pass explicit page sizes to `EventService::new`, and `tests/venue_client_test.rs` passes an explicit timeout.

## Notes and deviations

- **sqlx already sends `TimeZone=UTC`** at connection start (`sqlx-postgres` 0.8, `establish.rs`). `db::connect` still sets it explicitly, so ADR-003 doesn't depend on a driver default. Because of this, the UTC test checks the requirement (sessions are in UTC even when the database's default is not), not that the explicit setting exists.
- **Retry-After in task 4.1:** the default `Retry-After` comes from the middleware of task 5.1. The 4.1 tests check that each database failure is `503`; the end-to-end `Retry-After` check was added in 5.1.
- **Transient sqlx errors:** besides the cases in design §2, `PoolTimedOut`, `PoolClosed` and I/O errors inside a query are also classified as `503`. They mean the database could not be reached, which the database-sessions spec treats as unavailable.
- **Query parameter location:** utoipa cannot infer it through `ApiQuery`, so `EventPaginationQueryDto` declares `#[into_params(parameter_in = Query)]`. Without that, `openapi.yml` would have listed the list filters as path parameters.
- **`axum-macros` 0.4.2** appears in `Cargo.lock`. It is Axum's own proc-macro crate, pulled in by the `macros` feature (ADR-010). Cargo updated the lockfile; it was not edited by hand.
- **AGENTS.md variable table:** variables that are parsed but not used yet are marked "Read, not used yet" rather than "Implemented". Implemented: `DB_POOL_ACQUIRE_TIMEOUT_MS`, `DB_STATEMENT_TIMEOUT_MS`, `HTTP_REQUEST_TIMEOUT_MS`, `HTTP_RETRY_AFTER_SECONDS`, `EVENTS_PAGE_SIZE_DEFAULT`, `EVENTS_PAGE_SIZE_MAX` and `VENUE_SERVICE_TIMEOUT_MS`.
- **Files changed:** the files listed in proposal.md (Impact), plus files that follow from them: `Cargo.toml`/`Cargo.lock` (Axum feature), `tests/common/db.rs` (harness module), `tests/event_service_test.rs` (new constructor and tests), `src/service/mod.rs` (module registration), and `AGENTS.md`.
- **Not verified here:** the CI run of the `test` job with `TEST_DATABASE_URL`. It runs when the branch is pushed.
