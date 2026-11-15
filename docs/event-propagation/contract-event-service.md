# Service contract - Event Service

| Status | Date | Source |
|---|---|---|
| Proposed | 2026-11-15 | [Capabilities C1-C6](capabilities-and-design-event-propagation.md), [ADR-001 to ADR-008](adrs-event-propagation.md) |

This is what Event Service offers other services: HTTP endpoints to read and build events, and the Kafka topic `events.lifecycle` that announces public events. It is the target contract. The current `openapi.yml` describes the code on `main` and differs from it (see [Changes from the current API](#changes-from-the-current-api)). When an endpoint is built, `openapi.yml` is regenerated from code and becomes the reference for that endpoint.

| Consumer | Uses |
|---|---|
| Frontend (fans) | Public endpoints |
| Frontend (organizers) | Organizer endpoints |
| Booking Service | Kafka, consumer group `booking-service`; public endpoints to recover state |
| Search Service | Kafka, consumer group `search-service` |

---

## Conventions

| Topic | Rule |
|---|---|
| Base path | `/api/v1/events`. `/events` is a compatibility alias and gets no new routes. |
| Format | JSON, UTF-8, `snake_case` field names. |
| Identifiers | UUID strings. |
| Times | RFC 3339. Requests must include an offset (`2026-12-01T10:00:00-06:00`); a time without one gets `422`. Responses and messages are always UTC (`2026-12-01T16:00:00Z`). |
| Money | Integer thousandths of a US dollar: `1000` is $1.00, `65500` is $65.50. Never a string or float. Responses carry `"currency": "USD"`. See [ADR-005](adrs-event-propagation.md#adr-005---prices-in-us-dollars-as-integer-thousandths). |
| Errors | `{"error": "<message>"}`. Read the category from the HTTP status, not the message. `400` means the request is malformed (invalid JSON, wrong type, bad UUID, a server-owned field); `422` means it is well formed but breaks a rule. Internal and database details are never returned. |
| Authentication | `Authorization: Bearer <token>`, verified by Event Service on every protected request. Identity, role and ownership are never taken from the request. See [ADR-007](adrs-event-propagation.md#adr-007---the-backend-validates-every-identity-role-and-session). |
| Resiliency | Every request is cut at `HTTP_REQUEST_TIMEOUT_MS`. When Event Service or a service it depends on cannot answer, the response is `503` with `Retry-After`. Clients may retry with backoff; see [Retries and idempotency](#retries-and-idempotency) and [ADR-008](adrs-event-propagation.md#adr-008---timeouts-retries-idempotency-and-circuit-breakers-on-every-call). |
| Limits and durations | Read from environment variables. See [ADR-006](adrs-event-propagation.md#adr-006---every-time-duration-and-numeric-limit-comes-from-an-environment-variable). |

---

## Event lifecycle

| Stage | When | `status` | Visible to |
|---|---|---|---|
| Draft | Created (C1) | `Draft` | Owning organizer only |
| Scheduled | Organizer schedules it and all checks pass (C4) | `Scheduled` | Owning organizer only |
| Published | `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS` (C6) | `Scheduled`, `published_at` set | Everyone; `EventPublished` sent |
| Locked | `sale_starts_at` (C5) | `Scheduled` | Everyone; no more edits or messages |

An event is locked when the current time is at or after `sales_window.starts_at`; there is no separate field. Cancellation, sold out and completion are not designed yet, and no `status` other than `Draft` and `Scheduled` is returned.

---

## Event object

The same object is returned by `GET /api/v1/events/{id}` and carried in every Kafka message. On a published event every field except those marked optional is set.

| Field | Type | Description |
|---|---|---|
| `id` | UUID | Event id. Kafka message key. |
| `version` | integer | Starts at `1` and increases by 1 with each accepted edit ([ADR-004](adrs-event-propagation.md#adr-004---integer-event-version-and-full-state-messages)). |
| `status` | `Draft` \| `Scheduled` | See [Event lifecycle](#event-lifecycle). |
| `organizer_id` | UUID | Organizer that owns the event, taken from the token at creation. |
| `name` | string | Event name. |
| `artist` | string | Headliner. Null only on a draft. |
| `tour` | string, optional | Tour name. |
| `support_acts` | string[] | Opening acts in billing order. Empty when none. |
| `event_starts_at` | UTC time | When the show starts. Null only on a draft. |
| `doors_open_at` | UTC time, optional | When doors open. Not later than `event_starts_at`. |
| `description` | string, optional | Free text. |
| `poster_url` | string, optional | Poster image URL. |
| `venue_id` | UUID | Venue in Venue Service. Null only on a draft. |
| `currency` | `USD` | Always `USD`. |
| `zones` | Zone[] | One per zone of the venue, snapshotted when the venue was saved ([ADR-001](adrs-event-propagation.md#adr-001---soft-delete-zone-prices-when-the-venue-changes)). |
| `sales_window` | object | `starts_at` and `ends_at`, both UTC. Null only on a draft. |
| `published_at` | UTC time | When the event became public. Null until published. |
| `created_at`, `updated_at` | UTC time | Record timestamps. |

**Zone**

| Field | Type | Description |
|---|---|---|
| `venue_zone_id` | UUID | Zone id in Venue Service. |
| `name` | string | Zone name copied from Venue Service when the venue was saved. |
| `price` | integer | Thousandths of a US dollar, greater than `0`. Null only on a draft. |

Example of a published event:

```json
{
  "id": "6f1c2a9e-3b7d-4c55-9a1e-2d4f8b6c0a11",
  "version": 4,
  "status": "Scheduled",
  "organizer_id": "11111111-1111-1111-1111-111111111111",
  "name": "Fuerza Regida - Merida",
  "artist": "Fuerza Regida",
  "tour": "Pero No Te Enamores Tour",
  "support_acts": ["Grupo Frontera"],
  "event_starts_at": "2026-12-20T03:00:00Z",
  "doors_open_at": "2026-12-20T01:00:00Z",
  "description": "One night only.",
  "poster_url": "https://cdn.example.com/posters/6f1c2a9e.jpg",
  "venue_id": "33333333-3333-3333-3333-333333333333",
  "currency": "USD",
  "zones": [
    { "venue_zone_id": "a1d6c7e0-0001-4b1a-9c11-000000000001", "name": "Pit", "price": 65000 },
    { "venue_zone_id": "a1d6c7e0-0002-4b1a-9c11-000000000002", "name": "Lower Tier", "price": 45500 },
    { "venue_zone_id": "a1d6c7e0-0003-4b1a-9c11-000000000003", "name": "Upper Tier", "price": 30000 }
  ],
  "sales_window": {
    "starts_at": "2026-12-01T16:00:00Z",
    "ends_at": "2026-12-19T23:00:00Z"
  },
  "published_at": "2026-11-21T16:00:42Z",
  "created_at": "2026-11-10T18:22:05Z",
  "updated_at": "2026-11-25T09:14:31Z"
}
```

---

## HTTP endpoints

### Public endpoints

No token required. They return published events only, whatever filters the client sends.

#### `GET /api/v1/events`

List published events, soonest `event_starts_at` first.

| Query | Description |
|---|---|
| `page` | Starts at 1. Default 1; `0` is treated as 1. |
| `per_page` | Default `EVENTS_PAGE_SIZE_DEFAULT` (10), clamped to 1 through `EVENTS_PAGE_SIZE_MAX` (100). |
| `venue_id` | Only events at this venue. |
| `organizer_id` | Only events by this organizer. |

Response `200`:

```json
{
  "items": [
    {
      "id": "6f1c2a9e-3b7d-4c55-9a1e-2d4f8b6c0a11",
      "name": "Fuerza Regida - Merida",
      "artist": "Fuerza Regida",
      "event_starts_at": "2026-12-20T03:00:00Z",
      "venue_id": "33333333-3333-3333-3333-333333333333",
      "poster_url": "https://cdn.example.com/posters/6f1c2a9e.jpg",
      "min_price": 30000,
      "currency": "USD",
      "sales_window": { "starts_at": "2026-12-01T16:00:00Z", "ends_at": "2026-12-19T23:00:00Z" }
    }
  ],
  "page": 1,
  "per_page": 10,
  "total_items": 1,
  "total_pages": 1
}
```

Errors: `400` malformed query value, `503` timeout or database unavailable, `500`.

#### `GET /api/v1/events/{id}`

Return one [event object](#event-object). Drafts and unpublished events return `404`, unless the request carries a verified token of the owning organizer.

Errors: `400` malformed id, `404` not found or not visible to the caller, `503` timeout or database unavailable, `500`.

### Organizer endpoints

All require a bearer token. Event Service resolves the partner from the verified token and checks the role and ownership on every request.

| Status | Meaning on every organizer endpoint |
|---|---|
| `401` | Token missing, invalid or expired, or partner unknown. |
| `403` | Partner is not an organizer. |
| `404` | Event does not exist or belongs to another organizer. |
| `409` | Event locked: the event is `Scheduled` and `sales_window.starts_at` has passed (C5). Any edit endpoint. |
| `503` | Request timed out, or Auth Service, Venue Service or the database cannot answer. Carries `Retry-After`. An Auth Service outage is `503`, never `401`. |

A request that sends a field Event Service owns (`id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at`, zone `name`) gets `400`. Each successful edit returns `200` with the full event object and its new `version`. Edits to a `Scheduled` event must keep every C4 check true, or they get `422`.

| Method and path | Capability | Body | Success | Specific errors |
|---|---|---|---|---|
| `GET /api/v1/events/mine` | - | Query `page`, `per_page` | `200`, page of the caller's events in any state. The organizer comes from the token; there is no `organizer_id` parameter. | - |
| `POST /api/v1/events` | C1 | Header `Idempotency-Key` (UUID, required). `name` (required) and any show details. | `201`, event with `status: Draft`, `version: 1`. A repeat with the same key returns the first response. | `400` missing `Idempotency-Key`; `409` same key still in progress; `422` empty name, time without offset, or same key with a different body |
| `PATCH /api/v1/events/{id}` | C1 | Any of `name`, `artist`, `tour`, `support_acts`, `event_starts_at`, `doors_open_at`, `description`, `poster_url`. Omitted fields stay; `null` clears an optional field. | `200` | `422` empty name, time without offset, or `doors_open_at` after `event_starts_at` |
| `PUT /api/v1/events/{id}/venue` | C2 | `venue_id` and `zones: [{ "venue_zone_id", "price" }]`. A draft may omit prices. | `200`, zones snapshotted from Venue Service | `422` price not greater than 0, venue does not exist, or a zone is not in the venue |
| `PUT /api/v1/events/{id}/sales-window` | C3 | `starts_at`, `ends_at` | `200` | `422` breaks `now < starts_at < ends_at <= event_starts_at` (server time) |
| `POST /api/v1/events/{id}/schedule` | C4 | None | `200`, `status: Scheduled`; `published_at` set if the visibility moment has passed. Scheduling an event that is already `Scheduled` returns `200` with the event. | `422` a check failed (including sales open already passed), with the first failing check in `error` |

### Retries and idempotency

| Request | Safe to retry | How |
|---|---|---|
| `GET` | Yes | Repeat as is. |
| `PATCH`, `PUT` | Yes | Repeating sets the same state. It may increase `version` and send an `EventUpdated` with the same content. |
| `POST /api/v1/events/{id}/schedule` | Yes | An event already `Scheduled` returns `200`. |
| `POST /api/v1/events` | Only with the same `Idempotency-Key` | Create one UUID per new event form and reuse it on every retry of that form. Keys expire after `IDEMPOTENCY_KEY_TTL_HOURS` (24). |

Retry only on a timeout, `429` or `503`, with exponential backoff and jitter, and wait at least `Retry-After` when present. Do not retry other `4xx` responses.

Example `PUT /api/v1/events/{id}/venue` body:

```json
{
  "venue_id": "33333333-3333-3333-3333-333333333333",
  "zones": [
    { "venue_zone_id": "a1d6c7e0-0001-4b1a-9c11-000000000001", "price": 65000 },
    { "venue_zone_id": "a1d6c7e0-0002-4b1a-9c11-000000000002", "price": 45500 },
    { "venue_zone_id": "a1d6c7e0-0003-4b1a-9c11-000000000003", "price": 30000 }
  ]
}
```

---

## Kafka topic `events.lifecycle`

| Property | Value |
|---|---|
| Topic | `events.lifecycle` (set by `KAFKA_EVENTS_TOPIC`) |
| Key | Event `id` as a UUID string |
| Value | JSON, UTF-8 |
| Producer | Event Service only, through its outbox ([ADR-004](adrs-event-propagation.md#adr-004---integer-event-version-and-full-state-messages)) |
| Delivery | At least once, from an idempotent producer (`acks=all`). Ordered per event, since all messages for one event share a key and partition. While Kafka is down, messages wait in the outbox and are sent in order when it returns. |
| Consumer groups | `booking-service`, `search-service` |

### Message

```json
{
  "type": "EventUpdated",
  "event_id": "6f1c2a9e-3b7d-4c55-9a1e-2d4f8b6c0a11",
  "version": 4,
  "occurred_at": "2026-11-25T09:14:31Z",
  "event": { "...": "full event object, same as GET /api/v1/events/{id}" }
}
```

| Field | Type | Description |
|---|---|---|
| `type` | `EventPublished` \| `EventUpdated` | Why the message was sent. |
| `event_id` | UUID | Same as the key and `event.id`. |
| `version` | integer | Same as `event.version`. |
| `occurred_at` | UTC time | When the change was committed in Event Service. |
| `event` | [Event object](#event-object) | Full state at that version. Never a draft or unpublished event. |

| Type | Sent when | `version` |
|---|---|---|
| `EventPublished` | Once per event, at `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS`, or right away when the event is scheduled after that moment (C6). | Unchanged by publishing |
| `EventUpdated` | After each accepted edit of a published event, until sales open (C5). | Higher than any earlier message for the event |

No messages are sent for drafts, for scheduled events before their visibility moment, or after sales open.

### Consumer rules

1. Store the last `version` applied for each `event_id`.
2. Drop a message whose `version` is less than or equal to the stored one. It is a repeat or a replay.
3. Otherwise replace your whole copy of the event with `event`. Treat both types as an upsert: every message holds the complete state, so a consumer that missed earlier messages is still correct after the next one.
4. Do not rely on `type` for state. Use it only for side effects that should happen once, such as notifying fans that an event was announced.

### Timing

An event reaches consumers up to `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` (60 by default) plus `OUTBOX_RELAY_INTERVAL_MS` (1000 by default) after its visibility moment. Booking Service must not wait for a message to open sales: `sales_window.starts_at` is the opening time.

---

## Configuration that affects consumers

| Variable | Default | Effect for consumers |
|---|---|---|
| `EVENT_VISIBILITY_LEAD_DAYS` | `10` | How long before sales open an event becomes public. A change affects only events not yet published. |
| `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` | `60` | Maximum delay before `EventPublished` is produced. |
| `OUTBOX_RELAY_INTERVAL_MS` | `1000` | Extra delay before any message reaches Kafka. |
| `EVENTS_PAGE_SIZE_DEFAULT` / `EVENTS_PAGE_SIZE_MAX` | `10` / `100` | Paging of list endpoints. |
| `HTTP_REQUEST_TIMEOUT_MS` | `10000` | Longest time a request can take before `503`. |
| `IDEMPOTENCY_KEY_TTL_HOURS` | `24` | How long a `POST /api/v1/events` retry with the same key returns the first response. |
| `KAFKA_EVENTS_TOPIC` | `events.lifecycle` | Topic name. |

Resiliency variables are in ADR-008. The full list is in [ADR-006](adrs-event-propagation.md#adr-006---every-time-duration-and-numeric-limit-comes-from-an-environment-variable).

---

## Changes from the current API

The current `openapi.yml` (generated from `main`) differs from this contract. Consumers should build against this contract.

| Current | This contract | Reason |
|---|---|---|
| `price`, `min_price` are decimal strings with a free `currency` | Integers in thousandths of USD; `currency` always `USD` | ADR-005 |
| `pricing_tiers[]` with `id`, `name`, `description` | `zones[]` with `venue_zone_id`, `name`, `price` | C2, ADR-001 |
| `schedules[]` and `sales[]` arrays | `event_starts_at`, `doors_open_at` and one `sales_window` | C1, C3 |
| `media[]` | `poster_url` | C1 |
| `POST` requires `name`, `artist`, `date`, `venue_id` and creates `Scheduled` | `POST` requires `name` and creates `Draft`; venue, window and scheduling have their own endpoints | C1-C4 |
| Public list returns every event and filters by any `status`, including `Draft` | Public endpoints return published events only; no `status` filter | C1, ADR-007 |
| `mock:<partner-uuid>` tokens accepted even with `MOCK_AUTH=false` | Verified Auth Service token in every shared environment | ADR-007 |
| No `version`, `published_at`, `tour`, `support_acts` | Present | C1, C6, ADR-004 |
| `POST /api/v1/events` retried after a lost response creates a second event | `Idempotency-Key` header required | ADR-008 |
| Venue Service failure returns `500` | `503` with `Retry-After`, after retries and behind a circuit breaker | ADR-008 |
| Empty name or past date gets `400` | Rule violations get `422`; `400` only for malformed requests | C1, C3 diagrams |

---

## Open questions

| Question | Owner | Current assumption |
|---|---|---|
| Token format, issuer and verification keys | Auth Service | Bearer JWT. Until defined, only local development works (ADR-007). |
| Must prices be whole cents (multiples of 10)? | Payment / Booking | Event Service accepts any positive integer (ADR-005). |
| Which show details are required to schedule? | Product | `name`, `artist`, `event_starts_at`, venue with every zone priced, and sales window. `tour`, `support_acts`, `doors_open_at`, `description` and `poster_url` are optional. |
| Should events carry the venue name and time zone? | Venue Service | No. Consumers read them from Venue Service by `venue_id`. |
| Keep `event_type`, `age_policy` and `terms`? | Product | Not in C1, so not in this contract. They stay in the database until decided. |
| Cancellation after sales open | Product | Not designed. No message type exists for it. |
| Retention of `events.lifecycle` | Platform | Log compaction (`cleanup.policy=compact`), so a new consumer can read the latest state of every public event from the topic. |
| Does the lock apply to a draft whose `sale_starts_at` has passed? | Product | No. C5 and ADR-002 say every edit, but then such a draft could never get a new window. The lock applies once the event is `Scheduled`; scheduling the draft gets `422` (C4 diagram). |
