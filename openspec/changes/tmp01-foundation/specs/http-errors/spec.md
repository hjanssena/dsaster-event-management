# HTTP errors specification delta

## Purpose

Define the status codes and the single JSON error body Event Service returns, including for requests it cannot parse and for request times, so clients can rely on the status code alone (contract Conventions, ADR-008, ADR-010).

## ADDED Requirements

### Requirement: One JSON error body
Every error response SHALL have the body `{"error": "<message>"}` with content type `application/json`. Internal and database details SHALL NOT appear in the message.

#### Scenario: Not found
- **WHEN** a client requests an event that does not exist
- **THEN** the response is `404` with a JSON body that has an `error` string

#### Scenario: Database failure
- **WHEN** a request fails because of an unexpected database error
- **THEN** the response is `500` with the message `Internal server error` and no database details

### Requirement: Unparseable requests get a JSON 400
A request body, path parameter or query parameter that the service cannot parse SHALL get `400` with the JSON error body. This covers invalid JSON, a wrong type, a missing required field, a missing or wrong `Content-Type`, a malformed UUID in the path and a malformed query value.

#### Scenario: Invalid JSON
- **WHEN** a client sends a body that is not valid JSON to an endpoint that takes JSON
- **THEN** the response is `400` with the JSON error body

#### Scenario: Wrong type
- **WHEN** a client sends a number where the body expects a string
- **THEN** the response is `400` with the JSON error body

#### Scenario: Missing required field
- **WHEN** a client omits a required body field
- **THEN** the response is `400` with the JSON error body

#### Scenario: Missing content type
- **WHEN** a client sends a JSON body without `Content-Type: application/json`
- **THEN** the response is `400` with the JSON error body

#### Scenario: Malformed path UUID
- **WHEN** a client requests `/api/v1/events/not-a-uuid`
- **THEN** the response is `400` with the JSON error body

#### Scenario: Malformed query value
- **WHEN** a client lists events with `page=abc`
- **THEN** the response is `400` with the JSON error body

### Requirement: Request times need an offset
A time in a request SHALL be an RFC 3339 string with a timezone offset and SHALL be converted to UTC. A valid date and time without an offset SHALL get `422`. A string that is not a date and time SHALL get `400`. A JSON value that is not a string SHALL get `400`.

#### Scenario: Time with offset
- **WHEN** a request sends `2099-12-01T10:00:00-06:00`
- **THEN** the service uses `2099-12-01T16:00:00Z`

#### Scenario: Time without offset
- **WHEN** a request sends `2099-12-01T10:00:00`
- **THEN** the response is `422` with a message saying the time needs a timezone offset

#### Scenario: Not a time
- **WHEN** a request sends `tomorrow` as a time
- **THEN** the response is `400` with the JSON error body

#### Scenario: Not a string
- **WHEN** a request sends the number `1700000000` as a time
- **THEN** the response is `400` with the JSON error body

### Requirement: Conflict status
A request that conflicts with the current state of the resource SHALL get `409` with the JSON error body.

#### Scenario: Conflict reported
- **WHEN** the service reports a conflict for a request
- **THEN** the response is `409` with the JSON error body

### Requirement: Service unavailable status
When Event Service or a service it depends on cannot answer, the response SHALL be `503` with the JSON error body and a `Retry-After` header in whole seconds. When the cause knows how long to wait, that value SHALL be used; otherwise `HTTP_RETRY_AFTER_SECONDS`.

#### Scenario: Known wait
- **WHEN** a dependency reports that it can be tried again in 12 seconds
- **THEN** the response is `503` with `Retry-After: 12`

#### Scenario: Unknown wait
- **WHEN** a dependency is unavailable and gives no wait, with `HTTP_RETRY_AFTER_SECONDS=2`
- **THEN** the response is `503` with `Retry-After: 2`
