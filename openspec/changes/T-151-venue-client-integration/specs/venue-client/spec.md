# Venue client integration specification delta

## Purpose

Define the HTTP integration contract between `HttpVenueClient` and `venue-service`.

## ADDED Requirements

### Requirement: Query live Venue detail path with optional bearer token
`HttpVenueClient::verify_venue_exists` SHALL send `GET {base_url}/venues/{venue_id}` and include `Authorization: Bearer <token>` when `VENUE_SERVICE_TOKEN` is configured.

#### Scenario: Existing venue verified with bearer token
- **WHEN** `Venue Service` responds `200 OK` on `GET /venues/{id}` with the configured bearer token
- **THEN** `verify_venue_exists` returns `Ok(true)`

#### Scenario: Missing venue returns false only on 404
- **WHEN** `Venue Service` responds `404 Not Found` on `GET /venues/{id}`
- **THEN** `verify_venue_exists` returns `Ok(false)`

### Requirement: Surface upstream auth and server errors distinctly from missing venues
`HttpVenueClient` MUST NOT map `401`, `403`, or `5xx` responses to `Ok(false)`.

#### Scenario: Upstream 401/403/500 fails cleanly
- **WHEN** `Venue Service` responds `401 Unauthorized`, `403 Forbidden`, or `500 Internal Server Error`
- **THEN** `verify_venue_exists` returns an `Err(AppError::InternalServerError(_))`
