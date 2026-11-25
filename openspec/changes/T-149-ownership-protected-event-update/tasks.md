# Tasks

## 1. Specification

- [x] 1.1 Create OpenSpec proposal, design, spec delta, and tasks for `T-149`. Commit as `docs(T-149): add OpenSpec proposal and specs for event update endpoint`.

## 2. Backend implementation and OpenAPI contract

- [ ] 2.1 Add `UpdateEventRequestDto` and `EventUpdateChanges` in `src/model/dtos.rs`, implement atomic `update_event` in `src/repository/event_repo.rs` (SeaORM transaction + Mock failure support), and implement ownership/validation/schedule-targeting business logic in `src/service/event_srv.rs`.
- [ ] 2.2 Add `PATCH /api/v1/events/{id}` handler with strict `400` JSON parsing and `utoipa::path` metadata in `src/api/event_api.rs`, register route and schemas in `src/openapi.rs`, and regenerate `openapi.yml` via `cargo run --bin generate_openapi`. Commit as `feat(T-149): implement ownership-protected PATCH event updates and OpenAPI`.

## 3. Verification and testing

- [ ] 3.1 Add comprehensive service unit tests in `tests/event_service_test.rs` and HTTP router tests in `tests/event_api_test.rs` covering every scenario in `specs/event-update/spec.md` (validation, auth, ownership, schedule targeting, changed venue checks, atomic persistence failures, and OpenAPI schema assertions).
- [ ] 3.2 Run `cargo fmt --all -- --check`, `cargo lint`, `cargo test`, and `cargo run --bin generate_openapi`, and mark tasks complete. Commit as `test(T-149): cover validation, ownership, schedule targeting, and persistence failures`.
