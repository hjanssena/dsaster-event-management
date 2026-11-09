# ADRs - Event propagation

## ADR-001 - Soft-delete zone prices when the venue changes

| Status | Date | Capability |
|---|---|---|
| Proposed | 2026-11-07 | C2 - Select venue and price zones |

### Context

Changing the venue discards the previous venue's zone prices (C2). Zones belong to Venue Service; Event Service keeps one price row per zone per event in `event_pricing_tiers`, referencing the zone by `venue_zone_id`. Prices stay editable until sales open (PRD US-5, step 6), and no buyer pays before the lock (C5). The PRD asks for no price history.

### Decision

On a venue change, Event Service sets `deleted_at` on the previous venue's price rows and inserts one row per new zone with its `venue_zone_id`, a copy of the zone name in the existing `name` column, and the price. The copied name is the snapshot: later layout changes in Venue Service do not alter the event.

Editing a price on the same venue (Pit from €65 to €70) updates the row in place. Every read of prices, including the check at Save (C4), ignores rows with `deleted_at` set. Seats live in Venue Service and are not stored here.

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

A scheduled job in Event Service runs every X minutes and selects `Scheduled` events where `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS <= now()` and `published_at` is null. For each, one transaction sets `published_at` and writes `EventPublished` to the outbox; the relay publishes it to the Kafka topic `events.lifecycle`.

An event scheduled after its visibility moment is published by the scheduling transaction itself (C4), so the job skips it.

### Consequences

- Other services see an event up to X minutes after its visibility moment.
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
