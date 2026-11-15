# ADRs - Event propagation

## ADR-001 - Soft-delete zone prices when the venue changes

| Status | Date | Capability |
|---|---|---|
| Proposed | 2026-11-07 | C2 - Select venue and price zones |

### Context

Changing the venue discards the previous venue's zone prices (C2). Zones belong to Venue Service; Event Service keeps one price row per zone per event in `event_pricing_tiers`, referencing the zone by `venue_zone_id`. Prices stay editable until sales open (PRD US-5, step 6), and no buyer pays before the lock (C5). The PRD asks for no price history.

### Decision

On a venue change, Event Service sets `deleted_at` on the previous venue's price rows and inserts one row per new zone with its `venue_zone_id`, a copy of the zone name in the existing `name` column, and the price. The copied name is the snapshot: later layout changes in Venue Service do not alter the event.

Editing a price on the same venue (Pit from `65000` to `70000`, $65 to $70, see ADR-005) updates the row in place. Every read of prices, including the check at Save (C4), ignores rows with `deleted_at` set. Seats live in Venue Service and are not stored here.

### Consequences

- The previous venue's prices stay in the database for later investigation.
- There is no price history; other services still receive each change through `EventUpdated` (C6).
- A new migration adds `venue_zone_id` and `deleted_at` to `event_pricing_tiers` and drops the unused `event_seats` table.
- A unique index on `event_id` and `venue_zone_id`, if added, must apply only where `deleted_at` is null.

### Alternatives considered

- **Hard delete.** Rejected: the previous venue's prices are lost.
- **Archive every price change.** Rejected for this release: no buyer pays a changed price and the PRD asks for no audit. It can be added later without undoing this decision.

## ADR-002 - Use the database clock for the lock and timestamptz for all times

| Status | Date | Capability |
|---|---|---|
| Proposed | 2026-11-07 | C5 - Lock event after sale window opens |

### Context

No edit may be saved once `now >= sale_starts_at` (C5). Event Service checks this after reading the event, but an edit checked just before sales open could be written just after. All current time columns are `timestamptz` and the models use `DateTimeUtc`.

### Decision

Every edit is written with `WHERE sale_starts_at > now()`, using the database clock; if no row is updated, Event Service returns `409 Event Locked`. Event Service still checks the lock before writing, so most locked edits are rejected without a write. Every time column, including new ones, uses `timestamptz`.

### Consequences

- The final check and the write are one statement, so no edit is saved after sales open.
- Comparisons do not depend on any time zone setting, because `timestamptz` values are absolute instants.
- The lock uses the database clock while C3 and C4 use Event Service's clock; if they differ, an edit can be rejected slightly early, never saved late.

### Alternatives considered

- **Pass Event Service's time to the query (`:now`).** Rejected: one clock for all checks, but the gap between check and write returns.
- **Check only in Event Service.** Rejected: an edit checked just before sales open could be saved after.

## ADR-003 - Publish events with a scheduled job at the visibility moment

| Status | Date | Capability |
|---|---|---|
| Proposed | 2026-11-07 | C6 - Event propagation to other services |

### Context

Event Service publishes `EventPublished` at `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS` and never earlier (C6). No request arrives at that moment, so something has to notice it. PRD step 7 computes visibility at query time, which serves Event Service's own endpoints but never reaches Booking Service or Search Service.

### Decision

A scheduled job in Event Service runs every `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` (ADR-006) and selects `Scheduled` events where `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS <= now()` and `published_at` is null. For each, one transaction sets `published_at` and writes `EventPublished` to the outbox; the relay publishes it to the Kafka topic `events.lifecycle`.

An event scheduled after its visibility moment is published by the scheduling transaction itself (C4), so the job skips it.

### Consequences

- Other services see an event up to `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` plus `OUTBOX_RELAY_INTERVAL_MS` after its visibility moment.
- A new migration adds `published_at` (`timestamptz`) to `events`.
- Setting `published_at` only where it is still null publishes each event once, even if the job runs twice or concurrently.
- Changing `EVENT_VISIBILITY_LEAD_DAYS` affects only unpublished events. This differs from the PRD criterion that a change affects every event on the next query.

### Alternatives considered

- **Publish at scheduling with a "visible from" date.** Rejected: events would propagate before their visibility moment, and Search Service would enforce a rule owned by Event Service.
- **Compute visibility only at query time.** Rejected: Booking Service and Search Service would never receive the event.

## ADR-004 - Integer event version and full-state messages

| Status | Date | Capability |
|---|---|---|
| Proposed | 2026-11-07 | C6 - Event propagation to other services |

### Context

`EventUpdated` carries the event version, and consumers discard versions they already have (C6). Keying `events.lifecycle` by event id keeps each event's messages in order (ADR-003), but Kafka delivers at least once, so a consumer can receive a message again after a crash, a relay retry or a replay. `events` has no version column today.

### Decision

`events` gets an integer `version`, incremented by the same update that saves an accepted edit; an edit blocked by the lock (ADR-002) leaves it unchanged. Every `EventPublished` and `EventUpdated` carries the full event (show details, zone prices, sales window) and its version. Each consumer stores the last version applied per event and drops any message whose version is not higher.

#### Writing to the outbox

The outbox row is written in the same Postgres transaction that changes the event:

- **Edit:** `UPDATE events SET …, version = version + 1 WHERE id = :id AND sale_starts_at > now() RETURNING *`. If the returned row has `published_at` set, insert `EventUpdated` built from it.
- **Publish:** `UPDATE events SET published_at = now() WHERE id = :id AND published_at IS NULL RETURNING *`, then insert `EventPublished` built from it. `version` does not change.

An edit decides whether to emit `EventUpdated` from its own `RETURNING`, never from an earlier read. Postgres makes a second update to the same row wait for the first to commit, so an edit racing the job sees the committed `published_at`, and each event's outbox rows follow version order.

#### Outbox table

| Column | Purpose |
|---|---|
| `id` (increasing) | Publish order |
| `event_id` | Kafka message key |
| `type` | `EventPublished` or `EventUpdated` |
| `version` | Copied from `events.version` |
| `payload` (`jsonb`) | Full event at commit time |
| `created_at`, `sent_at` (`timestamptz`) | Written at; published at (null until sent) |

The relay publishes unsent rows in `id` order to `events.lifecycle`, keyed by `event_id`, then sets `sent_at`. It never reads `events`, so each message carries the state of its own version.

### Consequences

- Repeated or replayed messages are dropped.
- The latest applied message always holds the complete state; no earlier message is needed.
- An edit racing the scheduled job is never left unpublished.
- Messages are larger than a list of changed fields.
- A new migration adds `version` and the outbox table.

### Alternatives considered

- **Order by `updated_at`.** Rejected: edits in the same millisecond tie, and clock skew (ADR-002) can give a later edit an earlier time.
- **Send only changed fields.** Rejected: a dropped or missed message would lose its change.

## ADR-005 - Prices in US dollars as integer thousandths

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-15 | C2 - Select venue and price zones, C6 - Event propagation to other services |

### Context

The team agreed that every service works in US dollars and stores amounts with the decimal point moved three places: `1000` is one dollar. Event Service stores `price` as `numeric` with a free `currency` column, and the API returns it as a decimal string (`"65.50"`). Booking Service and Search Service read prices from `EventPublished` and `EventUpdated` (C6). A decimal string or a JSON float is parsed differently by each language, and floats round.

### Decision

Every amount Event Service accepts, stores, returns or publishes is a 64-bit integer count of thousandths of a US dollar:

| Amount | Value |
|---|---|
| $1.00 | `1000` |
| $65.50 | `65500` |
| $0.001 | `1` |

- In JSON, `price` and `min_price` are integers, never strings or floats.
- A zone price must be greater than `0` (C2).
- Requests carry no currency. Responses and messages carry `"currency": "USD"` so a consumer can assert it.
- `event_pricing_tiers.price` becomes `bigint`, and `currency` is always `USD`.
- Converting to a display string (`65500` to `$65.50`) is done only by the frontend.

### Consequences

- No rounding or parsing differences between services.
- Breaking change to the current API: `price` and `min_price` change from a decimal string to an integer.
- A new migration converts `price` to `bigint` (multiplying existing values by 1000) and sets `currency` to `USD`.
- A price below one cent (for example `65505`) is valid for Event Service. Whether Payment accepts it is an open question in the [service contract](contract-event-service.md#open-questions).

### Alternatives considered

- **Decimal string (`"65.50"`).** Rejected: every consumer needs a decimal library and parses it differently.
- **Integer cents.** Rejected: the team chose three decimal places.
- **JSON float.** Rejected: binary floats round prices.

## ADR-006 - Every time, duration and numeric limit comes from an environment variable

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-15 | All |

### Context

The team agreed that no day count, duration or numeric limit is hardcoded. C6 already names `EVENT_VISIBILITY_LEAD_DAYS`, though `src/config.rs` does not read it yet. Other values were left open or written in code: ADR-003 ran the scheduled job "every X minutes", and `EventService::get_events` uses `10` and `100` for pagination.

### Decision

Each such value is an environment variable, read only in `src/config.rs` into `AppConfig` and passed to the code that uses it. The name ends with its unit (`_DAYS`, `_SECONDS`, `_MS`). Every variable has a default for local development, and Event Service refuses to start if a value is not a positive integer.

| Variable | Default | Used by |
|---|---|---|
| `EVENT_VISIBILITY_LEAD_DAYS` | `10` | C6: days before `sale_starts_at` when the event is published |
| `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` | `60` | ADR-003: how often the scheduled job looks for events to publish |
| `OUTBOX_RELAY_INTERVAL_MS` | `1000` | ADR-004: how often the relay reads unsent outbox rows |
| `OUTBOX_RELAY_BATCH_SIZE` | `100` | ADR-004: outbox rows the relay sends per read |
| `EVENTS_PAGE_SIZE_DEFAULT` | `10` | `per_page` when the request has none |
| `EVENTS_PAGE_SIZE_MAX` | `100` | Largest `per_page` accepted; larger values are clamped |
| `KAFKA_BROKERS` | `localhost:9092` | Kafka bootstrap servers |
| `KAFKA_EVENTS_TOPIC` | `events.lifecycle` | Topic for `EventPublished` and `EventUpdated` (C6) |

Timeouts, retries, circuit breakers and idempotency keys have their own variables, listed in ADR-008.

Rules that define the domain are not variables: a price greater than `0`, `page >= 1`, and the date order `now < sale_starts_at < sale_ends_at <= event_starts_at` (C3).

### Consequences

- Operations can tune lead time, latency and limits without a release.
- A new variable needs a line in this table, in `.env.example` and in the AGENTS.md variable table.
- Tests build `AppConfig` with explicit values instead of relying on defaults.

### Alternatives considered

- **Constants in code.** Rejected by the team decision: a change needs a release.
- **Settings table in the database.** Rejected: no request needs to change them at runtime, and a table adds an admin endpoint to secure.

## ADR-007 - The backend validates every identity, role and session

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-15 | All |

### Context

The team agreed that a service never trusts identity or session data sent by the frontend. The PRD shows why: a reseller froze seat holds by killing the checkout page's JavaScript, because the countdown lived in the browser. In Event Service today, the dev token `mock:<partner-uuid>` lets the client name the partner it acts as, and `MOCK_AUTH=false` still accepts that format. The current public list also accepts `status=Draft`, which exposes drafts (C1).

### Decision

- **Identity.** The organizer comes only from a bearer token that Event Service verifies (signature, expiry, issuer). `organizer_id` is never read from a body, query or header.
- **Role and ownership.** On every protected request the service layer checks that the partner exists, has the organizer role and owns the event. A partner that is not an organizer gets `403`. An event the caller does not own gets `404`, so its existence is not revealed.
- **Fields the server owns.** `id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at` and zone names are set by Event Service only. A request that sends one of them gets `400`.
- **Time.** Every time check uses the server clock (C3, C4) or the database clock (C5, ADR-002). The client's clock is never used.
- **Visibility.** Public endpoints return only published events, whatever filter the client sends (C1, C6).
- **Mock tokens.** `MOCK_AUTH=true` is for local development and tests only. Every shared environment (`testing`, production) runs with `MOCK_AUTH=false` and the Auth Service token.

### Consequences

- A client cannot act as another organizer, read another organizer's drafts or set server-owned state.
- Until Auth Service defines its token, no shared environment can satisfy this ADR. Verifying that token is a dependency for the first release.
- Each protected endpoint needs tests for `401`, `403` and `404` on another organizer's event.

### Alternatives considered

- **Trust an `organizer_id` sent by the frontend after login.** Rejected: any client can send any id.
- **Ignore server-owned fields silently.** Rejected: a `400` shows the client what it got wrong, and a silently dropped field hides the client bug.

## ADR-008 - Timeouts, retries, idempotency and circuit breakers on every call

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-15 | All |

### Context

The team agreed that every call to another service, and every call into ours, uses resiliency methods: a timeout, idempotency where a repeat could do harm, retries, and a circuit breaker where it makes sense. The PRD expects 100,000 fans at once on a hot sale, so a slow dependency must not hold requests open until Event Service runs out of connections.

Today the only protection is a hardcoded 3-second timeout in `HttpVenueClient`. Incoming requests have no timeout, `POST /api/v1/events` creates a second draft when a client retries it, and a Venue Service failure becomes a `500`.

### Decision

Each call gets the methods that fit it. All numbers are environment variables (ADR-006).

| Call | Timeout | Retry | Idempotency | Circuit breaker |
|---|---|---|---|---|
| **Out:** Venue Service (C2) | Per attempt | Yes | Reads only, safe to repeat | Yes |
| **Out:** Auth Service keys (ADR-007) | Per attempt | Yes | Reads only, safe to repeat | Yes, and keep using cached keys |
| **Out:** Kafka, outbox relay (ADR-004) | Per message | Yes, on the next relay run | Idempotent producer, versioned messages | No: the outbox absorbs the outage |
| **Out:** PostgreSQL | Pool acquire and statement | No retry inside a request | Transactions | No |
| **In:** HTTP API | Per request | Clients may retry | `Idempotency-Key` on `POST /api/v1/events`; other edits are idempotent | No |

#### Calls to other services

- **Timeouts.** Every attempt has its own timeout, and the attempts of one incoming request together never exceed `HTTP_REQUEST_TIMEOUT_MS`. When the remaining time is shorter than one attempt, Event Service stops retrying.
- **Retries.** Only on a connection error, a timeout, `429`, `502`, `503` or `504`; never on another `4xx`. Exponential backoff with full jitter: the wait before retry `n` is a random value between 0 and `RETRY_BASE_DELAY_MS * 2^n`, capped at `RETRY_MAX_DELAY_MS`. A `Retry-After` header from the other service is respected.
- **Circuit breaker.** One breaker per service. After `CIRCUIT_BREAKER_FAILURE_THRESHOLD` consecutive failed calls (after retries) it opens, and calls fail at once for `CIRCUIT_BREAKER_OPEN_SECONDS`. Then a single trial call is allowed: success closes the breaker, failure opens it again. 4xx responses other than `429` do not count as failures.
- **What the caller sees.** When Venue Service or Auth Service cannot answer, Event Service returns `503 Service Unavailable` with `Retry-After`, never `500`. An Auth Service outage never becomes `401`, so organizers are not logged out by it.
- **Auth Service keys.** Verification keys are cached for `AUTH_KEYS_CACHE_SECONDS`. If a refresh fails, Event Service keeps verifying with the cached keys and retries the refresh on the next request.

#### Kafka

- The producer is idempotent (`enable.idempotence=true`, `acks=all`), so a producer retry neither duplicates nor reorders messages within a partition. Each send waits at most `KAFKA_MESSAGE_TIMEOUT_MS`.
- The relay sets `sent_at` only after Kafka confirms the message. A failed message stays unsent, and the relay stops its batch at that row so later rows of the same event never go first.
- After a failure the relay waits twice as long before each new attempt, starting at `OUTBOX_RELAY_INTERVAL_MS` and capped at `OUTBOX_RELAY_MAX_BACKOFF_MS`, and returns to the normal interval after a success.
- No circuit breaker: organizers keep editing and scheduling while Kafka is down, and messages are sent from the outbox when it returns. Consumers already drop repeats by `version` (ADR-004).

#### PostgreSQL

- Getting a pooled connection waits at most `DB_POOL_ACQUIRE_TIMEOUT_MS`, and every statement runs with `statement_timeout = DB_STATEMENT_TIMEOUT_MS`. Either one failing returns `503` with `Retry-After`.
- A request never retries its transaction. The scheduled job (ADR-003) and the relay retry naturally on their next run.

#### Incoming requests

- **Timeout.** Each request is cut at `HTTP_REQUEST_TIMEOUT_MS` and gets `503` with `Retry-After`. Its transaction rolls back.
- **`POST /api/v1/events`** requires an `Idempotency-Key` header (a UUID created by the client for each new event form). Event Service stores the key, the organizer and the response for `IDEMPOTENCY_KEY_TTL_HOURS`:
  - Same key and body from the same organizer: the stored response is returned and no second draft is created.
  - Same key with a different body: `422`.
  - Same key while the first request is still running: `409`.
  - Missing header: `400`.
  - Keys are scoped to the organizer from the token, so one organizer cannot read another's stored response (ADR-007).
- **`PATCH`, `PUT` and `POST /{id}/schedule`** are idempotent by design: repeating one leaves the event in the same state. Scheduling an event that is already `Scheduled` returns `200` with the event instead of an error. A repeated edit may increase `version` and send an `EventUpdated` with the same content, which consumers apply as a normal upsert.
- **Clients** may retry `GET`, `PUT`, `PATCH`, the schedule call and `POST` with the same `Idempotency-Key`, on a timeout, `429` or `503`, with backoff and respecting `Retry-After`.

| Variable | Default | Used for |
|---|---|---|
| `HTTP_REQUEST_TIMEOUT_MS` | `10000` | Total time for one incoming request |
| `VENUE_SERVICE_TIMEOUT_MS` | `2000` | One attempt to Venue Service (replaces the hardcoded 3 seconds) |
| `AUTH_SERVICE_TIMEOUT_MS` | `2000` | One attempt to fetch Auth Service keys |
| `AUTH_KEYS_CACHE_SECONDS` | `300` | How long verification keys are cached |
| `RETRY_MAX_ATTEMPTS` | `3` | Attempts per outgoing call, including the first |
| `RETRY_BASE_DELAY_MS` | `100` | Base of the exponential backoff |
| `RETRY_MAX_DELAY_MS` | `2000` | Longest wait between attempts |
| `CIRCUIT_BREAKER_FAILURE_THRESHOLD` | `5` | Consecutive failures that open a breaker |
| `CIRCUIT_BREAKER_OPEN_SECONDS` | `30` | Time a breaker stays open before a trial call |
| `KAFKA_MESSAGE_TIMEOUT_MS` | `30000` | Time the producer tries to deliver one message |
| `OUTBOX_RELAY_MAX_BACKOFF_MS` | `30000` | Longest wait of the relay after failures |
| `DB_POOL_ACQUIRE_TIMEOUT_MS` | `3000` | Wait for a pooled database connection |
| `DB_STATEMENT_TIMEOUT_MS` | `5000` | Longest single SQL statement |
| `IDEMPOTENCY_KEY_TTL_HOURS` | `24` | How long an `Idempotency-Key` and its response are kept |

### Consequences

- A slow or failing dependency costs a bounded time per request, and an open breaker answers at once instead of piling up requests.
- Clients and consumers can retry safely; a lost response never creates a second draft or a duplicate message state.
- `AppError` gains a `ServiceUnavailable` variant (`503` with `Retry-After`).
- A new migration adds an `idempotency_keys` table (`key`, `organizer_id`, request hash, response status and body, `created_at`), and the scheduled job deletes rows older than `IDEMPOTENCY_KEY_TTL_HOURS`.
- Breaker state lives in each instance's memory, so with several instances each opens on its own. This is enough for one dependency per breaker.
- Timeouts and retries use `reqwest` and `tower`, already dependencies, plus the `timeout` feature of `tower-http`. The circuit breaker is a small state machine behind the `VenueClient` and `TokenVerifier` traits, with no new crate. The Kafka producer crate is needed for C6 anyway and is named in its proposal.
- Each behavior needs tests with the mocks: retry then success, retries exhausted to `503`, breaker open to immediate `503`, and a repeated `Idempotency-Key`.

### Alternatives considered

- **Retries without a breaker.** Rejected: during a Venue Service outage every request would wait for all its attempts and multiply the load on the failing service.
- **A breaker on Kafka.** Rejected: the outbox already decouples requests from Kafka, so failing fast would protect nothing.
- **Make `POST /api/v1/events` idempotent by event name.** Rejected: an organizer can create two drafts with the same name on purpose.
- **Shared breaker state in Redis.** Rejected for this release: it adds a dependency that itself needs a breaker.
