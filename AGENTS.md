# AGENTS.md

Instructions for AI coding agents working in this repository. Humans should start with [README.md](README.md).

## Project

Event Management microservice for Dsaster. It owns the `EventManagement` sub-domain: event creation, venue assignment, zone pricing, sale scheduling, and event propagation to downstream services.

- Rust (edition 2024), Axum 0.7, SeaORM 1.0 on PostgreSQL, Tokio, utoipa for OpenAPI.
- Target capabilities, contract & ADRs:
  - [docs/event-propagation/capabilities-and-design-event-propagation.md](docs/event-propagation/capabilities-and-design-event-propagation.md)
  - [docs/event-propagation/contract-event-service.md](docs/event-propagation/contract-event-service.md)
  - [docs/event-propagation/adrs-event-propagation.md](docs/event-propagation/adrs-event-propagation.md)
- Architecture: [docs/architecture/architecture.md](docs/architecture/architecture.md). Data model: [docs/architecture/data-model.md](docs/architecture/data-model.md). Components: [docs/component_diagram.md](docs/component_diagram.md).
- Team process (in Spanish): [docs/process/branching.md](docs/process/branching.md), [docs/process/development.md](docs/process/development.md).

## Layout

Strict layered architecture. Each layer only calls the one below it.

```text
src/
├── main.rs        # Wiring: config, DB, migrations, mock/real selection, router
├── lib.rs         # AppState and module exports
├── config.rs      # AppConfig::from_env(), the only place that reads env vars
├── openapi.rs     # ApiDoc: utoipa paths, schemas, Swagger UI
├── api/           # Axum handlers + utoipa::path annotations. No business logic.
├── service/       # Business rules and validation. No HTTP types.
├── repository/    # Data access behind traits (SeaORM impl + Mock impl)
├── model/         # SeaORM entities and dtos.rs (request/response payloads)
├── core/error.rs  # AppError → HTTP status + {"error": "..."} body
├── migration/     # SeaORM migrations, registered in migration/mod.rs
└── bin/generate_openapi.rs  # Writes openapi.yml
tests/             # Integration tests (API through the router, services directly)
openspec/          # Specs and change proposals (see Spec-driven workflow)
```

## Commands

| Task | Command |
|---|---|
| Start the database | `docker compose up -d event-db` |
| Run the service | `cargo run` (needs `.env`, copy from `.env.example`) |
| Format | `cargo fmt --all` |
| Lint (CI) | `cargo fmt --all -- --check && cargo lint` (or `npm run lint`) |
| Test (CI) | `cargo test` |
| Regenerate the API contract | `cargo run --bin generate_openapi` |

`cargo lint` is an alias in `.cargo/config.toml` for `clippy --locked --all-targets -- -D warnings`: any warning fails. Tests use the in-memory mocks and need no database.

A task is done only when the format check, `cargo lint` and `cargo test` all pass. CI (`.github/workflows/validation.yml`) runs the same commands on every PR to `main` and `testing`.

Environment variables (read only in `src/config.rs`; see ADR-006, ADR-007, ADR-008):

| Variable | Default | Status | Purpose |
|---|---|---|---|
| `DATABASE_URL` | required | Implemented | Postgres connection |
| `SERVER_ADDR` | `0.0.0.0:3000` | Implemented | Bind address |
| `MOCK_AUTH` | `true` | Implemented | Local dev/tests only: mock tokens & dev partners instead of `HS256` JWT (ADR-007) |
| `MOCK_VENUE_SERVICE` | `true` | Implemented | `MockVenueClient` instead of calling Venue Service |
| `VENUE_SERVICE_URL` | `http://localhost:3001` | Implemented | Venue Service base URL |
| `JWT_SECRET` | required when `MOCK_AUTH=false` | Planned (ADR-007) | Shared `HS256` secret for local JWT verification (never commit) |
| `EVENT_VISIBILITY_LEAD_DAYS` | `10` | Planned (ADR-006) | Days before `sale_starts_at` when an event is published (C6) |
| `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` | `60` | Planned (ADR-006) | Scheduled publish job interval (ADR-003) |
| `OUTBOX_RELAY_INTERVAL_MS` | `1000` | Planned (ADR-006) | Outbox relay polling interval (ADR-004) |
| `OUTBOX_RELAY_BATCH_SIZE` | `100` | Planned (ADR-006) | Outbox rows processed per relay read (ADR-004) |
| `EVENTS_PAGE_SIZE_DEFAULT` | `10` | Planned (ADR-006) | Default `per_page` when omitted |
| `EVENTS_PAGE_SIZE_MAX` | `100` | Planned (ADR-006) | Maximum `per_page` accepted (larger values are clamped) |
| `KAFKA_BROKERS` | `localhost:9092` | Planned (ADR-006) | Kafka bootstrap servers |
| `KAFKA_EVENTS_TOPIC` | `events.lifecycle` | Planned (ADR-006) | Topic for `EventPublished` and `EventUpdated` (C6) |
| `HTTP_REQUEST_TIMEOUT_MS` | `10000` | Planned (ADR-008) | Total time for one incoming HTTP request |
| `VENUE_SERVICE_TIMEOUT_MS` | `2000` | Planned (ADR-008) | Per-attempt timeout calling Venue Service |
| `RETRY_MAX_ATTEMPTS` | `3` | Planned (ADR-008) | Total attempts per outgoing call, including the first |
| `RETRY_BASE_DELAY_MS` | `100` | Planned (ADR-008) | Base delay for exponential backoff with full jitter |
| `RETRY_MAX_DELAY_MS` | `2000` | Planned (ADR-008) | Longest wait between retry attempts |
| `CIRCUIT_BREAKER_FAILURE_THRESHOLD` | `5` | Planned (ADR-008) | Consecutive failures (after retries) that open a breaker |
| `CIRCUIT_BREAKER_OPEN_SECONDS` | `30` | Planned (ADR-008) | Time a breaker stays open before a trial call |
| `KAFKA_MESSAGE_TIMEOUT_MS` | `30000` | Planned (ADR-008) | Time the Kafka producer tries to deliver one message |
| `OUTBOX_RELAY_MAX_BACKOFF_MS` | `30000` | Planned (ADR-008) | Longest wait of the outbox relay after failures |
| `DB_POOL_ACQUIRE_TIMEOUT_MS` | `3000` | Planned (ADR-008) | Wait for a pooled database connection |
| `DB_STATEMENT_TIMEOUT_MS` | `5000` | Planned (ADR-008) | Longest single SQL statement |
| `IDEMPOTENCY_KEY_TTL_HOURS` | `24` | Planned (ADR-008) | Retention time for `Idempotency-Key` records |

## Code conventions

- **Dependencies are traits.** Services take `Arc<dyn Trait>` (`EventRepository`, `PartnerRepository`, `VenueClient`, `TokenVerifier`). Every new external dependency gets a trait, a real implementation and a `Mock*` implementation, and `main.rs` selects between them.
- **Errors.** Return `AppError` from services and handlers. Use the variant that matches the status code (`BadRequest` 400 for malformed requests, server-owned fields or missing `Idempotency-Key`; `Unauthorized` 401; `Forbidden` 403; `NotFound` 404; `Conflict` 409 for event locked or in-progress idempotency key; `UnprocessableEntity` 422 for domain rule violations; `ServiceUnavailable` 503 with `Retry-After` for timeouts or unavailable dependencies). Never put database or internal error details in a response; `DatabaseError` and `InternalServerError` already log them and return a generic message.
- **Validation** lives in the service layer, not in handlers. Clamp pagination (`page >= 1`, `per_page` between `1` and `EVENTS_PAGE_SIZE_MAX`, defaulting to `EVENTS_PAGE_SIZE_DEFAULT`) the way `EventService::get_events` does.
- **Auth.** Protected endpoints take the `AuthenticatedPartner` extractor from `src/api/auth.rs` and check the partner's role (`ORGANIZER`, case-insensitive) and ownership in the service. Per ADR-007, Auth Service issues `HS256` JWTs (`sub`, `role`, `iat`, `exp`) verified locally with `JWT_SECRET` (no runtime call to Auth Service); `mock:<partner-uuid>` remains wired for local development and tests (`MOCK_AUTH=true`) until `MockTokenVerifier` is replaced.
- **Never trust the client.** Identity, role and ownership come only from the token the service verifies, never from a body, query or header field. Fields the server owns (`id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at`, zone `name`) are rejected with 400 if a request sends them. Time checks use the server or database clock. See ADR-007 in `docs/event-propagation/adrs-event-propagation.md`.
- **No hardcoded days, durations or limits.** Every one is an environment variable read in `src/config.rs`, with its unit in the name (`_DAYS`, `_SECONDS`, `_MS`). Add it to ADR-006, `.env.example` and the table above. Domain rules (price > 0, date order) stay in code.
- **Resiliency on every call** (ADR-008). Outgoing calls have a per-attempt timeout, retries with exponential backoff and jitter on connection errors, timeouts, 429 and 5xx, and a circuit breaker per service; an unavailable dependency returns `503` with `Retry-After`, never `500` or `401`. Incoming requests are cut at `HTTP_REQUEST_TIMEOUT_MS`. `POST` endpoints that create something require an `Idempotency-Key`; other edits must be safe to repeat.
- **Money is an integer in thousandths of a US dollar** (`1000` = $1.00, `65500` = $65.50): `i64` in Rust, `bigint` in Postgres, a JSON integer in the API and Kafka. Never `f64`, `Decimal` or a string. See ADR-005.
- **Migrations.** Add a new file `src/migration/mYYYYMMDD_NNNNNN_<description>.rs` and register it in `src/migration/mod.rs`. Never edit a migration that is already merged.
- **Routes** are mounted under `/api/v1/events`. `/events` is a compatibility alias; don't add new routes only to the alias.
- Write new code, comments and docs in English. Existing Spanish comments may stay as they are.

## API contract

The OpenAPI document is generated from code, and `openapi.yml` is committed. When you add or change an endpoint or DTO:

1. Add or update its `#[utoipa::path(...)]` annotation in `src/api/` and the `ToSchema`/`IntoParams` derive in `src/model/dtos.rs`.
2. Register new paths and schemas in `ApiDoc` in `src/openapi.rs`.
3. Run `cargo run --bin generate_openapi` and commit the updated `openapi.yml` in the same commit as the change.

Swagger UI is served at `/swagger-ui` and the JSON at `/api-docs/openapi.json`.

## Testing

- API tests go in `tests/event_api_test.rs`. Build the router with `create_test_app` and the mock repository and clients, then send requests with `tower::ServiceExt::oneshot`.
- Service tests go in `tests/event_service_test.rs` and call `EventService` with mocks.
- Every new behavior and every error status code an endpoint can return needs a test.

## Branches, commits and PRs

The `commit-msg` hook (husky + `commitlint.config.js`) rejects messages that don't follow these rules.

- **Commit format:** `type(TICKET): subject`
  - `type` is one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`.
  - `TICKET` is the task ID from the project board (e.g. `VE05T1`, `EV14T01`), or `NA` when there is no ticket. It is required and must match `^[A-Z0-9_-]+$`.
  - The subject has no trailing period.
  - Good: `feat(VE05T1): validate venue exists when creating an event`
  - Rejected: `core(NA): ...` (invalid type), `docs(components diagram): ...` (invalid scope), `feat: add endpoint` (no scope).
- **One commit per task.** Don't bypass the hook with `--no-verify`.
- **Branch name:** `<type>_<TICKET>_<shortDescription>`, e.g. `feat_VE07T1_partner_role_validation` or `docs_NA_update_readme`.
- **Flow:** ticket branch → PR into `testing` → PR from `testing` into `main`. Never commit directly to `main` or `testing`. Use the same format as the commits for the PR title.

## Spec-driven workflow (OpenSpec)

Rule: no code before a spec you have read and approved. `openspec/specs/` describes how the service behaves today; `openspec/changes/` holds work in progress. Project rules for proposals and tasks are in `openspec/config.yaml`. The skills call the `openspec` CLI, so install it first: `npm install -g @fission-ai/openspec@latest`.

1. Create the ticket branch.
2. Propose: `/opsx:propose "VE05T1: <ticket title>"` (Claude Code) or `/openspec-propose` (other agents using `.agents/skills`). Wait for a human to review `proposal.md`, the delta specs and `tasks.md` before applying.
3. Apply: `/opsx:apply`, one commit per task.
4. Open the PR with the change folder included.
5. After merge: `/opsx:archive`, so the deltas are merged into `openspec/specs/`.

If requirements change during implementation, update the spec first and re-plan. Don't agree on changes only in chat.

## Do not

- Run `scripts/bump.sh` or push tags; a tag publishes a release.
- Commit `.env` or real credentials.
- Edit files in `node_modules/`.
- Edit existing migrations, `Cargo.lock` by hand, or `.github/workflows/` unless the ticket asks for it.
- Add a crate without stating why in the proposal.
