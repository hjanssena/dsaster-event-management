# Request timeout specification delta

## Purpose

Bound the time any incoming request can take, so a slow dependency cannot hold connections open, and give clients a retryable answer when that bound is hit (ADR-008).

## ADDED Requirements

### Requirement: Requests are cut at the request timeout
A request that has not completed within `HTTP_REQUEST_TIMEOUT_MS` SHALL be answered with `503`, the JSON error body and `Retry-After: HTTP_RETRY_AFTER_SECONDS`. Work in progress for that request SHALL be abandoned, and its database transaction SHALL NOT commit.

#### Scenario: Slow request
- **WHEN** handling a request takes longer than `HTTP_REQUEST_TIMEOUT_MS`
- **THEN** the client gets `503` with the JSON error body and `Retry-After` set to `HTTP_RETRY_AFTER_SECONDS`, within about `HTTP_REQUEST_TIMEOUT_MS`

#### Scenario: Fast request
- **WHEN** handling a request finishes within `HTTP_REQUEST_TIMEOUT_MS`
- **THEN** the client gets the handler's response unchanged

#### Scenario: Abandoned transaction
- **WHEN** a request times out while its database transaction is open
- **THEN** none of that transaction's changes are saved

### Requirement: Every 503 carries Retry-After
Every `503` response SHALL carry `Retry-After`. A `503` that does not set one SHALL get `HTTP_RETRY_AFTER_SECONDS`; a `503` that sets one SHALL keep it.

#### Scenario: 503 without Retry-After
- **WHEN** a request ends in `503` without a `Retry-After` header
- **THEN** the response carries `Retry-After` set to `HTTP_RETRY_AFTER_SECONDS`

#### Scenario: 503 with its own Retry-After
- **WHEN** a request ends in `503` with `Retry-After: 12`
- **THEN** the response keeps `Retry-After: 12`

### Requirement: Request deadline available to outgoing calls
While a request is handled, its deadline (arrival time plus `HTTP_REQUEST_TIMEOUT_MS`) SHALL be available to the code that calls other services, so retries can stop before the deadline.

#### Scenario: Deadline during a request
- **WHEN** code runs while a request that arrived at time T is handled
- **THEN** it can read the deadline T plus `HTTP_REQUEST_TIMEOUT_MS`

#### Scenario: No request
- **WHEN** code runs outside any request, such as a background job
- **THEN** it reads no deadline
