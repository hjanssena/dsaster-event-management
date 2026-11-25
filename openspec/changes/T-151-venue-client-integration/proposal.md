# T-151: Align HttpVenueClient with live Venue Service contract

## Why

Live integration testing against the running SubAgentes `venues` service (`MOCK_VENUE_SERVICE=false`) showed two mismatches in `HttpVenueClient`:
1. `HttpVenueClient` queried `GET /api/v1/venues/{id}`, receiving `404 Not Found` because `venue-service` mounts routes at `/venues/{id}` (and alias `/api/venues/{id}`).
2. `venue-service` protects `GET /venues/{id}` with Bearer JWT authentication (`401` when omitted), while `HttpVenueClient` sent no `Authorization` header and mapped non-404 errors (`401`, `403`, `5xx`) into `Ok(false)` (false `422` missing venue).

## What Changes

- Update `HttpVenueClient` in `src/service/venue_client.rs` to request `GET {base_url}/venues/{venue_id}` and send `Authorization: Bearer <token>` when `VENUE_SERVICE_TOKEN` is configured.
- Return `Ok(false)` only for `404 Not Found`; return `Err(AppError::InternalServerError)` for `401`, `403`, `5xx`, or transport failures so credential or upstream issues never masquerade as nonexistent venues.
- Read optional `VENUE_SERVICE_TOKEN` in `src/config.rs`, pass it in `src/main.rs`, and document it in `.env.example`.
- Add integration unit tests in `tests/venue_client_test.rs` exercising real HTTP responses from a local test listener.

## Capabilities

### New Capabilities

- `venue-client-integration`: Authenticated Venue existence lookup matching the live `GET /venues/{id}` contract.

### Modified Capabilities

None.

## Impact

- Exclusive files: `openspec/changes/T-151-venue-client-integration/*`, `src/config.rs`, `src/service/venue_client.rs`, `src/main.rs`, `.env.example`, `tests/venue_client_test.rs`.
- Zero overlap with `feat_T-149_event_update_patch`.
