# Event Service API brief

Short guide for teams that read events from Event Service. Full rules and rationale: [service contract](contract-event-service.md).

Status: proposed target contract. The current `openapi.yml` describes older code and differs from it.

| You are | Use |
|---|---|
| Fan frontend | Public HTTP endpoints |
| Organizer frontend (Backstage) | Organizer HTTP endpoints |
| Booking Service | Kafka, group `booking-service`. Public endpoints to recover state. |
| Search Service | Kafka, group `search-service` |

---

## Basics

| | |
|---|---|
| Base path | `/api/v1/events` |
| Format | JSON, `snake_case`, UUID ids |
| Times | Send RFC 3339 with an offset (`2026-12-01T10:00:00-06:00`); without one you get `422`. Responses and messages are UTC. |
| Money | Integer thousandths of a US dollar: `65500` = $65.50. `currency` is always `"USD"`. |
| Auth | `Authorization: Bearer <token>` from Auth Service. Only organizer endpoints need it. Public endpoints treat a missing or invalid token as anonymous. |
| Text limits | `name`, `artist`, `tour` and each support act: 200 characters. Up to 20 support acts. `description`: 5,000. `poster_url`: 2,048, `https://` only. Longer gets `422`. |
| Errors | `{"error": "..."}` on every error, including requests the service cannot parse. Use the status code, not the message. |

---

## The event

`GET /api/v1/events/{id}` and every Kafka message return the same object.

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
    { "venue_zone_id": "z-pit", "name": "Pit", "price": 65000 }
  ],
  "sales_window": { "starts_at": "2026-12-01T16:00:00Z", "ends_at": "2026-12-19T23:00:00Z" },
  "published_at": "2026-11-21T16:00:42Z",
  "created_at": "2026-11-10T18:22:05Z",
  "updated_at": "2026-11-25T09:14:31Z"
}
```

- `tour`, `doors_open_at`, `description` and `poster_url` may be null. Every other field is set on a published event.
- `zones` is a snapshot of the zones of the venue's seat map, taken when the organizer saved the venue. `venue_map_version` is the map version they came from, and `venue_zone_id` is the zone's id in that map: a string, not a UUID.
- The event has no venue name, time zone or seats. Read seats from Venue Service at `GET /venues/{venue_id}/map?version={venue_map_version}`: a zone's seats are those whose `zone` is its `venue_zone_id`. That version never changes.
- `version` goes up by 1 with every change: each edit, scheduling and publication.

**Lifecycle**

| Stage | When | Public |
|---|---|---|
| Draft | Created | No |
| Scheduled | Organizer scheduled it | No |
| Published | `sales_window.starts_at` minus 10 days (`EVENT_VISIBILITY_LEAD_DAYS`), or at once if scheduling or an edit reaches that moment | Yes. `published_at` is set, and stays set even if dates move later. |
| Locked | `sales_window.starts_at` | Yes. No more edits, rescheduling or `EventUpdated`. |

`status` is only ever `Draft` or `Scheduled`. Cancellation and sold out do not exist yet.

---

## HTTP endpoints

### Public (no token)

Only published events are returned.

| Request | Returns |
|---|---|
| `GET /api/v1/events?page=1&per_page=10&venue_id=&organizer_id=` | Page of summaries of upcoming shows (`event_starts_at` not reached yet), soonest first: `items` with `id`, `name`, `artist`, `event_starts_at`, `venue_id`, `poster_url`, `min_price`, `currency`, `sales_window`, plus `page`, `per_page`, `total_items`, `total_pages`. `per_page` is capped at 100. |
| `GET /api/v1/events/{id}` | The full event. An unpublished event is returned only to its owner's token; anyone else gets `404`. |

### Organizer (token with role `ORGANIZER`)

The organizer comes from the token. Each successful edit returns `200` with the full event and its new `version`.

| Request | Body | Does |
|---|---|---|
| `GET /api/v1/events/mine` | Query `page`, `per_page`, optional `status` | Summaries of the caller's events in any state, most recently updated first |
| `POST /api/v1/events` | Header `Idempotency-Key: <uuid>`. `name`, optional show details | Creates a `Draft` (`201`) |
| `PATCH /api/v1/events/{id}` | Any of `name`, `artist`, `tour`, `support_acts`, `event_starts_at`, `doors_open_at`, `description`, `poster_url` | Edits show details |
| `PUT /api/v1/events/{id}/venue` | `venue_id`, `zones: [{venue_zone_id, price}]` | Sets the venue and zone prices. Zones are read again from the venue's latest seat map on every save; a zone left out keeps its price. A venue with no seat map gets `422`. |
| `PUT /api/v1/events/{id}/sales-window` | `starts_at`, `ends_at` | Sets the sales window |
| `POST /api/v1/events/{id}/schedule` | | Moves `Draft` to `Scheduled` if every check passes. Already scheduled: `200` until sales open, then `409`. |

Never send `id`, `organizer_id`, `status`, `version`, `published_at`, `created_at`, `updated_at` or zone `name`. Sending any of them gets `400`.

### Status codes

| Code | Meaning |
|---|---|
| `400` | Malformed request: bad JSON, wrong type, missing field or `Content-Type`, bad UUID or query value, a server-owned field, missing `Idempotency-Key` |
| `401` | Token missing, invalid or expired |
| `403` | Token is not an organizer |
| `404` | Not found, not public, or owned by another organizer |
| `409` | Event locked (sales opened) |
| `422` | Breaks a rule: price not above 0, time without offset, dates out of order, unknown venue, failed schedule check |
| `503` | Timeout, or a dependency is down. Has `Retry-After` in seconds (2 by default). |

### Retries

Retry only on a timeout, `429` or `503`, with exponential backoff and jitter, and wait at least `Retry-After`. `GET`, `PATCH`, `PUT` and `schedule` are safe to repeat. Repeat `POST /api/v1/events` only with the same `Idempotency-Key`. A repeat that arrives while the first request is still running waits for it and gets the same response. Keys last 24 hours.

---

## Kafka: `events.lifecycle`

| | |
|---|---|
| Key | Event `id` |
| Value | JSON |
| Order | Ordered per event, because all of an event's messages share a key and a single relay sends them |
| Delivery | At least once. Expect repeats. |
| Groups | `booking-service`, `search-service` |

```json
{
  "type": "EventUpdated",
  "event_id": "6f1c2a9e-3b7d-4c55-9a1e-2d4f8b6c0a11",
  "version": 4,
  "occurred_at": "2026-11-25T09:14:31Z",
  "event": { "...": "the full event, same as GET /api/v1/events/{id}" }
}
```

| `type` | Sent |
|---|---|
| `EventPublished` | Once per event, when it becomes public. Can arrive after sales open if Event Service's publish job was down. |
| `EventUpdated` | After each edit of a public event, until sales open |

Nothing is sent for drafts or for scheduled events that are not public yet. After sales open, only a late `EventPublished` can arrive; no `EventUpdated`.

### How to consume

1. Keep the last `version` you applied for each `event_id`.
2. Drop a message whose `version` is less than or equal to it.
3. Otherwise replace your whole copy with `event`. Every message is the full state, so treat both types as an upsert.
4. Use `type` only for one-time side effects, such as announcing a new event.

### Timing

A message arrives up to about 61 seconds after the event becomes public (`EVENT_PUBLISH_JOB_INTERVAL_SECONDS` + `OUTBOX_RELAY_INTERVAL_MS`), or about 1 second after the request when scheduling or an edit published it. Booking Service opens sales at `sales_window.starts_at` and must not wait for a message to do it.

---

## Not settled yet

These affect consumers. Details in [open questions](contract-event-service.md#open-questions).

- Venue Service has no time zone, and every venue endpoint needs a token (the seat map only `VENUE_OWNER` or `ORGANIZER`). So fans, Search Service and Booking Service cannot read venue names or seats from it today, even though the seats themselves are settled (`venue_map_version`).
- Topic retention: log compaction is planned, so a new consumer can rebuild state from the topic.
