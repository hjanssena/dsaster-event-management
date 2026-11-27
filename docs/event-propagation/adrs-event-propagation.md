# ADRs - Event propagation

## ADR-001 - Soft-delete zone prices when the venue changes

| Status | Date | Capability |
|---|---|---|
| Proposed, amended 2026-11-27 (twice) | 2026-11-07 | C2 - Select venue and price zones |

### Context

Changing the venue discards the previous venue's zone prices (C2). Zones belong to Venue Service, which keeps them in numbered seat map versions that never change once saved; Event Service keeps one price row per zone per event in `event_pricing_tiers`, referencing the zone by `venue_zone_id`. Prices stay editable until sales open (PRD US-5, step 6), and no buyer pays before the lock (C5). The PRD asks for no price history.

### Decision

On a venue change, Event Service sets `deleted_at` on the previous venue's price rows and inserts one row per new zone with its `venue_zone_id`, a copy of the zone name in the existing `name` column, and the price. The copied name is the snapshot: later layout changes in Venue Service do not alter the event.

Every save of the venue reads the venue's **latest** seat map version from Venue Service (`GET /venues/{id}/map`), also when the venue does not change, and records its number in `events.venue_map_version`. A venue without a map (`latestMapVersion` `0`) gets `422`. Zones are matched by their map id, which Venue Service keeps stable across versions. On the same venue, each zone is handled this way:

| Zone | Result |
|---|---|
| Still in the latest map, and in the request | Row updated in place: new price (Pit from `65000` to `70000`, $65 to $70, see ADR-005) and the zone's current name |
| Still in the latest map, left out of the request | Keeps its price; its name is refreshed |
| No longer in the latest map | `deleted_at` is set, whatever the request says |
| New in the latest map | New row; priced if the request includes it, otherwise without a price |

If the request lists the same `venue_zone_id` more than once, the last entry replaces the earlier ones. On a new venue, the zones the request leaves out start without a price. Every read of prices, including the check at Save (C4), ignores rows with `deleted_at` set. Seats live in Venue Service and are not stored here: consumers read them from the event's `venue_map_version`, which never changes.

### Consequences

- The previous venue's prices stay in the database for later investigation.
- There is no price history; other services still receive each change through `EventUpdated` (C6).
- The schema has `events.venue_map_version` and a string `event_pricing_tiers.venue_zone_id` (map ids are not UUIDs), plus `deleted_at`; the unused `event_seats` table is dropped.
- A unique index on `event_id` and `venue_zone_id`, if added, must apply only where `deleted_at` is null.
- Saving the venue again moves the event to the latest map version and refreshes zones and names. Between saves, new map versions never reach the event.
- On a `Scheduled` event, a zone added to the venue starts without a price, so saving the venue gets `422` (C4) unless the request prices it.
- Venue Service publishes no change events, so the snapshot and the recorded map version are what keep an event stable.

### Alternatives considered

- **Hard delete.** Rejected: the previous venue's prices are lost.
- **Archive every price change.** Rejected for this release: no buyer pays a changed price and the PRD asks for no audit. It can be added later without undoing this decision.

### Amendment - 2026-11-27

The first version did not say what saving the same venue again does. The team decided that every save reads the zones again; a zone left out of the request keeps its price unless the zone no longer exists in the venue, and a duplicate `venue_zone_id` is replaced by its last entry. The decision above now states this.

### Amendment - 2026-11-27 (seat maps)

Venue Service now serves zones and seats in versioned seat maps (`venues` `main`, `88c214b`): `GET /venues/{id}/map[?version=N]`, with zone and seat ids that are strings and stay stable across versions, and stored versions that never change. The decision now reads the zones from the latest map, records the version on the event (`venue_map_version`) so consumers can read the exact seats, and rejects a venue that has no map. `venue_zone_id` changes from a UUID to a string of up to 64 characters.

## ADR-002 - Use the database clock for the lock and timestamptz for all times

| Status | Date | Capability |
|---|---|---|
| Proposed, amended 2026-11-27 | 2026-11-07 | C5 - Lock event after sale window opens |

### Context

No edit may be saved once `now >= sale_starts_at` (C5). Event Service checks this after reading the event, but an edit checked just before sales open could be written just after. All current time columns are `timestamptz` and the models use `DateTimeUtc`.

### Decision

Every edit is written with `WHERE status = 'Draft' OR sale_starts_at > now()`, using the database clock; if no row is updated, Event Service returns `409 Event Locked`. The lock applies only to `Scheduled` events: a draft may have no sales window yet (`sale_starts_at` is null), and a draft whose sales start has passed must stay editable so it can get a new window. Scheduling such a draft gets `422` (C4). Product still has to confirm this (see [Open questions](contract-event-service.md#open-questions)). Scheduling is under the same lock: scheduling an event that is already `Scheduled` returns it unchanged until sales open, and gets `409` after. Event Service still checks the lock before writing, so most locked edits are rejected without a write. Every time column, including new ones, uses `timestamptz`.

### Consequences

- The final check and the write are one statement, so no edit is saved after sales open.
- Comparisons do not depend on any time zone setting, because `timestamptz` values are absolute instants.
- The lock uses the database clock while C3 and C4 use Event Service's clock; if they differ, an edit can be rejected slightly early, never saved late.

### Alternatives considered

- **Pass Event Service's time to the query (`:now`).** Rejected: one clock for all checks, but the gap between check and write returns.
- **Check only in Event Service.** Rejected: an edit checked just before sales open could be saved after.

### Amendment - 2026-11-27

Scheduling an already `Scheduled` event after sales open was undefined. The team decided that once sales open the event can no longer be edited or scheduled again, so it gets `409` like any edit.

## ADR-003 - Publish events with a scheduled job at the visibility moment

| Status | Date | Capability |
|---|---|---|
| Proposed, amended 2026-11-27 | 2026-11-07 | C6 - Event propagation to other services |

### Context

Event Service publishes `EventPublished` at `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS` and never earlier (C6). No request arrives at that moment, so something has to notice it. PRD step 7 computes visibility at query time, which serves Event Service's own endpoints but never reaches Booking Service or Search Service.

### Decision

A scheduled job in Event Service runs every `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` (ADR-006) and publishes the `Scheduled` events whose visibility moment has passed and whose `published_at` is null. Each event is published in its own short transaction:

```sql
UPDATE events
SET published_at = now(), version = version + 1, updated_at = now()
WHERE id = (
    SELECT id FROM events
    WHERE status = 'Scheduled'
      AND published_at IS NULL
      AND sale_starts_at <= now() + make_interval(hours => 24 * :lead_days)
    ORDER BY sale_starts_at, id
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING *;
```

The same transaction writes `EventPublished` to the outbox, and the relay publishes it to the Kafka topic `events.lifecycle` (ADR-009). A run publishes at most `EVENT_PUBLISH_JOB_BATCH_SIZE` events. If it reached that number, the next run starts at once instead of waiting for the interval, so a backlog drains quickly.

- **The index can be used.** The lead time is added to `now()` instead of subtracted from the column, so the condition can use the partial index on `sale_starts_at` (`idx_events_pending_publish`).
- **A day is 24 hours.** The lead time is computed as `24 * EVENT_VISIBILITY_LEAD_DAYS` hours, and every database connection sets `TimeZone = 'UTC'`. A `days` interval added to a `timestamptz` follows the session time zone's daylight saving changes, which would move the visibility moment by an hour twice a year.
- **Several instances.** `FOR UPDATE SKIP LOCKED` lets jobs on several instances split the work without waiting on each other, and `published_at IS NULL` publishes each event once.

**Requests run the same check.** Scheduling an event (C4) and every accepted edit of a `Scheduled` event that is not published yet (C1, C2, C3) check visibility in their own transaction, right after their change, with the same condition on that one event. If the moment has passed, the request sets `published_at` and writes `EventPublished` itself, and the job skips the event. An event scheduled late, or whose sales window is moved earlier, is therefore published at once. The request is one change, so `version` goes up once (ADR-004).

**Publication is final.** Once `published_at` is set, it is never cleared, even if an edit moves the sales window or the show date further away than `EVENT_VISIBILITY_LEAD_DAYS`. Consumers already have the event, and withdrawing it would need a message type that does not exist.

**Late publication.** The job does not check whether sales have opened. If it was not running when an event's visibility moment and `sale_starts_at` both passed (an outage longer than the lead time), it publishes the event when it runs again, because Booking Service needs the event to sell tickets. This late `EventPublished` is the only message that can be sent after sales open.

### Consequences

- Other services see an event up to `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` plus `OUTBOX_RELAY_INTERVAL_MS` after its visibility moment, or within `OUTBOX_RELAY_INTERVAL_MS` when scheduling or an edit published it.
- A published event whose sales window moves later stays public for longer than `EVENT_VISIBILITY_LEAD_DAYS` before its new sale date.
- Consumers must accept an `EventPublished` whose sales have already opened.
- `EVENT_PUBLISH_JOB_BATCH_SIZE` (`100`) is a new variable (ADR-006).
- A new migration adds `published_at` (`timestamptz`) to `events`.
- Setting `published_at` only where it is still null publishes each event once, even if the job runs twice or concurrently.
- The job reads only Event Service's database: it calls neither Venue Service nor Auth Service and needs no token, which matters because Auth Service has no service role.
- Changing `EVENT_VISIBILITY_LEAD_DAYS` affects only unpublished events. This differs from the PRD criterion that a change affects every event on the next query.

### Alternatives considered

- **Publish at scheduling with a "visible from" date.** Rejected: events would propagate before their visibility moment, and Search Service would enforce a rule owned by Event Service.
- **Compute visibility only at query time.** Rejected: Booking Service and Search Service would never receive the event.

### Amendment - 2026-11-27

- **Query.** The first version wrote the condition as `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS <= now()`, which cannot use an index, with a `days` interval that shifts with daylight saving time, and had no batch limit. It now adds `24 * EVENT_VISIBILITY_LEAD_DAYS` hours to `now()`, connections run in UTC, each event gets its own transaction with `FOR UPDATE SKIP LOCKED`, and runs are capped by `EVENT_PUBLISH_JOB_BATCH_SIZE`.
- **Requests publish too.** Only scheduling published at once. The team decided that every edit of a scheduled, unpublished event also checks visibility, and that a published event stays published when its dates move later.
- **Late publication.** The contract said nothing is sent after sales open. An `EventPublished` delayed by a job outage is now an explicit exception.

## ADR-004 - Integer event version and full-state messages

| Status | Date | Capability |
|---|---|---|
| Proposed, amended 2026-11-27 | 2026-11-07 | C6 - Event propagation to other services |

### Context

`EventUpdated` carries the event version, and consumers discard versions they already have (C6). Keying `events.lifecycle` by event id keeps each event's messages in order (ADR-003), but Kafka delivers at least once, so a consumer can receive a message again after a crash, a relay retry or a replay. `events` has no version column today.

### Decision

`events` gets an integer `version`, incremented by one for every change to the event, in the same update that saves it: each accepted edit, scheduling (C4), and publication by the job (ADR-003). A request that changes the event and also publishes it (ADR-003) is one change and increments `version` once. A request blocked by the lock (ADR-002), or one that changes nothing (scheduling an event that is already `Scheduled`), leaves it unchanged. Two reads with the same `version` therefore always return the same event. Every `EventPublished` and `EventUpdated` carries the full event (show details, zone prices, sales window) and its version. Each consumer stores the last version applied per event and drops any message whose version is not higher.

#### Writing to the outbox

The outbox row is written in the same Postgres transaction that changes the event:

- **Edit:** `UPDATE events SET …, version = version + 1 WHERE id = :id AND (status = 'Draft' OR sale_starts_at > now()) RETURNING *` (lock condition from ADR-002). If the returned row has `published_at` set, insert `EventUpdated` built from it.
- **Publish inside a request:** after the edit's or scheduling's update, `UPDATE events SET published_at = now() WHERE id = :id AND status = 'Scheduled' AND published_at IS NULL AND sale_starts_at <= now() + make_interval(hours => 24 * :lead_days) RETURNING *`. If it returns a row, insert `EventPublished` built from it instead of `EventUpdated`. `version` is not incremented a second time.
- **Publish by the job:** `UPDATE events SET published_at = now(), version = version + 1 WHERE id = :id AND published_at IS NULL RETURNING *` (full query in ADR-003), then insert `EventPublished` built from it.

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

The relay publishes unsent rows in `id` order to `events.lifecycle`, keyed by `event_id`, then sets `sent_at`. It never reads `events`, so each message carries the state of its own version. Only one relay sends at a time, even when several instances of Event Service run ([ADR-009](#adr-009---one-active-outbox-relay-through-a-postgres-advisory-lock)).

### Consequences

- Repeated or replayed messages are dropped.
- The latest applied message always holds the complete state; no earlier message is needed.
- An edit racing the scheduled job is never left unpublished.
- Messages are larger than a list of changed fields.
- A new migration adds `version` and the outbox table.

### Alternatives considered

- **Order by `updated_at`.** Rejected: edits in the same millisecond tie, and clock skew (ADR-002) can give a later edit an earlier time.
- **Send only changed fields.** Rejected: a dropped or missed message would lose its change.

### Amendment - 2026-11-27

The first version incremented `version` only on edits; publishing left it unchanged. The team decided that every change to the event increments it, including scheduling and publication, so a `version` always identifies one state of the event, `published_at` included.

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
| Accepted, amended 2026-11-27 | 2026-11-15 | All |

### Context

The team agreed that no day count, duration or numeric limit is hardcoded. C6 already names `EVENT_VISIBILITY_LEAD_DAYS`, though `src/config.rs` does not read it yet. Other values were left open or written in code: ADR-003 ran the scheduled job "every X minutes", and `EventService::get_events` uses `10` and `100` for pagination.

### Decision

Each such value is an environment variable, read only in `src/config.rs` into `AppConfig` and passed to the code that uses it. The name ends with its unit (`_DAYS`, `_SECONDS`, `_MS`). Every variable has a default for local development, and Event Service refuses to start if a value is not a positive integer.

| Variable | Default | Used by |
|---|---|---|
| `EVENT_VISIBILITY_LEAD_DAYS` | `10` | C6: days before `sale_starts_at` when the event is published |
| `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` | `60` | ADR-003: how often the scheduled job looks for events to publish |
| `EVENT_PUBLISH_JOB_BATCH_SIZE` | `100` | ADR-003: events the job publishes per run before it starts the next run at once |
| `OUTBOX_RELAY_INTERVAL_MS` | `1000` | ADR-004: how often the relay reads unsent outbox rows |
| `OUTBOX_RELAY_BATCH_SIZE` | `100` | ADR-004: outbox rows the relay sends per read |
| `EVENTS_PAGE_SIZE_DEFAULT` | `10` | `per_page` when the request has none |
| `EVENTS_PAGE_SIZE_MAX` | `100` | Largest `per_page` accepted; larger values are clamped |
| `KAFKA_BROKERS` | `localhost:9092` | Kafka bootstrap servers |
| `KAFKA_EVENTS_TOPIC` | `events.lifecycle` | Topic for `EventPublished` and `EventUpdated` (C6) |

Timeouts, retries, circuit breakers and idempotency keys have their own variables, listed in ADR-008.

Rules that define the domain are not variables: a price greater than `0`, `page >= 1`, and the date order `now < sale_starts_at < sale_ends_at <= event_starts_at` (C3). Field length limits ([contract](contract-event-service.md#field-limits)) are not variables either: each matches a database column or check, so changing one needs a migration, not a configuration change.

### Consequences

- Operations can tune lead time, latency and limits without a release.
- A new variable needs a line in this table, in `.env.example` and in the AGENTS.md variable table.
- Tests build `AppConfig` with explicit values instead of relying on defaults.

### Alternatives considered

- **Constants in code.** Rejected by the team decision: a change needs a release.
- **Settings table in the database.** Rejected: no request needs to change them at runtime, and a table adds an admin endpoint to secure.

### Amendment - 2026-11-27

Adds `EVENT_PUBLISH_JOB_BATCH_SIZE` (ADR-003), and states that field length limits belong to the schema and the contract, not to configuration.

## ADR-007 - The backend validates every identity, role and session

| Status | Date | Capability |
|---|---|---|
| Accepted, amended 2026-11-16 and 2026-11-27 | 2026-11-15 | All |

### Context

The team agreed that a service never trusts identity or session data sent by the frontend. The PRD shows why: a reseller froze seat holds by killing the checkout page's JavaScript, because the countdown lived in the browser. In Event Service today, the dev token `mock:<partner-uuid>` lets the client name the partner it acts as, and `MOCK_AUTH=false` still accepts that format. The current public list also accepts `status=Draft`, which exposes drafts (C1).

### Decision

- **Identity.** The organizer comes only from a bearer token that Event Service verifies locally: `HS256` signature with the shared `JWT_SECRET` (no other algorithm accepted), `exp`, and a UUID `sub`. The issuer is checked once Auth Service adds an `iss` claim. `organizer_id` is never read from a body, query or header.
- **Role and ownership.** On every protected request the service layer checks that the token's `role` is `ORGANIZER` (compared without case, since Auth Service sends it in uppercase) and that the organizer owns the event. A partner that is not an organizer gets `403`. An event the caller does not own gets `404`, so its existence is not revealed.
- **Fields the server owns.** `id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at` and zone names are set by Event Service only. A request that sends one of them gets `400`.
- **Time.** Every time check uses the server clock (C3, C4) or the database clock (C5, ADR-002). The client's clock is never used.
- **Visibility.** An event that is not published yet (a draft, or scheduled before its visibility moment) can be read only by its owner: a valid `ORGANIZER` token whose `sub` is the event's `organizer_id`. A published event can be read by anyone. Public endpoints treat a missing, invalid or expired token, and a token that is not the owner's, as anonymous: they never answer `401` or `403`, and an unpublished event is `404` for them. Public lists return only published events, whatever filter the client sends (C1, C6).
- **No mock tokens.** Every environment, including local development and tests, verifies real `HS256` tokens. `MockTokenVerifier`, the `mock:<uuid>` format and `MOCK_AUTH` are removed. Tests sign tokens with a test secret; local development uses a token from Auth Service, or one signed with the local `JWT_SECRET` by a small helper binary.

### Consequences

- A client cannot act as another organizer, read another organizer's drafts or set server-owned state.
- Auth Service has defined its token (see [Auth Service](contract-event-service.md#auth-service) in the service contract), so a shared environment can satisfy this ADR once `MockTokenVerifier` is replaced. Auth Service has stored users in PostgreSQL since `594af49` (checked 2026-11-27), so `sub` no longer changes on restart and no longer blocks the first release.
- Event Service needs `JWT_SECRET` in every environment, including local development and tests, and refuses to start without it. In shared environments it is the secret shared with Auth Service and Venue Service. Any service holding it can sign tokens, so it is handled as a credential: never committed, and set per environment.
- Each protected endpoint needs tests for `401`, `403` and `404` on another organizer's event. `GET /api/v1/events/{id}` needs tests for the owner reading an unpublished event, and for a missing, invalid and non-owner token getting `404` on it.
- `TokenVerifier` has no mock implementation, an exception to the rule that every dependency has one. Tests use the real verifier with a test secret.

### Amendment - 2026-11-16

The first version checked the issuer and that "the partner exists". Reviewing the Auth Service contract (`TicketDSaster-Auth` `main`, `587250f`) showed that:

- The token is `HS256` with a shared secret, with claims `sub`, `role`, `iat` and `exp` only. There is no `iss` to check.
- Auth Service offers no endpoint or event to list or sync users, so the local `partners` table cannot be kept up to date. Checking it would reject every real organizer with `401`. The `partners` table and its foreign key on `events.organizer_id` are dropped (see [Data Model](../architecture/data-model.md)), and Event Service trusts `sub` and `role` from the verified token directly.

The decision now checks only what the token carries. It is otherwise unchanged.

### Amendment - 2026-11-27

- **Mock tokens removed.** The first version kept `mock:<uuid>` tokens for local development and tests. Once the `partners` table is dropped they carry no role, and keeping them would mean keeping a second, weaker way in. The team decided to remove them: every environment uses real `HS256` tokens.
- **Auth Service persistence.** Auth Service now stores users in PostgreSQL (`TicketDSaster-Auth` `main`, `9e1fcf4`). The token is unchanged: `HS256`, `{ sub, role }` plus `iat` and `exp`, 8 hours, no refresh.
- **Reading unpublished events.** The contract and the brief disagreed on whether the owner can read an unpublished event by id, and an invalid token on a public endpoint was undefined. The owner can read it; everyone else, including callers with a missing or invalid token, is treated as anonymous and gets `404`.

### Alternatives considered

- **Trust an `organizer_id` sent by the frontend after login.** Rejected: any client can send any id.
- **Ignore server-owned fields silently.** Rejected: a `400` shows the client what it got wrong, and a silently dropped field hides the client bug.

## ADR-008 - Timeouts, retries, idempotency and circuit breakers on every call

| Status | Date | Capability |
|---|---|---|
| Accepted, amended 2026-11-16 and 2026-11-27 | 2026-11-15 | All |

### Context

The team agreed that every call to another service, and every call into ours, uses resiliency methods: a timeout, idempotency where a repeat could do harm, retries, and a circuit breaker where it makes sense. The PRD expects 100,000 fans at once on a hot sale, so a slow dependency must not hold requests open until Event Service runs out of connections.

Today the only protection is a hardcoded 3-second timeout in `HttpVenueClient`. Incoming requests have no timeout, `POST /api/v1/events` creates a second draft when a client retries it, and a Venue Service failure becomes a `500`.

### Decision

Each call gets the methods that fit it. All numbers are environment variables (ADR-006).

| Call | Timeout | Retry | Idempotency | Circuit breaker |
|---|---|---|---|---|
| **Out:** Venue Service (C2) | Per attempt | Yes | Reads only, safe to repeat | Yes |
| **Out:** Kafka, outbox relay (ADR-004) | Per message | Yes, on the next relay run | Idempotent producer, versioned messages | No: the outbox absorbs the outage |
| **Out:** PostgreSQL | Pool acquire and statement | No retry inside a request | Transactions | No |
| **In:** HTTP API | Per request | Clients may retry | `Idempotency-Key` on `POST /api/v1/events`; other edits are idempotent | No |

#### Calls to other services

- **Timeouts.** Every attempt has its own timeout, and the attempts of one incoming request together never exceed `HTTP_REQUEST_TIMEOUT_MS`. When the remaining time is shorter than one attempt, Event Service stops retrying.
- **Retries.** Only on a connection error, a timeout, `429`, `502`, `503` or `504`; never on another `4xx`. Exponential backoff with full jitter: the wait before retry `n` is a random value between 0 and `RETRY_BASE_DELAY_MS * 2^n`, capped at `RETRY_MAX_DELAY_MS`. A `Retry-After` header from the other service is respected.
- **Circuit breaker.** One breaker per service. After `CIRCUIT_BREAKER_FAILURE_THRESHOLD` consecutive failed calls (after retries) it opens, and calls fail at once for `CIRCUIT_BREAKER_OPEN_SECONDS`. Then a single trial call is allowed: success closes the breaker, failure opens it again. 4xx responses other than `429` do not count as failures.
- **What the caller sees.** When Venue Service cannot answer, Event Service returns `503 Service Unavailable` with `Retry-After` (see [Retry-After](#retry-after)), never `500`. A `401` or `403` from Venue Service is not passed on: Event Service has already verified the same token with the same secret, so it means the two services disagree (secret or role names). It is logged as a configuration error and returned as `503`, and it is not retried.
- **Auth Service is not called.** Tokens are verified locally with the shared secret (ADR-007), so an Auth Service outage never causes `401` or `503`, and organizers are not logged out by it.

#### Kafka

- The producer is idempotent (`enable.idempotence=true`, `acks=all`), so a producer retry neither duplicates nor reorders messages within a partition. Each send waits at most `KAFKA_MESSAGE_TIMEOUT_MS`.
- The relay sets `sent_at` only after Kafka confirms the message. A failed message stays unsent, and the relay stops its batch at that row so later rows of the same event never go first.
- After a failure the relay waits twice as long before each new attempt, starting at `OUTBOX_RELAY_INTERVAL_MS` and capped at `OUTBOX_RELAY_MAX_BACKOFF_MS`, and returns to the normal interval after a success.
- No circuit breaker: organizers keep editing and scheduling while Kafka is down, and messages are sent from the outbox when it returns. Consumers already drop repeats by `version` (ADR-004).

#### PostgreSQL

- Getting a pooled connection waits at most `DB_POOL_ACQUIRE_TIMEOUT_MS`, and every statement runs with `statement_timeout = DB_STATEMENT_TIMEOUT_MS`. Either one failing returns `503` with `Retry-After`.
- A request never retries its transaction. The scheduled job (ADR-003) and the relay retry naturally on their next run.

#### Incoming requests

- **Timeout.** Each request is cut at `HTTP_REQUEST_TIMEOUT_MS` and gets `503` with `Retry-After` and the usual `{"error": "..."}` body. An Axum middleware (`axum::middleware::from_fn`) runs the rest of the request under `tokio::time::timeout`. When the time runs out, the handler's future is dropped, its transaction rolls back, and the middleware answers `503` itself. `tower_http::timeout::TimeoutLayer` is not used, because it answers `408` with an empty body.
- **Request deadline.** The same middleware stores the request's deadline (start plus `HTTP_REQUEST_TIMEOUT_MS`) in a `tokio::task_local!` value. The Venue Service client reads it to stop retrying when the time left is shorter than one attempt (see *Timeouts* above), without passing the deadline through every function.
- **`POST /api/v1/events`** requires an `Idempotency-Key` header (a UUID created by the client for each new event form). Event Service stores the key, the organizer and the response for `IDEMPOTENCY_KEY_TTL_HOURS`:
  - Same key and body from the same organizer: the stored response is returned and no second draft is created.
  - Same key with a different body: `422`.
  - Same key while the first request is still running: the repeat waits for the first to finish, then returns its stored response. If the first request failed and rolled back, the repeat creates the draft itself. The wait counts toward `DB_STATEMENT_TIMEOUT_MS`; if it runs out, the repeat gets `503` with `Retry-After`.
  - Missing header: `400`.
  - Keys are scoped to the organizer from the token, so one organizer cannot read another's stored response (ADR-007).
  - The body is compared by the SHA-256 hash of the request as parsed (the DTO serialized back to JSON), so whitespace and key order do not count as a different body.
  - **How.** The key row is written in the same transaction as the draft. The transaction inserts the key first (`INSERT ... ON CONFLICT DO NOTHING`), creates the draft, stores the response in the key row and commits. Until the commit, no other request can see the key. A second insert of the same key waits for that transaction, as Postgres does for any unique conflict. A request therefore never sees a key without its response, and a crash leaves no key behind that would block retries.
- **`PATCH`, `PUT` and `POST /{id}/schedule`** are idempotent by design: repeating one leaves the event in the same state. Scheduling an event that is already `Scheduled` returns `200` with the event instead of an error, until sales open; after that it gets `409` like any edit (ADR-002). A repeated edit may increase `version` and send an `EventUpdated` with the same content, which consumers apply as a normal upsert.
- **Clients** may retry `GET`, `PUT`, `PATCH`, the schedule call and `POST` with the same `Idempotency-Key`, on a timeout, `429` or `503`, with backoff and respecting `Retry-After`.

#### Retry-After

Every `503` carries `Retry-After` in whole seconds:

- While the Venue Service breaker is open: the time left until its trial call, rounded up, at least `1`.
- In every other case (request timeout, retries exhausted, Venue Service `401`/`403`, database pool or statement timeout): `HTTP_RETRY_AFTER_SECONDS`.

`AppError::ServiceUnavailable` carries an optional value. The breaker sets one; other sources leave it empty, including database errors converted with `From<DbErr>`, which cannot read the configuration. The timeout middleware wraps every request, so it also adds `Retry-After: HTTP_RETRY_AFTER_SECONDS` to any `503` response that does not have the header yet.

| Variable | Default | Used for |
|---|---|---|
| `HTTP_REQUEST_TIMEOUT_MS` | `10000` | Total time for one incoming request |
| `HTTP_RETRY_AFTER_SECONDS` | `2` | `Retry-After` on a `503` when no better value is known |
| `VENUE_SERVICE_TIMEOUT_MS` | `2000` | One attempt to Venue Service (replaces the hardcoded 3 seconds) |
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
- `AppError` gains a `ServiceUnavailable` variant (`503`, with an optional `Retry-After` that the timeout middleware fills in when empty) and a `Conflict` variant (`409`, event locked).
- A new migration adds an `idempotency_keys` table (`key`, `organizer_id`, request hash, response status and body, `created_at`), and the scheduled job deletes rows older than `IDEMPOTENCY_KEY_TTL_HOURS`.
- A repeated `POST /api/v1/events` holds a database connection while it waits for the first request. The wait is bounded by `DB_STATEMENT_TIMEOUT_MS`, and repeats of one form are rare.
- Breaker state lives in each instance's memory, so with several instances each opens on its own. This is enough for one dependency per breaker.
- Outgoing timeouts and retries use `reqwest` and `tokio`, already dependencies. The incoming timeout is an Axum middleware over `tokio::time::timeout`, so it needs no `tower-http` feature. The circuit breaker is a small state machine behind the `VenueClient` trait, with no new crate. `TokenVerifier` makes no network call and needs none. The Kafka producer crate is needed for C6 anyway and is named in its proposal.
- Each behavior needs tests with the mocks: retry then success, retries exhausted to `503`, breaker open to immediate `503`, and a repeated `Idempotency-Key`.

### Alternatives considered

- **Retries without a breaker.** Rejected: during a Venue Service outage every request would wait for all its attempts and multiply the load on the failing service.
- **A breaker on Kafka.** Rejected: the outbox already decouples requests from Kafka, so failing fast would protect nothing.
- **Make `POST /api/v1/events` idempotent by event name.** Rejected: an organizer can create two drafts with the same name on purpose.
- **Shared breaker state in Redis.** Rejected for this release: it adds a dependency that itself needs a breaker.

### Amendment - 2026-11-16

The first version had an outgoing call to fetch and cache Auth Service verification keys, with `AUTH_SERVICE_TIMEOUT_MS` (`2000`) and `AUTH_KEYS_CACHE_SECONDS` (`300`). The Auth Service contract (`TicketDSaster-Auth` `main`, `587250f`) signs tokens with `HS256` and a shared secret, and `GET /.well-known/jwks.json` returns no key, so there is nothing to fetch. The row, the cache rule and both variables are removed, and Event Service makes no call to Auth Service. If Auth Service moves to asymmetric keys published as a JWKS, that call returns with a timeout, retries, a breaker and a key cache.

The amendment also defines what a `401` or `403` from Venue Service means (see *What the caller sees*), since Venue Service requires the organizer's token on every endpoint.

### Amendment - 2026-11-27

Three gaps found when checking the design against Axum and Postgres:

- **Idempotency key in progress.** The first version returned `409` while the first request with the same key was still running. If the key is written in the same transaction as the draft, other requests cannot see it until it commits: Postgres makes them wait, so `409` never happens. If the key were committed first in its own transaction, a crash would leave a key without a response, and every retry would get `409` until the key expired. The team chose the single transaction and dropped `409`: a repeat now waits and returns the first response (see *Incoming requests*). `409` now means only "event locked".
- **`Retry-After` value.** The first version required `Retry-After` but gave no value, and hardcoding one would break ADR-006. The breaker's remaining open time is used when it applies, and the new variable `HTTP_RETRY_AFTER_SECONDS` (`2`) covers every other case (see [Retry-After](#retry-after)).
- **Request timeout.** The first version named the `timeout` feature of `tower-http`, whose `TimeoutLayer` answers `408` with an empty body, not `503` with `Retry-After` and a JSON body. It is replaced by an Axum middleware over `tokio::time::timeout`, which also stores the request deadline that the Venue Service retries need.

## ADR-009 - One active outbox relay through a Postgres advisory lock

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-27 | C6 - Event propagation to other services |

### Context

The outbox relay (ADR-004) sends unsent outbox rows to `events.lifecycle` in `id` order. Consumers rely on the messages of one event arriving in version order. Kafka keeps that order only within one producer. More than one instance of Event Service can be running: when it is scaled out, and for a moment during every rolling deploy. If every instance runs a relay, two relays can read the same unsent rows. Duplicates are harmless, because consumers drop repeated versions. Reordering is not: one relay can write version 5 of an event to the partition before another relay writes version 4. A consumer that follows the version rule keeps the right state, but:

- With log compaction (planned for `events.lifecycle`), Kafka keeps the last message written for each key. That would be version 4, so a new consumer rebuilding from the topic gets stale state.
- If an `EventUpdated` lands before the `EventPublished` of the same event, consumers drop the `EventPublished` as an older version, and its one-time side effect (announcing the event) never happens.
- The contract promises consumers that each event's messages arrive in order.

### Decision

Every instance starts the relay, but only one sends at a time. Each relay run is one Postgres transaction that starts with `SELECT pg_try_advisory_xact_lock(<relay lock id>)`:

- **Lock not taken.** Another instance is relaying. The transaction ends without reading anything, and the relay tries again after `OUTBOX_RELAY_INTERVAL_MS`.
- **Lock taken.** The relay reads up to `OUTBOX_RELAY_BATCH_SIZE` unsent rows in `id` order and sends them in that order, waiting for Kafka to confirm each one. It then sets `sent_at` on the confirmed rows and commits. If a row fails, it commits the rows confirmed before it and stops the batch there (ADR-008).

The lock belongs to the transaction, so Postgres releases it at commit or rollback, and also when the instance dies and its connection closes. No instance has to be elected leader, and two relays can never send at once. The lock id is a fixed `bigint` constant in the code. It is an identifier, not a duration or limit, so ADR-006 does not apply to it.

The publish job (ADR-003) does not take this lock. Its guarded update (`published_at IS NULL`) publishes each event once however many instances run it. It selects candidate events with `FOR UPDATE SKIP LOCKED`, so concurrent runs split the work instead of waiting on each other.

### Consequences

- Messages of one event reach Kafka in version order, with any number of instances.
- All sending happens in one instance at a time. `events.lifecycle` carries one message per accepted edit of a public event, not per ticket, so one relay is enough.
- A relay run keeps a transaction and a pooled connection open while Kafka confirms the batch, bounded by `KAFKA_MESSAGE_TIMEOUT_MS`.
- If an instance dies after Kafka confirmed a row but before the commit, the next relay sends the row again. Consumers drop it by `version` (ADR-004).
- The relay may see a row with a higher `id` before a lower one commits, because ids are assigned at insert, not at commit. Those rows always belong to different events: writers of the same event wait on its `events` row (ADR-004), so per-event order holds.
- The relay needs no new crate and no new variable.

### Alternatives considered

- **Session-level advisory lock held by a leader.** Rejected: one connection stays pinned for good. And if that connection drops while the leader is in the middle of a batch, a second leader can start before the first one notices, so two relays would overlap.
- **`SELECT ... FOR UPDATE SKIP LOCKED` on outbox rows.** Rejected: two relays would send different rows of the same event at the same time, which is exactly the reordering to avoid.
- **Assign events to instances by hashing the event id.** Rejected: each instance would need to know how many instances are running, which a rolling deploy changes.
- **Leader election outside the database (Kubernetes lease, ZooKeeper).** Rejected: it adds infrastructure that Postgres already covers.
- **Run the relay as a separate single-replica deployment.** Rejected: nothing would enforce the single replica, and a deploy briefly runs two anyway.

## ADR-010 - Malformed requests get a JSON 400 and times are parsed in the service

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-27 | All |

### Context

The contract says `400` means a malformed request (invalid JSON, wrong type, bad UUID, server-owned field) and `422` means a well-formed request that breaks a rule, such as a time without a timezone offset. Every error uses the body `{"error": "..."}`. Axum 0.7's built-in extractors do not behave that way:

- `axum::Json` answers `400` for invalid JSON, `422` for a wrong type or missing field, and `415` for a missing `Content-Type`, with a plain-text body.
- `axum::extract::Path` and `Query` answer `400` with a plain-text body.
- `chrono`'s `DateTime<Utc>` deserializer rejects a time without an offset as a type error. Axum turns that into `422`, but with a plain-text body, and from the same code path that sends every other type error to `422`. The two cases cannot be told apart.

### Decision

- **Custom extractors.** `src/api/extract.rs` defines `ApiJson<T>`, `ApiPath<T>` and `ApiQuery<T>` with Axum's derive macros: `#[derive(FromRequest)]` with `#[from_request(via(axum::Json), rejection(AppError))]`, and `#[derive(FromRequestParts)]` with `via(axum::extract::Path)` or `via(axum::extract::Query)`. `AppError` implements `From<JsonRejection>`, `From<PathRejection>` and `From<QueryRejection>`, and every rejection becomes `AppError::BadRequest` (`400`, JSON body) with the rejection's message. Handlers use these extractors, never Axum's directly. `ApiJson` also implements `IntoResponse`, so handlers can return it.
- **Times as strings.** Request DTOs carry times as `RequestTime`, a `#[serde(transparent)]` wrapper around `String`. A JSON value that is not a string fails while parsing the body, so it gets `400`. The service converts each `RequestTime`:
  - RFC 3339 with an offset (`DateTime::parse_from_rfc3339`): accepted and converted to UTC.
  - A valid date and time without an offset (`NaiveDateTime`): `422`, "time must include a timezone offset".
  - Anything else: `400`, malformed time.
- **Responses** keep `DateTime<Utc>` and are always written in UTC.

### Consequences

- Every error, including a body, path or query the service cannot parse, has the `{"error": "..."}` shape and the status the contract names.
- A missing `Content-Type` gets `400`, not `415`.
- The offset check lives in the service, so the service tests cover it without HTTP.
- Axum needs its `macros` feature. No new crate.
- Each endpoint needs tests for an unparseable body or parameter (`400`, JSON body) and, where it takes times, a time without an offset (`422`).

### Alternatives considered

- **`axum_extra::extract::WithRejection`.** Rejected: it adds a crate and puts a wrapper type in every handler signature, for the same result as the derive.
- **A middleware that rewrites plain-text error responses.** Rejected: it would have to guess the cause from the text, and it loses the difference between `400` and `422`.
- **A custom `DateTime` deserializer that accepts times without an offset.** Rejected: the error would still come from serde as a type error, so it could not become `422`.
- **Keep Axum's defaults and document them.** Rejected: clients would have to handle two error formats, and the status codes would break the contract.

## ADR-011 - Tests run against real PostgreSQL and real tokens, without in-memory mocks

| Status | Date | Capability |
|---|---|---|
| Accepted | 2026-11-27 | All |

### Context

The guarantees that matter most live in SQL, not in Rust:

- The lock is a conditional `UPDATE ... WHERE status = 'Draft' OR sale_starts_at > now()` (C5, ADR-002).
- Whether an edit emits `EventUpdated` or `EventPublished` comes from its own `RETURNING` (ADR-003, ADR-004).
- The check constraints, the `Idempotency-Key` wait on a unique conflict (ADR-008), `FOR UPDATE SKIP LOCKED` in the publish job and the relay's advisory lock (ADR-009).

Today the tests run on `MockEventRepository`, an in-memory `HashMap` that re-implements that behavior in Rust. A test that passes on it proves the mock right, not the SQL. `MockTokenVerifier` accepts a token format that no environment will use (ADR-007). Auth Service now has a stable token contract and persistent users, so real tokens can be used everywhere.

### Decision

- **No in-memory mocks of Event Service's own parts.** `MockEventRepository`, `MockPartnerRepository` and `MockTokenVerifier` are removed. Services use the SeaORM repository directly; a repository trait is kept only where a second real implementation exists.
- **Tests use a real PostgreSQL** at `TEST_DATABASE_URL` (the `event-db` container from `docker compose up -d event-db`). The test harness migrates a template database once per test run. Each test then gets its own database (`CREATE DATABASE ... TEMPLATE ...`), which it drops at the end, so tests are isolated and run in parallel. API tests still build the router with `create_test_app` and send requests with `tower::ServiceExt::oneshot`.
- **Tests use real tokens.** A helper in `tests/common` signs `HS256` tokens with a test secret using the same crate as the verifier. It also makes the tokens for `401` tests: expired, wrong secret, wrong algorithm, missing `sub`.
- **Time comes from real clocks.** Tests set times relative to now. To test a locked event, they write a past `sale_starts_at` straight into the database. The lock uses the database clock, which no Rust trait could fake, so there is no `Clock` trait.
- **Kafka is real too.** Relay tests run against the Kafka broker in `docker-compose.yml`, at `KAFKA_BROKERS`.
- **Venue Service is the one exception, for now.** `VenueClient` keeps a fake implementation for tests and for local development (`MOCK_VENUE_SERVICE`). Venue Service has exposed zones through its seat maps since 2026-11-27, so the fake is removed with the venue pricing work; tests then call a Venue Service container, or a stub HTTP server that serves recorded Venue Service responses.
- **CI** runs PostgreSQL and Kafka as service containers in `.github/workflows/validation.yml`. That file is protected (AGENTS.md), so the change needs its own ticket.

### Consequences

- The tests check the SQL that carries C5 and C6, including the races, against the same Postgres version as production.
- `cargo test` needs Docker running `event-db` (and Kafka for the relay tests). Contributors and CI both need it.
- Tests are slower than in memory. A database copied from a template takes tens of milliseconds, and tests run in parallel.
- There is no mock to keep in step with the real code.
- `TEST_DATABASE_URL` is a new variable for tests only. It is not read by `src/config.rs` and not used by the running service.

### Alternatives considered

- **Keep the mocks and add opt-in database tests.** Rejected: two suites that can disagree, and the default suite would still not cover the SQL.
- **`testcontainers`.** Rejected: it adds a crate, starts containers from inside the tests and needs the Docker socket in CI. The compose database already exists.
- **One shared database, each test inside a transaction that rolls back.** Rejected: the code under test opens and commits its own transactions, and the lock and idempotency tests need two transactions at once.
- **A `Clock` trait to fake time.** Rejected: the lock and the publish condition use the database clock, so a Rust clock would fake only part of the time.
