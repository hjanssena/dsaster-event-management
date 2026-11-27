# Database sessions specification delta

## Purpose

Define the limits and session settings of every database connection, and how database unavailability reaches the client, so time comparisons never depend on a server's local time zone and a slow database cannot hold requests open (ADR-002, ADR-003, ADR-008).

## ADDED Requirements

### Requirement: Sessions run in UTC
Every database connection Event Service opens SHALL use the `UTC` session time zone, whatever the database server's default.

#### Scenario: Server default is not UTC
- **WHEN** Event Service opens a connection to a server whose default time zone is `America/Mexico_City`
- **THEN** the session's `TimeZone` is `UTC`

### Requirement: Statements are time-limited
Every database connection SHALL limit each statement to `DB_STATEMENT_TIMEOUT_MS`. A request whose statement exceeds it SHALL get `503` with `Retry-After`.

#### Scenario: Session limit set
- **WHEN** Event Service opens a connection with `DB_STATEMENT_TIMEOUT_MS=5000`
- **THEN** the session's `statement_timeout` is 5000 milliseconds

#### Scenario: Statement too slow
- **WHEN** a statement runs longer than `DB_STATEMENT_TIMEOUT_MS`
- **THEN** the request gets `503` with `Retry-After`

### Requirement: Waiting for a connection is time-limited
Getting a connection from the pool SHALL wait at most `DB_POOL_ACQUIRE_TIMEOUT_MS`. A request that cannot get one in time SHALL get `503` with `Retry-After`.

#### Scenario: Pool exhausted
- **WHEN** every pooled connection stays busy longer than `DB_POOL_ACQUIRE_TIMEOUT_MS`
- **THEN** a request that needs a connection gets `503` with `Retry-After`

### Requirement: Unreachable database is unavailable, not an internal error
A request that fails because the database cannot be reached SHALL get `503` with `Retry-After`, not `500`.

#### Scenario: Database down
- **WHEN** the database refuses connections while a request needs it
- **THEN** the request gets `503` with `Retry-After`
