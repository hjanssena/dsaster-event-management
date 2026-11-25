# T-149: Ownership-protected event updates via PATCH /api/v1/events/{id}

## Why

Organizers already register events (`POST /api/v1/events`), list their events (`GET /api/v1/events`), and view details (`GET /api/v1/events/{id}`), and `venues-front` already ships the `/events/{id}/edit` UI targeting `PATCH /api/v1/events/{id}` (T-150). Without the backend endpoint, organizers cannot persist edits to event metadata, schedule, or venue.

## What Changes

- Add `PATCH /api/v1/events/{id}` (and compatibility alias `/events/{id}`) protected by `AuthenticatedPartner`.
- Enforce `ORGANIZER` role and strict event ownership (`event.organizer_id == partner.id`), rejecting unauthorized callers with `401` and non-organizers or non-owners with `403`.
- Support partial updates to `name`, `artist`, `date` (targeting an explicit schedule via optional `schedule_id`), and `venue_id` (`venueId` alias accepted).
- Reject server-owned or unknown payload fields (`id`, `organizer_id`, `status`, `created_at`, `updated_at`, `version`, `published_at`) and empty update payloads with `400 Bad Request`.
- Validate changed text fields (non-blank trimmed values within 255 characters) and changed dates (must be future UTC timestamps).
- Verify changed `venue_id` values through `VenueClient::verify_venue_exists`, returning `422 Unprocessable Entity` when the venue is nonexistent or invalid.
- Enforce explicit schedule targeting: update `schedule_id` when supplied (requiring it to belong to the event, else `422`), allow omitting `schedule_id` only when the event has exactly one schedule, reject zero-schedule date updates with `422`, and reject ambiguous multi-schedule date updates without `schedule_id` with `400` so an arbitrary schedule is never updated.
- Persist event and targeted schedule changes atomically inside a SeaORM database transaction, updating only changed fields and server-controlled `updated_at` while preserving `id`, `organizer_id`, `status`, and `created_at`.
- Update `utoipa` annotations, register the path and DTO in `ApiDoc`, and regenerate `openapi.yml`.

## Capabilities

### New Capabilities

- `event-update`: Ownership-protected partial updates for events and explicitly targeted schedules.

### Modified Capabilities

None; `openspec/specs/` currently contains no prior capability specifications.

## Impact

- Affected layers: `src/model/dtos.rs`, `src/repository/event_repo.rs`, `src/service/event_srv.rs`, `src/api/event_api.rs`, `src/openapi.rs`, and generated `openapi.yml`.
- Affected tests: `tests/event_service_test.rs` and `tests/event_api_test.rs`.
- No new crates or database migrations are required (`events` and `event_schedules` tables already exist).
