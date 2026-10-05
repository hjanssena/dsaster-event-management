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
- **OpenAPI JSON Spec:** [http://localhost:3000/api-docs/openapi.json](http://localhost:3000/api-docs/openapi.json)

---

## API Documentation

With the service running (default port `3000`):

- **Swagger UI:** http://localhost:3000/swagger-ui/
- **OpenAPI JSON:** http://localhost:3000/api-docs/openapi.json

Both endpoints are public. Swagger UI supports trying API requests and supplying a bearer token through **Authorize** for event creation.

The served specification is generated from the Utoipa annotations at startup. To export the same specification as `openapi.yml`, run:

```sh
cargo run --bin generate_openapi
```

---

## Working with AI agents (OpenSpec)

We use [OpenSpec](https://github.com/Fission-AI/OpenSpec) for spec-driven development: no code before a spec you have reviewed. Agent conventions are in [AGENTS.md](AGENTS.md).

Install the CLI once: `npm install -g @fission-ai/openspec@latest`

Per ticket, on its branch:

1. `/opsx:propose "VE05T1: <ticket title>"` creates `openspec/changes/<change>/` (proposal, delta specs, tasks). Review it before continuing.
2. `/opsx:apply` implements the tasks, one commit each.
3. Open the PR with the change folder included.
4. After merge, `/opsx:archive` merges the deltas into `openspec/specs/`.

Agents other than Claude Code use the matching skills in `.agents/skills/` (e.g. `/openspec-propose`).

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
