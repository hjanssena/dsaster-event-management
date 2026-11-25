# Venue client integration validation

Validated on 2026-11-25 on `fix_T-151_venue_client_integration`.

## Automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo lint` (`clippy --locked --all-targets -- -D warnings`): passed.
- `cargo test`: passed (17 in `tests/event_api_test.rs`, 13 in `tests/event_service_test.rs`, 2 in `tests/venue_client_test.rs`).

## Live integration check

Verified against running `HectorBarrera13/venues` (`http://localhost:3001`, MongoDB `rs0`) and PostgreSQL 15 (`MOCK_VENUE_SERVICE=false`, `VENUE_SERVICE_TOKEN` configured):
- Existing mapped venue (`POST /api/v1/events` & `PATCH /api/v1/events/{id}`) verified cleanly (`201` / `200`).
- Missing venue UUID rejected with `422 Unprocessable Entity` (`404` from `GET /venues/{id}`).
