# Component Diagram — Event Management Microservice

This document describes the internal components and external boundaries of the **Event Management Microservice** (Rust / Axum / SeaORM / PostgreSQL).

- **Implemented today:** HTTP API (`GET/POST /api/v1/events`, `GET /api/v1/events/{id}`, `/health`, Swagger UI), `EventService`, `AuthService`, `MockTokenVerifier`, `HttpVenueClient` / `MockVenueClient`, `EventRepository`, and `PartnerRepository`.
- **Target Event Propagation additions** (specified in [Capabilities C1–C6](event-propagation/capabilities-and-design-event-propagation.md), [Service Contract](event-propagation/contract-event-service.md), and [ADR-001 to ADR-011](event-propagation/adrs-event-propagation.md)): local `HS256` JWT verifier, resiliency middleware and circuit breaker, draft-to-scheduled workflow endpoints, `Publish Scheduler`, `Outbox Relay`, and Kafka (`events.lifecycle`).

```mermaid
graph TD
    classDef external fill:#f9f9f9,stroke:#333,stroke-width:2px,stroke-dasharray: 5 5;
    classDef planned fill:#fff8e1,stroke:#f9a825,stroke-width:2px,stroke-dasharray: 4 2;
    classDef database fill:#ffcc80,stroke:#f57c00,stroke-width:2px;
    classDef broker fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px,stroke-dasharray: 4 2;

    subgraph "External Systems"
        Client["Clients (Fan & Organizer Frontends)"]:::external
        AuthServiceExt["Auth Service (issues HS256 JWTs — not called at runtime)"]:::external
        VenueServiceExt["Venue Management Service (GET /venues/{id})"]:::external
        DB[("PostgreSQL (event_store)")]:::database
        Kafka[["Kafka (events.lifecycle) — planned (C6)"]]:::broker
        BookingExt["Booking Service (group: booking-service)"]:::external
        SearchExt["Search Service (group: search-service)"]:::external
    end

    subgraph "Event Management Microservice (Rust / Axum)"
        subgraph "API & Middleware Layer (src/api)"
            Middleware["HTTP Middlewares (CORS, Trace, Timeout — ADR-008)"]
            EventAPI["Event API Handlers (event_api.rs)"]
            AuthExtractor["AuthenticatedPartner Extractor (auth.rs)"]
            HealthAPI["Health Check (health_api.rs)"]
            OpenAPI["Swagger UI & OpenAPI (openapi.rs)"]
        end

        subgraph "Service Layer (src/service)"
            EventSrv["Event Service (event_srv.rs)"]
            AuthSrv["Auth Service (auth_srv.rs)"]
            TokenVerifier["Token Verifier (token_verifier.rs: Mock -> local HS256 JWT)"]
            VenueClient["Venue Client (venue_client.rs: retry + circuit breaker)"]
        end

        subgraph "Background Workers — planned (ADR-003 / ADR-004)"
            PublishJob["Publish Scheduler (visibility lead time + idempotency cleanup)"]:::planned
            OutboxRelay["Outbox Relay (one active instance, at-least-once Kafka producer)"]:::planned
        end

        subgraph "Repository Layer (src/repository)"
            EventRepo["Event Repository (event_repo.rs)"]
            PartnerRepo["Partner Repository (partner_repo.rs — legacy dev table; dropped per ADR-007)"]
        end

        subgraph "Domain & Persistence Model (src/model)"
            Models["Entities & DTOs (Event, Zone Pricing, Sales Window, Outbox, Idempotency)"]
        end
    end

    %% Client interactions
    Client -.->|"Login (outside this service)"| AuthServiceExt
    Client -->|"HTTP / JSON (Bearer JWT for organizer routes)"| Middleware
    Middleware --> EventAPI
    Middleware --> HealthAPI
    Middleware --> OpenAPI

    %% Auth extraction (local verification, no network call to Auth Service)
    EventAPI -->|"Extracts organizer"| AuthExtractor
    AuthExtractor --> AuthSrv
    AuthSrv -->|"Verifies locally (HS256 + JWT_SECRET)"| TokenVerifier
    AuthSrv -.->|"Current lookup (removed once HS256 JWT replaces mock)"| PartnerRepo

    %% Core service interactions
    EventAPI --> EventSrv
    EventSrv --> EventRepo
    EventSrv -->|"Forwards organizer JWT"| VenueClient
    VenueClient -->|"HTTP JSON (timeout, retry, breaker)"| VenueServiceExt

    %% Persistence
    EventRepo -->|"SeaORM / SQL (transactions & DB-clock lock)"| DB
    PartnerRepo -->|"SeaORM / SQL"| DB

    %% Async Event Propagation (C6)
    PublishJob -->|"Sets published_at & writes outbox"| DB
    OutboxRelay -->|"Polls unsent outbox rows"| DB
    OutboxRelay -->|"EventPublished / EventUpdated (key = event_id)"| Kafka
    Kafka --> BookingExt
    Kafka --> SearchExt

    %% Domain usage
    EventSrv -.-> Models
    EventRepo -.-> Models
```

## Layer & Component Responsibilities

1. **API & Middleware Layer (`src/api`, `src/openapi.rs`)**
   - Exposes REST endpoints under `/api/v1/events` (with `/events` as a compatibility alias), `/health`, `/swagger-ui`, and `/api-docs/openapi.json`.
   - Applies cross-cutting HTTP middleware (`CorsLayer`, `TraceLayer`, and an Axum middleware that cuts each request at `HTTP_REQUEST_TIMEOUT_MS` with `503` and `Retry-After`, per ADR-008).
   - `AuthenticatedPartner` (`src/api/auth.rs`) is an Axum `FromRequestParts` extractor—not a standalone HTTP endpoint—that parses `Authorization: Bearer <token>` and delegates to `AuthService`.

2. **Service Layer (`src/service`)**
   - Owns all business validation, lifecycle transitions, ownership checks, and external client orchestration. Contains no HTTP types.
   - `EventService`: Orchestrates event creation, edits, venue/zone pricing snapshots, sales-window rules, scheduling, and lock enforcement.
   - `AuthService` & `TokenVerifier`: Verifies bearer tokens locally. Per amended ADR-007 and ADR-008, production verification uses `HS256` with the shared `JWT_SECRET` (no runtime network call to Auth Service). There are no mock tokens in the target design (ADR-007, amended 2026-11-27); `MockTokenVerifier` exists only until the verifier is replaced.
   - `VenueClient`: Trait (`HttpVenueClient` / `MockVenueClient`) communicating with Venue Service via `reqwest`. Calls `GET /venues/{id}` forwarding the organizer's bearer token to validate venues and snapshot zone metadata, protected by per-attempt timeout, full-jitter retries, and a circuit breaker (ADR-008).

3. **Background Workers (Planned — ADR-003, ADR-004, ADR-008, ADR-009)**
   - **Publish Scheduler:** Runs every `EVENT_PUBLISH_JOB_INTERVAL_SECONDS` against PostgreSQL only (no token or external calls). Marks `Scheduled` events as published once `sale_starts_at - EVENT_VISIBILITY_LEAD_DAYS <= now()`, writes `EventPublished` to the outbox in the same transaction, and purges expired `idempotency_keys`.
   - **Outbox Relay:** Polls unsent outbox rows every `OUTBOX_RELAY_INTERVAL_MS` in `id` order and publishes full-state `EventPublished` / `EventUpdated` messages to Kafka topic `events.lifecycle` (idempotent producer, `acks=all`, keyed by `event_id`). Every instance runs it, but a transaction-level Postgres advisory lock lets only one send at a time (ADR-009).

4. **Repository Layer (`src/repository`)**
   - Trait-backed data access (`EventRepository`, `PartnerRepository`) with SeaORM PostgreSQL implementations and in-memory `Mock*` implementations for tests.
   - Enforces transactional writes, atomic DB-clock locking (`WHERE status = 'Draft' OR sale_starts_at > now()`, ADR-002), and outbox inserts in the same transaction as event updates (ADR-004).

5. **Domain & Model Layer (`src/model`)**
   - Defines SeaORM entities and API/Kafka DTOs (`dtos.rs`).
   - **No seat storage:** Seats belong to Venue Service. Event Service snapshots only venue zones (`venue_zone_id`, copied `name`, and `price` as `i64` thousandths of USD per ADR-001 and ADR-005). The legacy `event_seats` table is unused and scheduled to be dropped.

6. **External Systems**
   - **PostgreSQL (`event_store`):** Sole persistent database owned by this microservice.
   - **Auth Service:** Issues `HS256` JWTs to clients (`sub`, `role`, `iat`, `exp`). Event Service never calls it during request handling.
   - **Venue Management Service:** Source of truth for venues, zones, and seats.
   - **Kafka (`events.lifecycle`):** Event stream consumed independently by **Booking Service** (`booking-service`) and **Search Service** (`search-service`).
