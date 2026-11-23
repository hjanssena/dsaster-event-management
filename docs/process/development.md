# Guía de Desarrollo Local

Esta guía detalla la forma más eficiente y rápida de trabajar en el código del proyecto día a día.

## El Flujo Recomendado (Híbrido)

Para programar de forma ágil, la regla de oro es: **Docker se encarga de la base de datos, y tu computadora compila la API.**
Si compilas todo dentro de Docker en cada cambio, se desperdicia mucho tiempo.

### Pasos para iniciar

1. **Levantar solo la Base de Datos:**
   Abre una terminal y ejecuta:

   ```bash
   docker compose up -d event-db
   ```

   *Esto levanta a PostgreSQL en segundo plano. No levantará el contenedor de la API.*

2. **Configurar tu archivo `.env`:**
   Copia la plantilla base y verifica que apunte a `localhost` para conectarse a la base de datos que levantó Docker:

   ```bash
   cp .env.example .env
   ```

   ```env
   DATABASE_URL=postgres://usuario:password@localhost:5432/event_store
   SERVER_ADDR=0.0.0.0:3000
   MOCK_AUTH=true
   MOCK_VENUE_SERVICE=true
   VENUE_SERVICE_URL=http://localhost:3001
   ```

   Consulta [`.env.example`](../../.env.example) y los ADRs ([ADR-006 / ADR-007 / ADR-008](../event-propagation/adrs-event-propagation.md)) para ver el resto de las variables de configuración.

3. **Arrancar tu servidor local:**
   Ejecuta el código directamente con Rust:

   ```bash
   cargo run
   ```

   *(Si usas `cargo watch -x run`, el servidor se reiniciará automáticamente cada vez que guardes cambios en tu editor).*

4. **Verificar el servicio:**
   - Health check: `http://localhost:3000/health`
   - Swagger UI: `http://localhost:3000/swagger-ui`
   - OpenAPI JSON: `http://localhost:3000/api-docs/openapi.json`

---

## Pruebas, linting y contrato OpenAPI

Antes de abrir un Pull Request, ejecuta las mismas verificaciones que corre CI (`.github/workflows/validation.yml`):

```bash
npm run lint
cargo test
```

`npm run lint` ejecuta `cargo fmt --all -- --check` y `cargo lint` (alias de Clippy en todos los targets con `--locked` y `-D warnings`). Las pruebas usan mocks en memoria y no requieren levantar PostgreSQL.

También puedes ejecutar las verificaciones directamente con Cargo, sin Node.js:

```bash
cargo fmt --all -- --check
cargo lint
cargo test
```

Para corregir el formato automáticamente, usa `cargo fmt --all` o `npm run format`.
Si faltan las herramientas, instálalas con `rustup component add rustfmt clippy`.

Si agregas o modificas endpoints o DTOs, regenera el archivo `openapi.yml` y haz commit del cambio junto con el código:

```bash
cargo run --bin generate_openapi
```

---

## Autenticación Simulada (`MOCK_AUTH`)

El contrato objetivo de autenticación usa JWT firmados con `HS256` y el secreto compartido `JWT_SECRET` verificados localmente (ver [ADR-007](../event-propagation/adrs-event-propagation.md#adr-007---the-backend-validates-every-identity-role-and-session)).

Mientras `MockTokenVerifier` siga activo para desarrollo local y pruebas (`MOCK_AUTH=true`), los endpoints protegidos (como `POST /api/v1/events`) aceptan tokens simulados con formato `mock:<uuid-del-partner>`:

```bash
curl -X POST http://localhost:3000/api/v1/events \
  -H "Authorization: Bearer mock:11111111-1111-1111-1111-111111111111" \
  -H "Content-Type: application/json" \
  -d '{"name": "Concierto", "artist": "Artista", "date": "2099-12-01T21:00:00Z", "venue_id": "33333333-3333-3333-3333-333333333333"}'
```

Con `MOCK_AUTH=true` (valor por defecto en local) se precargan en memoria dos partners de desarrollo:

| Partner | ID | Rol |
| --- | --- | --- |
| `dev_organizer` | `11111111-1111-1111-1111-111111111111` | `organizer` |
| `dev_venue_owner` | `22222222-2222-2222-2222-222222222222` | `venue_owner` |

En la implementación actual, con `MOCK_AUTH=false` los partners se consultan en la tabla `partners` de PostgreSQL (hasta reemplazar `MockTokenVerifier` por el verificador JWT `HS256` de ADR-007).

Respuestas esperadas: sin token o con token inválido → `401`; partner inexistente → `401`; venue owner → `403`. Para el contrato objetivo de todos los endpoints, consulta [contract-event-service.md](../event-propagation/contract-event-service.md).

---

## Flujo Full Docker (Solo para Testing Final)

Si necesitas simular cómo se comportará la aplicación en un servidor de Producción, puedes correr ambos componentes (Base de Datos y API) dentro de Docker.

1. **Asegúrate de liberar el puerto:**
   Detén cualquier `cargo run` que tengas en tu terminal presionando `Ctrl + C`.

2. **Construye y levanta la infraestructura completa:**
   Ejecuta el siguiente comando. Es vital agregar `--build` para que Docker lea tu código nuevo y re-compile la imagen.

   ```bash
   docker compose up -d --build
   ```

---

## Errores Comunes

* **Error: `Address already in use` (Puerto 3000 ocupado)**
  *Por qué ocurre:* Estás intentando usar `cargo run` y `docker compose up` al mismo tiempo. Solo uno puede usar el puerto 3000.
  *Solución:* Decide cuál quieres usar. Si quieres usar el local, apaga el de Docker ejecutando `docker compose stop event-api`.

* **Error de conexión a la Base de Datos (`Connection refused`)**
  *Por qué ocurre:* Tu código local está intentando buscar una base de datos y no la encuentra.
  *Solución:* Asegúrate de que encendiste el contenedor con `docker compose up -d event-db` y que las contraseñas del `docker-compose.yml` coincidan con las de tu archivo `.env`.
