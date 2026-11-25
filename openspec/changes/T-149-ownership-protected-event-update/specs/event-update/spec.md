# Event update specification delta

## Purpose

Define ownership-protected partial updates for existing events via `PATCH /api/v1/events/{id}`.

## ADDED Requirements

### Requirement: Authenticate and authorize owning organizer
Only an authenticated partner with the `Organizer` role who owns the target event (`event.organizer_id == partner.id`) SHALL be permitted to update it. Missing, malformed, or unknown tokens MUST fail with `401 Unauthorized`. Non-organizer roles and organizers who do not own the event MUST fail with `403 Forbidden`. Missing event IDs MUST fail with `404 Not Found`.

#### Scenario: Missing or invalid authentication token
- **WHEN** `PATCH /api/v1/events/{id}` is called without a bearer token or with an invalid/unknown token
- **THEN** the response status is `401 Unauthorized` and no event is modified

#### Scenario: Non-organizer or non-owning organizer attempts update
- **WHEN** a `VenueOwner` or an organizer whose `id` differs from `event.organizer_id` calls `PATCH /api/v1/events/{id}`
- **THEN** the response status is `403 Forbidden` and the persisted event remains unchanged

#### Scenario: Target event does not exist
- **WHEN** an authenticated organizer calls `PATCH /api/v1/events/{id}` for a nonexistent event UUID
- **THEN** the response status is `404 Not Found`

### Requirement: Validate partial update payload and protect server-owned fields
`PATCH /api/v1/events/{id}` SHALL accept partial updates for `name`, `artist`, `date`, `schedule_id`, and `venue_id` (`venueId` alias). Requests containing server-owned fields (`id`, `organizer_id`, `status`, `created_at`, `updated_at`, `version`, `published_at`), unknown fields, malformed JSON, empty update payloads, blank text, lengths above 255 characters, or non-future `date` timestamps MUST be rejected with `400 Bad Request`. Changed `venue_id` values MUST be verified with `VenueClient` and return `422 Unprocessable Entity` if invalid or nonexistent.

#### Scenario: Client sends server-owned or unknown fields
- **WHEN** a request includes `id`, `organizer_id`, `status`, `created_at`, `updated_at`, or unknown fields
- **THEN** the response status is `400 Bad Request` with `{"error": "..."}` and no changes are persisted

#### Scenario: Validation failures on changed fields
- **WHEN** a request sends an empty JSON object, blank `name` or `artist`, over-255 character strings, or non-future `date`
- **THEN** the response status is `400 Bad Request` and original values remain intact

#### Scenario: Changed venue fails verification
- **WHEN** `venue_id` changes to a UUID rejected by `VenueClient::verify_venue_exists`
- **THEN** the response status is `422 Unprocessable Entity` and the original `venue_id` remains persisted

### Requirement: Target event schedule explicitly
Date updates SHALL target an explicit schedule row. When `schedule_id` is provided with `date`, it MUST belong to the event (`422` if not) and ONLY that schedule row SHALL be updated. When `schedule_id` is omitted with `date`, the update SHALL succeed only if the event has a single schedule, SHALL fail with `422 Unprocessable Entity` if the event has zero schedules, and SHALL fail with `400 Bad Request` if the event has multiple schedules. Sending `schedule_id` without `date` SHALL fail with `400 Bad Request`.

#### Scenario: Multi-schedule update with explicit schedule_id
- **WHEN** an event has multiple schedules and `PATCH` supplies both `schedule_id` (belonging to the event) and a future `date`
- **THEN** only that matching schedule's `starts_at` is updated and sibling schedules remain untouched

#### Scenario: Multi-schedule update without schedule_id
- **WHEN** an event has multiple schedules and `PATCH` supplies `date` without `schedule_id`
- **THEN** the response status is `400 Bad Request` and no schedule is modified

#### Scenario: Foreign or zero schedules
- **WHEN** `schedule_id` does not belong to the event or `date` is sent for an event with zero schedules
- **THEN** the response status is `422 Unprocessable Entity`

### Requirement: Persist event and schedule changes atomically
Changed event attributes and schedule start time MUST persist atomically within a single database transaction, refreshing `updated_at` using the server clock while preserving `id`, `organizer_id`, `status`, and `created_at`. Persistence failures MUST leave no partial updates and MUST NOT expose internal database error details.

#### Scenario: Successful partial or full update
- **WHEN** the owning organizer submits valid changes
- **THEN** the response status is `200 OK` with `EventResponseDto`, changed fields and `updated_at` reflect the update, `created_at`/`id`/`organizer_id`/`status` are unchanged, and subsequent `GET /api/v1/events/{id}` returns the persisted state

#### Scenario: Persistence failure rolls back atomically
- **WHEN** the repository encounters a persistence failure while updating the event or schedule
- **THEN** the endpoint returns `500 Internal Server Error` with a generic error message and zero partial state is persisted
