# Event update validation

Validated on 2026-11-25 on `feat_T-149_event_update_patch` using the `dsaster-rust-dev` (`rust:bookworm` + `rustfmt` + `clippy`) container.

## Required checks

- `cargo fmt --all -- --check`: passed.
- `cargo lint` (`clippy --locked --all-targets -- -D warnings`): passed with zero code warnings.
- `cargo test`: passed (26 API tests in `tests/event_api_test.rs`, 19 service tests in `tests/event_service_test.rs`).
- `cargo run --bin generate_openapi`: passed; regenerated `openapi.yml` includes `/api/v1/events/{id}` `patch` operation and `UpdateEventRequestDto`.
- `git diff --check`: passed.

## Scope & limitations

- All automated tests run against `MockEventRepository`, `MockPartnerRepository`, `MockTokenVerifier`, and `MockVenueClient` per `AGENTS.md` without requiring a live PostgreSQL instance in unit CI.
- Live PostgreSQL (`SeaOrmEventRepository` transaction execution) and live `HttpVenueClient` verification against `venue-service` are scheduled in Phase 2 (`T-151`).
