# D-saster - Event Management Microservice

**Short Description:**
Welcome to the **Event Management Microservice** for **Dsaster**. This service owns the `EventManagement` sub-domain. It is responsible for the complete lifecycle of events—including event creation, venue assignment, seat mapping configuration, and sale scheduling. It provides the foundational event data needed by the high-concurrency sales platform to deliver reliable ticketing experiences.

---

## 🚀 Running Locally

Follow these exact steps to go from a fresh clone to a running service on your local machine. A PostgreSQL database is required to run the service.

### 1. Prerequisites
- **Rust & Cargo**
- **Docker** or **Podman** (to run the local database)

### 2. Environment Configuration
Create your environment variables file:
```bash
cp .env.example .env
```
Ensure your `.env` contains the local database URL:
```env
SERVER_ADDR=0.0.0.0:3000
DATABASE_URL=postgres://usuario:password@localhost:5432/event_store
```

### 3. Start the Database
Spin up the PostgreSQL database container.

**If using Docker:**
```bash
docker compose up -d event-db
```

**If using Podman** (and experiencing `pasta` rootless networking errors), use this direct command:
```bash
podman run -d --name event-db -p 5432:5432 \
  -e POSTGRES_USER=usuario -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=event_store \
  --network slirp4netns docker.io/library/postgres:15-alpine
```

### 4. Build and Run
Start the service:
```bash
cargo run
```
If running dockerized:
```bash
docker compose up -d
```
*(The service will automatically run database migrations and seed test data on startup).*

### 5. Verify the API
- **Health Check:** `curl http://localhost:3000/health`
- **Swagger UI (Interactive Docs):** [http://localhost:3000/swagger-ui](http://localhost:3000/swagger-ui)
- **OpenAPI JSON Spec:** [http://localhost:3000/api-doc/openapi.json](http://localhost:3000/api-doc/openapi.json)

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
