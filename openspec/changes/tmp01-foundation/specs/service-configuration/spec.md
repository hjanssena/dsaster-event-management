# Service configuration specification delta

## Purpose

Define how Event Service reads its durations, limits and connection settings from environment variables, which defaults apply, and when it refuses to start (ADR-006, ADR-008).

## ADDED Requirements

### Requirement: Documented variables with defaults
Event Service SHALL read every variable listed in ADR-006 and ADR-008 from the environment, and SHALL use the documented default for each variable that is not set. `DATABASE_URL` has no default and SHALL be required.

#### Scenario: Variable not set
- **WHEN** Event Service starts without `HTTP_REQUEST_TIMEOUT_MS` set
- **THEN** it uses a request timeout of 10000 milliseconds

#### Scenario: Variable set
- **WHEN** Event Service starts with `EVENTS_PAGE_SIZE_MAX=50`
- **THEN** it uses 50 as the largest page size

#### Scenario: Database URL missing
- **WHEN** Event Service starts without `DATABASE_URL`
- **THEN** it refuses to start and names `DATABASE_URL` as missing

### Requirement: Numeric values must be positive integers
Every duration, size, count and limit variable SHALL be a positive integer. When one or more values are not, Event Service SHALL refuse to start and SHALL name every invalid variable in one message, not only the first.

#### Scenario: Zero value
- **WHEN** Event Service starts with `RETRY_MAX_ATTEMPTS=0`
- **THEN** it refuses to start and names `RETRY_MAX_ATTEMPTS`

#### Scenario: Non-numeric value
- **WHEN** Event Service starts with `DB_STATEMENT_TIMEOUT_MS=5s`
- **THEN** it refuses to start and names `DB_STATEMENT_TIMEOUT_MS`

#### Scenario: Several invalid values
- **WHEN** Event Service starts with `OUTBOX_RELAY_BATCH_SIZE=-1` and `HTTP_RETRY_AFTER_SECONDS=abc`
- **THEN** the refusal message names both variables

### Requirement: Default page size within the maximum
`EVENTS_PAGE_SIZE_DEFAULT` SHALL NOT be greater than `EVENTS_PAGE_SIZE_MAX`. Event Service SHALL refuse to start otherwise.

#### Scenario: Default above maximum
- **WHEN** Event Service starts with `EVENTS_PAGE_SIZE_DEFAULT=200` and `EVENTS_PAGE_SIZE_MAX=100`
- **THEN** it refuses to start and names both variables

### Requirement: Pagination uses the configured page sizes
List endpoints SHALL use `EVENTS_PAGE_SIZE_DEFAULT` when `per_page` is omitted, SHALL clamp `per_page` to between 1 and `EVENTS_PAGE_SIZE_MAX`, and SHALL treat a `page` of 0 as 1.

#### Scenario: Page size omitted
- **WHEN** a client lists events without `per_page` and `EVENTS_PAGE_SIZE_DEFAULT` is 7
- **THEN** the response has `per_page` 7

#### Scenario: Page size above the maximum
- **WHEN** a client lists events with `per_page=500` and `EVENTS_PAGE_SIZE_MAX` is 50
- **THEN** the response has `per_page` 50

### Requirement: Venue Service calls use the configured timeout
Each call to Venue Service SHALL be limited to `VENUE_SERVICE_TIMEOUT_MS`.

#### Scenario: Venue Service slower than the timeout
- **WHEN** Venue Service takes longer than `VENUE_SERVICE_TIMEOUT_MS` to answer
- **THEN** Event Service stops waiting for that call after `VENUE_SERVICE_TIMEOUT_MS`
