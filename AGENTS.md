# AGENTS.md

Instructions for AI coding agents working in this repository. Humans should start with [README.md](README.md).

## Project

Event Management microservice for Dsaster. It owns the `EventManagement` sub-domain: event creation, venue assignment, seat mapping, pricing tiers and sale scheduling.

- Rust (edition 2024), Axum 0.7, SeaORM 1.0 on PostgreSQL, Tokio, utoipa for OpenAPI.
- Architecture: [docs/architecture/architecture.md](docs/architecture/architecture.md). Components: [docs/component_diagram.md](docs/component_diagram.md).
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

Environment variables (read only in `src/config.rs`):

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | required | Postgres connection |
| `SERVER_ADDR` | `0.0.0.0:3000` | Bind address |
| `MOCK_AUTH` | `true` | In-memory dev partners instead of the `partners` table |
| `MOCK_VENUE_SERVICE` | `true` | `MockVenueClient` instead of calling Venue Management |
| `VENUE_SERVICE_URL` | `http://localhost:3001` | Venue Management base URL |

## Code conventions

- **Dependencies are traits.** Services take `Arc<dyn Trait>` (`EventRepository`, `PartnerRepository`, `VenueClient`, `TokenVerifier`). Every new external dependency gets a trait, a real implementation and a `Mock*` implementation, and `main.rs` selects between them.
- **Errors.** Return `AppError` from services and handlers. Use the variant that matches the status code (`BadRequest` 400, `Unauthorized` 401, `Forbidden` 403, `NotFound` 404, `UnprocessableEntity` 422). Never put database or internal error details in a response; `DatabaseError` and `InternalServerError` already log them and return a generic message.
- **Validation** lives in the service layer, not in handlers. Clamp pagination (`page >= 1`, `per_page` between 1 and 100) the way `EventService::get_events` does.
- **Auth.** Protected endpoints take the `AuthenticatedPartner` extractor from `src/api/auth.rs` and check the partner's role in the service. Tokens are `mock:<partner-uuid>` until the auth service defines the JWT.
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
