# Service contract - Event Service

| Status | Date | Source |
|---|---|---|
| Proposed, updated 2026-11-27 | 2026-11-15 | [Capabilities C1-C6](capabilities-and-design-event-propagation.md), [ADR-001 to ADR-011](adrs-event-propagation.md) |

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
| Times | RFC 3339. Requests must include an offset (`2026-12-01T10:00:00-06:00`); a time without one gets `422`, and a value that is not a time gets `400` ([ADR-010](adrs-event-propagation.md#adr-010---malformed-requests-get-a-json-400-and-times-are-parsed-in-the-service)). Responses and messages are always UTC (`2026-12-01T16:00:00Z`). |
| Money | Integer thousandths of a US dollar: `1000` is $1.00, `65500` is $65.50. Never a string or float. Responses carry `"currency": "USD"`. See [ADR-005](adrs-event-propagation.md#adr-005---prices-in-us-dollars-as-integer-thousandths). |
| Errors | `{"error": "<message>"}`. Read the category from the HTTP status, not the message. `400` means the request is malformed (invalid JSON, wrong type, missing required field, missing `Content-Type`, bad UUID or query value, a server-owned field); `422` means it is well formed but breaks a rule. Every error has this shape, including a body, path or query the service cannot parse ([ADR-010](adrs-event-propagation.md#adr-010---malformed-requests-get-a-json-400-and-times-are-parsed-in-the-service)). Internal and database details are never returned. |
| Authentication | `Authorization: Bearer <token>`, verified by Event Service on every protected request. Identity, role and ownership are never taken from the request. See [ADR-007](adrs-event-propagation.md#adr-007---the-backend-validates-every-identity-role-and-session). |
| Resiliency | Every request is cut at `HTTP_REQUEST_TIMEOUT_MS`. When Event Service or a service it depends on cannot answer, the response is `503` with `Retry-After` in seconds: the time left before Venue Service is tried again when its circuit breaker is open, otherwise `HTTP_RETRY_AFTER_SECONDS` (2). Clients may retry with backoff; see [Retries and idempotency](#retries-and-idempotency) and [ADR-008](adrs-event-propagation.md#adr-008---timeouts-retries-idempotency-and-circuit-breakers-on-every-call). |
| Limits and durations | Read from environment variables. See [ADR-006](adrs-event-propagation.md#adr-006---every-time-duration-and-numeric-limit-comes-from-an-environment-variable). |

---

## Services Event Service calls

Reviewed against `venues` `main` (`88c214b`, `openapi.json` version `1.0.0`, [`docs/api.md`](https://github.com/HectorBarrera13/venues/blob/main/docs/api.md), [`docs/map-format.md`](https://github.com/HectorBarrera13/venues/blob/main/docs/map-format.md)) and `TicketDSaster-Auth` `main` (`9e1fcf4`), both on 2026-11-27. Gaps are listed in [Open questions](#open-questions).

### Venue Service

| Item | Contract today |
|---|---|
| Endpoints used | `GET /venues/{id}` for the venue, then `GET /venues/{id}/map` for its latest seat map (aliases under `/api/venues`). There is no `/api/v1` prefix. |
| Auth | Bearer JWT (`HS256`, shared `JWT_SECRET`) on every endpoint. `GET /venues/{id}` accepts any signed-in role; `GET /venues/{id}/map` accepts `VENUE_OWNER` and `ORGANIZER`. Event Service forwards the organizer's token, so it needs no token of its own. Auth Service has no service role. |
| Venue | `id` (UUID), `name`, `description`, `address`, `city`, `country` (ISO 3166-1 alpha-2), `location` (`lat`, `lng`), `ownerId`, `createdAt`, and `latestMapVersion` (`0` while the venue has no seat map). |
| Seat map | Numbered versions (`1`, `2`, ...). Each saved map is a new version, and a stored version never changes. A version has a stage, `zones` (`id`, `name`, `outline`) and `seats` (`id`, `label`, `row`, `zone`, `position`). Every zone has at least one seat, and every seat belongs to a zone. |
| Ids | Zone and seat ids are strings matching `^[A-Za-z0-9_-]{1,64}$`, not UUIDs. They are unique within a map and stay the same across versions when an element is moved or renamed; a deleted element's id is never reused. Venue Service asks consumers to reference a seat as `(venueId, mapVersion, seatId)`. |
| Size | A map holds up to 200 zones and 50,000 seats (bodies up to 10 MB). There is no endpoint that returns only the zones, so `VENUE_SERVICE_TIMEOUT_MS` must cover reading a whole map. |
| Time zone | Not available. A venue has no time zone field. |
| Errors | `400`, `401`, `403`, `404`, `500` (and `501` for a route that is not implemented), with body `{"error": "..."}`. |
| Change notifications | None. Instead, the event records the map version it was priced on ([ADR-001](adrs-event-propagation.md#adr-001---soft-delete-zone-prices-when-the-venue-changes)). |

| Venue Service answers | Event Service returns |
|---|---|
| `200` with `latestMapVersion: 0`, or `404` from `GET /venues/{id}/map` | `422` the venue has no seat map yet, so it cannot host events. |
| `200` with a map | Continues with the zones of that map version. |
| `404` from `GET /venues/{id}` | `422` venue does not exist. |
| `401` or `403` | `503` with `Retry-After`, logged as a configuration error. Event Service has already verified the same token with the same secret, so this means the two services disagree (see [Open questions](#open-questions)). |
| Timeout, connection error, `429`, `5xx` | Retried, then `503` with `Retry-After` ([ADR-008](adrs-event-propagation.md#adr-008---timeouts-retries-idempotency-and-circuit-breakers-on-every-call)). |

### Auth Service

Event Service never calls Auth Service during a request: tokens are verified locally.

| Item | Contract today |
|---|---|
| Token | JWT signed with `HS256` and the shared secret `JWT_SECRET`. Event Service accepts only `HS256`. |
| Claims | `sub` (user id, UUID), `role` (`ORGANIZER` or `VENUE_OWNER`, uppercase), `iat`, `exp`. No `iss`, `aud` or user name. |
| Lifetime | 8 hours (`JWT_EXPIRES_IN_SECONDS`). No refresh endpoint (`POST /auth/refresh` is only a proposal in its README). |
| Keys | None to fetch: `GET /.well-known/jwks.json` returns no key material, because the secret is shared. `POST /auth/validate` exists but is not needed. |
| Users | Stored in PostgreSQL with `gen_random_uuid()` ids since `594af49`, so `sub` stays the same across restarts. Auth Service offers no endpoint or event to list or sync users. |

The organizer is the token's `sub`, and organizer endpoints require `role` `ORGANIZER`, compared without case. Because there are no keys to fetch, an Auth Service outage cannot affect token verification. ADR-007 and ADR-008 were amended on 2026-11-16 to match: no issuer check until the token has `iss`, no partner lookup, and no call to Auth Service. Since the 2026-11-27 amendment of ADR-007, Event Service accepts no other kind of token in any environment: there are no mock tokens.

---

## Event lifecycle

| Stage | When | `status` | Visible to |
|---|---|---|---|
| Draft | Created (C1) | `Draft` | Owning organizer only |
| Scheduled | Organizer schedules it and all checks pass (C4) | `Scheduled` | Owning organizer only |
| Published | `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS`, found by the publish job, or at once when scheduling or an edit reaches that moment (C6) | `Scheduled`, `published_at` set | Everyone; `EventPublished` sent |
| Locked | `sale_starts_at` (C5) | `Scheduled` | Everyone; no more edits, rescheduling or `EventUpdated` |

An event is locked when the current time is at or after `sales_window.starts_at`; there is no separate field. Publication is final: an edit that moves the sales window or the show date later never makes a published event private again ([ADR-003](adrs-event-propagation.md#adr-003---publish-events-with-a-scheduled-job-at-the-visibility-moment)). Cancellation, sold out and completion are not designed yet, and no `status` other than `Draft` and `Scheduled` is returned.

---

## Event object

The same object is returned by `GET /api/v1/events/{id}` and carried in every Kafka message. On a published event every field except those marked optional is set.

| Field | Type | Description |
|---|---|---|
| `id` | UUID | Event id. Kafka message key. |
| `version` | integer | Starts at `1` and increases by 1 with every change: each accepted edit, scheduling and publication. A request that changes the event and publishes it counts once ([ADR-004](adrs-event-propagation.md#adr-004---integer-event-version-and-full-state-messages)). |
| `status` | `Draft` \| `Scheduled` | See [Event lifecycle](#event-lifecycle). |
| `organizer_id` | UUID | Organizer that owns the event, taken from the token at creation. |
| `name` | string | Event name. See [Field limits](#field-limits). |
| `artist` | string | Headliner. Null only on a draft. |
| `tour` | string, optional | Tour name. |
| `support_acts` | string[] | Opening acts in billing order. Empty when none. |
| `event_starts_at` | UTC time | When the show starts. Null only on a draft. |
| `doors_open_at` | UTC time, optional | When doors open. Not later than `event_starts_at`. |
| `description` | string, optional | Free text. |
| `poster_url` | string, optional | Poster image URL. |
| `venue_id` | UUID | Venue in Venue Service. Null only on a draft. |
| `venue_map_version` | integer | Version of the venue's seat map the zones were taken from. Seats are read from Venue Service as `GET /venues/{venue_id}/map?version={venue_map_version}`. Null only on a draft. |
| `currency` | `USD` | Always `USD`. |
| `zones` | Zone[] | One per zone of that map version, snapshotted when the venue was saved ([ADR-001](adrs-event-propagation.md#adr-001---soft-delete-zone-prices-when-the-venue-changes)). |
| `sales_window` | object | `starts_at` and `ends_at`, both UTC. Null only on a draft. |
| `published_at` | UTC time | When the event became public. Null until published. |
| `created_at`, `updated_at` | UTC time | Record timestamps. |

**Zone**

| Field | Type | Description |
|---|---|---|
| `venue_zone_id` | string | Zone id in the seat map (`^[A-Za-z0-9_-]{1,64}$`, not a UUID). Seats of the zone are the map's seats whose `zone` is this id. |
| `name` | string | Zone name copied from Venue Service when the venue was saved. |
| `price` | integer | Thousandths of a US dollar, greater than `0`. Null only on a draft. |

### Field limits

Requests that break a limit get `422`. Leading and trailing whitespace is removed before the check and before saving. Lengths count characters (Unicode code points), as Postgres does. The limits match the database columns, so they are part of the contract and not configuration ([ADR-006](adrs-event-propagation.md#adr-006---every-time-duration-and-numeric-limit-comes-from-an-environment-variable)).

| Field | Limit |
|---|---|
| `name` | Required, 1–200 characters. |
| `artist` | 1–200 characters. Required to schedule. |
| `tour` | 1–200 characters, or null. |
| `support_acts` | At most 20 entries, each 1–200 characters. |
| `description` | At most 5,000 characters, or null. |
| `poster_url` | Absolute `https://` URL, at most 2,048 characters, or null. |

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
  "venue_map_version": 2,
  "currency": "USD",
  "zones": [
    { "venue_zone_id": "z-pit", "name": "Pit", "price": 65000 },
    { "venue_zone_id": "z-lower", "name": "Lower Tier", "price": 45500 },
    { "venue_zone_id": "z-upper", "name": "Upper Tier", "price": 30000 }
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

No token required. They return published events only, whatever filters the client sends. A token is optional: a missing, invalid or expired token is treated as anonymous, never answered with `401` or `403` ([ADR-007](adrs-event-propagation.md#adr-007---the-backend-validates-every-identity-role-and-session)).

#### `GET /api/v1/events`

List published events whose show has not started yet (`event_starts_at` later than the current time), soonest `event_starts_at` first. An event leaves the list when its show starts; it can still be read with `GET /api/v1/events/{id}`.

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

Return one [event object](#event-object). A published event is returned to anyone. An unpublished event (a draft, or scheduled before its visibility moment) is returned only to its owner: a valid `ORGANIZER` token whose `sub` is the event's `organizer_id`. Everyone else, including a caller with a missing, invalid or expired token, gets `404`.

Errors: `400` malformed id, `404` not found or not visible to the caller, `503` timeout or database unavailable, `500`.

### Organizer endpoints

All require a bearer token. Event Service extracts the organizer and role from the verified token and checks ownership on every request.

| Status | Meaning on every organizer endpoint |
|---|---|
| `401` | Token missing, invalid or expired. |
| `403` | Partner is not an organizer. |
| `404` | Event does not exist or belongs to another organizer. |
| `409` | Event locked: the event is `Scheduled` and `sales_window.starts_at` has passed (C5). Any edit endpoint, and scheduling. |
| `503` | Request timed out, or Venue Service or the database cannot answer. Carries `Retry-After`. Tokens are verified locally, so an Auth Service outage never causes `401` or `503`. |

A request that sends a field Event Service owns (`id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at`, zone `name`) gets `400`. Each successful edit returns `200` with the full event object and its new `version`. Edits to a `Scheduled` event must keep every C4 check true, or they get `422`. If an edit brings a `Scheduled` event that is not public yet to its visibility moment (for example by moving the sales window earlier), the same request publishes it.

| Method and path | Capability | Body | Success | Specific errors |
|---|---|---|---|---|
| `GET /api/v1/events/mine` | - | Query `page`, `per_page`, optional `status` | `200`, page of the caller's events in any state, most recently updated first ([details](#get-apiv1eventsmine)). The organizer comes from the token; there is no `organizer_id` parameter. | `400` unknown `status` |
| `POST /api/v1/events` | C1 | Header `Idempotency-Key` (UUID, required). `name` (required) and any show details. | `201`, event with `status: Draft`, `version: 1`. A repeat with the same key returns the first response. A repeat sent while the first request is still running waits for it, then returns the same response. | `400` missing `Idempotency-Key`; `422` empty name, time without offset, or same key with a different body; `503` the repeat waited longer than `DB_STATEMENT_TIMEOUT_MS` |
| `PATCH /api/v1/events/{id}` | C1 | Any of `name`, `artist`, `tour`, `support_acts`, `event_starts_at`, `doors_open_at`, `description`, `poster_url`. Omitted fields stay; `null` clears an optional field. | `200` | `422` a field outside its [limit](#field-limits), time without offset, `doors_open_at` after `event_starts_at`, `event_starts_at` earlier than `sales_window.ends_at`, or, on a `Scheduled` event, clearing `artist` or `event_starts_at` |
| `PUT /api/v1/events/{id}/venue` | C2 | `venue_id` and `zones: [{ "venue_zone_id", "price" }]`, `price` required in each entry. Zones may be left out ([rules](#saving-the-venue)). | `200`, zones read again from the venue's latest seat map and snapshotted, with its `venue_map_version` | `422` price not greater than 0, venue does not exist, venue has no seat map, a zone is not in the map, or, on a `Scheduled` event, a zone left without a price |
| `PUT /api/v1/events/{id}/sales-window` | C3 | `starts_at`, `ends_at` | `200` | `422` breaks `now < starts_at < ends_at <= event_starts_at` (server time) |
| `POST /api/v1/events/{id}/schedule` | C4 | None | `200`, `status: Scheduled`, `version` increased; `published_at` set if the visibility moment has passed. Scheduling an event that is already `Scheduled` returns `200` with the event unchanged, until sales open. | `409` already `Scheduled` and sales have opened; `422` a check failed (including sales open already passed on a draft), with the first failing check in `error` |

#### `GET /api/v1/events/mine`

The caller's events in any state, most recently updated first (`updated_at` descending, then `id`). `page` and `per_page` work as on the public list. `status` (`Draft` or `Scheduled`) narrows the list; any other value gets `400`.

Each item is a summary. Fields not set yet are null.

```json
{
  "id": "6f1c2a9e-3b7d-4c55-9a1e-2d4f8b6c0a11",
  "version": 4,
  "status": "Scheduled",
  "name": "Fuerza Regida - Merida",
  "artist": "Fuerza Regida",
  "event_starts_at": "2026-12-20T03:00:00Z",
  "venue_id": "33333333-3333-3333-3333-333333333333",
  "poster_url": "https://cdn.example.com/posters/6f1c2a9e.jpg",
  "sales_window": { "starts_at": "2026-12-01T16:00:00Z", "ends_at": "2026-12-19T23:00:00Z" },
  "published_at": null,
  "updated_at": "2026-11-25T09:14:31Z"
}
```

The page uses the same envelope as the public list (`items`, `page`, `per_page`, `total_items`, `total_pages`). The full event, with zones, comes from `GET /api/v1/events/{id}`.

#### Saving the venue

Every `PUT /api/v1/events/{id}/venue` reads the venue's **latest** seat map from Venue Service again, also when the venue does not change, and records its version as `venue_map_version` ([ADR-001](adrs-event-propagation.md#adr-001---soft-delete-zone-prices-when-the-venue-changes)). Zones are matched by `venue_zone_id`, which stays the same across map versions.

| Zone | Same venue | New venue |
|---|---|---|
| In the venue and in the request | Takes the request's price | Takes the request's price |
| In the venue, left out of the request | Keeps its current price | No price yet |
| No longer in the latest map | Removed from the event, whatever the request says | - |
| In the request but not in the latest map | `422` | `422` |

Zone names are refreshed from the map on every save. A venue whose `latestMapVersion` is `0` has no seat map and gets `422`. If the request lists the same `venue_zone_id` more than once, the last entry replaces the earlier ones. A `Scheduled` event must still have every zone priced afterwards, or the save gets `422`.

### Retries and idempotency

| Request | Safe to retry | How |
|---|---|---|
| `GET` | Yes | Repeat as is. |
| `PATCH`, `PUT` | Yes | Repeating sets the same state. It may increase `version` and send an `EventUpdated` with the same content. |
| `POST /api/v1/events/{id}/schedule` | Yes | An event already `Scheduled` returns `200` until sales open, then `409`. |
| `POST /api/v1/events` | Only with the same `Idempotency-Key` | Create one UUID per new event form and reuse it on every retry of that form. A retry that overlaps the first request waits for it; it never creates a second draft. Keys expire after `IDEMPOTENCY_KEY_TTL_HOURS` (24). |

Retry only on a timeout, `429` or `503`, with exponential backoff and jitter, and wait at least `Retry-After` when present. Do not retry other `4xx` responses.

Example `PUT /api/v1/events/{id}/venue` body:

```json
{
  "venue_id": "33333333-3333-3333-3333-333333333333",
  "zones": [
    { "venue_zone_id": "z-pit", "price": 65000 },
    { "venue_zone_id": "z-lower", "price": 45500 },
    { "venue_zone_id": "z-upper", "price": 30000 }
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
| Producer | Event Service only, through its outbox ([ADR-004](adrs-event-propagation.md#adr-004---integer-event-version-and-full-state-messages)). One relay sends at a time, however many instances run ([ADR-009](adrs-event-propagation.md#adr-009---one-active-outbox-relay-through-a-postgres-advisory-lock)). |
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
| `EventPublished` | Once per event, at `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS`, or right away when scheduling or an edit reaches that moment (C6). Late after a publish job outage (see below). | Increased by publishing, like every change |
| `EventUpdated` | After each accepted edit of a published event, until sales open (C5). | Higher than any earlier message for the event |

No messages are sent for drafts or for scheduled events before their visibility moment. After sales open no `EventUpdated` is sent, because the event can no longer be edited (C5).

The one message that can arrive after sales open is a late `EventPublished`. If the publish job was not running when an event's visibility moment and `sales_window.starts_at` both passed, it publishes the event when it runs again, because Booking Service needs the event to sell tickets. Consumers must accept an `EventPublished` whose sales window has already opened.

### Consumer rules

1. Store the last `version` applied for each `event_id`.
2. Drop a message whose `version` is less than or equal to the stored one. It is a repeat or a replay.
3. Otherwise replace your whole copy of the event with `event`. Treat both types as an upsert: every message holds the complete state, so a consumer that missed earlier messages is still correct after the next one.
4. Do not rely on `type` for state. Use it only for side effects that should happen once, such as notifying fans that an event was announced.

### Timing

An event reaches consumers up to `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` (60 by default) plus `OUTBOX_RELAY_INTERVAL_MS` (1000 by default) after its visibility moment, or within `OUTBOX_RELAY_INTERVAL_MS` when scheduling or an edit published it. Booking Service must not wait for a message to open sales: `sales_window.starts_at` is the opening time.

---

## Configuration that affects consumers

| Variable | Default | Effect for consumers |
|---|---|---|
| `EVENT_VISIBILITY_LEAD_DAYS` | `10` | How long before sales open an event becomes public. A change affects only events not yet published. |
| `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` | `60` | Maximum delay before `EventPublished` is produced. |
| `OUTBOX_RELAY_INTERVAL_MS` | `1000` | Extra delay before any message reaches Kafka. |
| `EVENTS_PAGE_SIZE_DEFAULT` / `EVENTS_PAGE_SIZE_MAX` | `10` / `100` | Paging of list endpoints. |
| `HTTP_REQUEST_TIMEOUT_MS` | `10000` | Longest time a request can take before `503`. |
| `HTTP_RETRY_AFTER_SECONDS` | `2` | `Retry-After` on a `503`, unless the Venue Service circuit breaker gives a longer wait. |
| `IDEMPOTENCY_KEY_TTL_HOURS` | `24` | How long a `POST /api/v1/events` retry with the same key returns the first response. |
| `KAFKA_EVENTS_TOPIC` | `events.lifecycle` | Topic name. |

Resiliency variables are in ADR-008. The full list is in [ADR-006](adrs-event-propagation.md#adr-006---every-time-duration-and-numeric-limit-comes-from-an-environment-variable).

---

## Changes from the current API

The current `openapi.yml` (generated from `main`) differs from this contract. Consumers should build against this contract.

| Current | This contract | Reason |
|---|---|---|
| `price`, `min_price` are decimal strings with a free `currency` | Integers in thousandths of USD; `currency` always `USD` | ADR-005 |
| `pricing_tiers[]` with `id`, `name`, `description` | `zones[]` with `venue_zone_id`, `name`, `price`, and the event's `venue_map_version` | C2, ADR-001 |
| `schedules[]` and `sales[]` arrays | `event_starts_at`, `doors_open_at` and one `sales_window` | C1, C3 |
| `media[]` | `poster_url` | C1 |
| `POST` requires `name`, `artist`, `date`, `venue_id` and creates `Scheduled` | `POST` requires `name` and creates `Draft`; venue, window and scheduling have their own endpoints | C1-C4 |
| Public list returns every event and filters by any `status`, including `Draft` | Public endpoints return published events only; no `status` filter | C1, ADR-007 |
| `mock:<partner-uuid>` tokens accepted even with `MOCK_AUTH=false` | Only verified `HS256` tokens, in every environment; mock tokens removed | ADR-007 |
| No `version`, `published_at`, `tour`, `support_acts` | Present | C1, C6, ADR-004 |
| `POST /api/v1/events` retried after a lost response creates a second event | `Idempotency-Key` header required | ADR-008 |
| Venue Service failure returns `500` | `503` with `Retry-After`, after retries and behind a circuit breaker | ADR-008 |
| A body, path or query that cannot be parsed gets a plain-text `400`, `415` or `422` | `400` with `{"error": "..."}` | ADR-010 |
| Public list includes past events, newest created first | Upcoming shows only, soonest first | C6 |
| Empty name or past date gets `400` | Rule violations get `422`; `400` only for malformed requests | C1, C3 diagrams |

---

## Open questions

| Question | Owner | Current assumption |
|---|---|---|
| Will the token get `iss` and `aud` claims? ADR-007 checks the issuer once it exists. | Auth Service | Verify `HS256` signature, `exp`, `sub` and `role` only. |
| Will there be a token refresh? An 8-hour token can expire while an organizer edits a draft. | Auth Service | No refresh. The organizer logs in again; the draft is already saved step by step (C1). |
| How does Booking Service read the seats of an event? | Booking Service, Venue Service | Settled on 2026-11-27: Venue Service serves seats in its seat maps, and a map version never changes. Booking reads `GET /venues/{venue_id}/map?version={venue_map_version}` and takes the seats whose `zone` is the zone's `venue_zone_id`. What is still open is the token: that endpoint accepts only `VENUE_OWNER` and `ORGANIZER`, and Auth Service has no service role. |
| A map holds up to 50,000 seats and there is no zones-only endpoint. Is `VENUE_SERVICE_TIMEOUT_MS` (`2000`) enough to read one? | Event Service, Venue Service | Measure against a large map before release. Raise the variable, or ask Venue Service for a zones-only read, if it is not. |
| What does a `401` or `403` from Venue Service mean, when Event Service already accepted the same token? | Event Service, Venue Service | A configuration mismatch (secret or role names). Returned as `503` and logged, never passed to the organizer as `401`. |
| Must prices be whole cents (multiples of 10)? | Payment / Booking | Event Service accepts any positive integer (ADR-005). |
| Which show details are required to schedule? | Product | `name`, `artist`, `event_starts_at`, venue with every zone priced, and sales window. `tour`, `support_acts`, `doors_open_at`, `description` and `poster_url` are optional. |
| Should events carry the venue name and time zone? Venue Service has no time zone field, and every venue endpoint requires a token (`GET /venues/{id}` any signed-in role, the map `VENUE_OWNER` or `ORGANIZER`), while Auth Service issues only `VENUE_OWNER` and `ORGANIZER` tokens. So fans, Search Service and Booking Service cannot read venues today. | Venue Service, Search Service, Booking Service | No. Consumers read them from Venue Service by `venue_id`, which needs a public or service-to-service read and a time zone (IANA name, e.g. `America/Mexico_City`) on the venue. |
| Keep `event_type`, `age_policy` and `terms`? | Product | Not in C1 or the v1 contract; dropped in the target [data model](../architecture/data-model.md) and can be re-introduced later if Product requests them. |
| Cancellation after sales open | Product | Not designed. No message type exists for it. |
| Retention of `events.lifecycle` | Platform | Log compaction (`cleanup.policy=compact`), so a new consumer can read the latest state of every public event from the topic. |
| Does the lock apply to a draft whose `sale_starts_at` has passed? | Product | No. Otherwise such a draft could never get a new window, and a draft with no window yet (`sale_starts_at` null) could never be edited. The lock applies once the event is `Scheduled`; scheduling the draft gets `422` (C4 diagram). C5, ADR-002 and ADR-004 already use this condition. |
