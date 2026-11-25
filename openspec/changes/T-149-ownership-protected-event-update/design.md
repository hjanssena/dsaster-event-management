# Design

## Context

`dsaster-event-management` follows a strict layered architecture (`api/` -> `service/` -> `repository/` -> `model/`). `venues-front` (`EventEditForm.jsx` + `EventsApiClient.updateEvent`) sends `PATCH /api/v1/events/{id}` with `name`, `artist`, `date` (UTC RFC 3339), and `venue_id`, and expects an HTTP `200` confirmation payload containing the persisted event state.

## Goals / Non-Goals

**Goals:**
- Provide `PATCH /api/v1/events/{id}` through existing traits and layers.
- Enforce organizer role and event ownership strictly from the verified `AuthenticatedPartner` token.
- Keep `id`, `organizer_id`, `status`, and `created_at` immutable and `updated_at` server-controlled.
- Ensure atomic updates across `events` and `event_schedules`.
- Guarantee explicit schedule targeting so multi-schedule events can never silently overwrite an arbitrary row.

**Non-Goals:**
- Event deletion, schedule creation/deletion, seat maps, pricing tier edits, or Kafka outbox event propagation (covered by separate future capabilities).
- Migrating from `MockTokenVerifier` to HS256 JWT verification (covered by ADR-007).

## Decisions

1. **Strict Payload Parsing in Handler (`400 Bad Request` for server-owned/unknown fields):**
   `UpdateEventRequestDto` uses `#[serde(deny_unknown_fields)]` with optional fields `name`, `artist`, `date`, `schedule_id`, and `venue_id` (aliasing `venueId`). The handler extracts raw `Bytes` and returns `AppError::BadRequest` (`{"error": "..."}`) on malformed JSON, wrong types, unknown fields, or server-owned fields (`id`, `organizer_id`, `status`, `created_at`, `updated_at`, `version`, `published_at`) instead of Axum's plain-text `422`.

2. **Explicit Schedule Targeting Helper in Service Layer:**
   - When `date` is omitted and `schedule_id` is present -> `400 Bad Request`.
   - When `date` is present and `schedule_id` is present -> verify `schedule_id` exists in `existing.schedules`; if missing -> `422 Unprocessable Entity`.
   - When `date` is present and `schedule_id` is omitted:
     - `existing.schedules.len() == 1` -> target `existing.schedules[0].id`.
     - `existing.schedules.is_empty()` -> `422 Unprocessable Entity`.
     - `existing.schedules.len() > 1` -> `400 Bad Request` (requires explicit `schedule_id`).

3. **Selective Venue Verification:**
   Verify `venue_id` through `VenueClient::verify_venue_exists` only when `venue_id` is supplied and differs from `existing.venue_id`, avoiding redundant calls on metadata-only edits while enforcing `422 Unprocessable Entity` on nonexistent venues and propagating communication failures cleanly.

4. **Atomic Repository Update Contract:**
   Define `EventUpdateChanges` as a validated internal command passed from `EventService::update_event` to `EventRepository::update_event`. `SeaOrmEventRepository` executes both `events` and `event_schedules` updates inside `self.db.begin().await?` and commits only after both succeed. `MockEventRepository` supports deterministic failure simulation to verify atomic rollback in unit and HTTP tests.

## Risks / Trade-offs

- **Multi-schedule UI limitation:** `venues-front` T-150 sends `date` without `schedule_id` (assuming single-schedule events created via `POST /api/v1/events`). Requiring `schedule_id` only when `schedules.len() > 1` preserves full compatibility with single-schedule events while protecting future multi-schedule events from ambiguous mutations.
- **No database migration needed:** All columns (`updated_at`, `venue_id`, `name`, `artist`, `starts_at`) already exist in PostgreSQL entities.
