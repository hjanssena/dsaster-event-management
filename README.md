# D-saster - Event Management Microservice

Welcome to the **Event Management Microservice** for **Dsaster** (Ticket D-Saster), a high-scale event ticket sales platform engineered to deliver reliable, fraud-resistant ticketing experiences under extreme concurrent demand.

---

## Team: Aura Team

This microservice is developed and maintained by the **Aura Team**:

- **Hugo de Jesús Janssen Aguilar**
- **Emiliano Contreras Gamboa**
- **Edwing Mauricio Molina Chim**
- **Alejandro Magdiel Duran Varela**
- **Alejandro Lopez Maldonado**
- **Osmar Ruben Ciau Martin**
- **Eduardo Alexander Canto Paredes**

---

## Architectural Context & DDD Sub-Domains

Based on the system's strategic Domain-Driven Design (DDD) decomposition:

- **Core Domain:** `SALES` (in charge of processing fans' high-concurrency purchases, sessions, holds, payments, and queues).
- **Generic Sub-Domains (Secondary):**
  - **`EventManagement` (This Service):** In charge of the complete management and lifecycle of events.
  - **`VenueManagement`:** In charge of venue registrations and physical seat map definitions.
  - **`Search`:** In charge of processing fans' event and venue discovery/search queries.
- **Support Sub-Domain:** `Auth / UserManagement` (fan registration and backstage users: organizers and venue owners).

### Domain Entity Relationships

```
  +---------------+
  |     Venue     |
  +---------------+
    |           \
    | Host       \ Provides Map
    v             v
+-------+  Has  +-------+
| Event | ----> | Seat  |
+-------+       +-------+
                    |
                    | Has
                    v
                +--------+  Reserve  +---------+
                | Ticket | <-------- | Session |
                +--------+           +---------+
                                      |       \
                                      | Can do \ Join
                                      v         v
                                 +---------+  +-------+
                                 | Payment |  | Queue |
                                 +---------+  +-------+
```

### Context Entities within EventManagement

1. **`Event` (Aggregate Root):** Core entity representing the show/performance, its settings, status, schedule, and metadata.
2. **`Venue` (Association):** Reference to the hosting venue registered via `VenueManagement`.
3. **`Seat` (Event Seating Configuration):** Event-specific seating tiers, pricing categories, and availability layout based on the map provided by the venue.

---

## Service Scope & Responsibilities

The **Event Management Microservice** is responsible for:

### 1. Event Creation, Configuration & Publishing

- **Metadata & Settings Registration:** Registering and updating event settings (performer/artist info, descriptions, multimedia assets ~100 KB, age policies, and event terms).
- **Publication Workflow:** Publishing events from draft state, making them accessible to downstream read-models and search catalogs.

### 2. Venue Assignment & Seat Mapping

- **Venue Hosting (`Venue -> Host -> Event`):** Associating an event with an existing registered venue without redrawing physical infrastructure.
- **Seating & Pricing Setup (`Event -> Has -> Seat`):** Overlaying pricing tiers (VIP, General, Balcony) onto the physical seats and sections provided by the venue map (`Venue -> Provides Map -> Seat`).

### 3. Ticket Sale Scheduling

- **Sale Timing Configuration:** Defining exact, synchronized dates and timestamps for when tickets go on sale (e.g., scheduled Friday on-sale rituals down to the exact second).
- **Sale Triggers:** Signaling the Core Sales and Queue services when ticket sales open or close.

### 4. Event State Management

Maintains and coordinates the end-to-end lifecycle state machine of each event:

- **`Draft`:** Initial configuration by organizers; invisible to fans.
- **`Scheduled / Announced`:** Visible for discovery, but ticket purchasing is locked.
- **`On Sale (Active)`:** Sales window is open; queues and seat reservations active.
- **`Sold Out`:** All ticket capacity has been committed or sold.
- **`In Progress`:** Event is actively taking place.
- **`Completed`:** Event has concluded; sales and modifications closed.
- **`Postponed / Cancelled`:** Rescheduling or cancellation workflows, triggering downstream refund and notification events.

---

## Non-Functional & Architectural Goals

- **Event-Driven Decoupling:** Emits domain events (`EventCreated`, `EventPublished`, `SaleDateScheduled`, `EventStateChanged`) to Kafka/RabbitMQ for downstream consumption.
- **High Read Reliability:** Provides stable, cache-friendly event projections to isolate organizer configuration loads from high-traffic fan spikes.
- **Time Synchronization:** Guarantees accurate, server-controlled sale date scheduling, avoiding client-side time manipulation.
