# Guía de Desarrollo Local

Esta guía detalla la forma más eficiente y rápida de trabajar en el código del proyecto día a día.

## El Flujo Recomendado (Híbrido)

Para programar de forma ágil, la regla de oro es: **Docker se encarga de la base de datos, y tu computadora compila la API.**
Si compilas todo dentro de Docker en cada cambio, se desperdicia mucho tiempo.

### Pasos para iniciar

1. **Levantar solo la Base de Datos:**
   Abre una terminal y ejecuta:

   ```bash
   docker-compose up -d event-db
   ```

   *Esto levanta a PostgreSQL en segundo plano. No levantará el contenedor de la API.*

2. **Verificar tu archivo `.env`:**
   Asegúrate de que el archivo `.env` apunte a `localhost` para conectarse a la base de datos que acaba de levantar Docker:

   ```env
   DATABASE_URL=postgres://usuario:password@localhost:5432/event_store
   SERVER_ADDR=0.0.0.0:3000
   ```

3. **Arrancar tu servidor local:**
   Ejecuta el código directamente con Rust:

   ```bash
   cargo run
   ```

   *(Si usas `cargo watch -x run`, el servidor se reiniciará automáticamente cada vez que presiones "Guardar" en tu editor).*

4. **Listo** código respondiendo en `http://localhost:3000`.

5. **Probar** vida de los servicios con `http://localhost:3000/health`

---

## Linting y formato de código

Antes de abrir un Pull Request, ejecuta las mismas verificaciones de formato y linting que usa CI:

```bash
npm run lint
```

Este comando ejecuta `cargo fmt --all -- --check` y `cargo lint`. El alias
`cargo lint` ejecuta Clippy en todos los targets (incluyendo binarios y tests)
con `--locked` y trata todas las advertencias como errores.

También puedes ejecutar las verificaciones directamente con Cargo, sin Node.js:

```bash
cargo fmt --all -- --check
cargo lint
```

Para corregir el formato automáticamente, usa `cargo fmt --all` o `npm run format`.
Si faltan las herramientas, instálalas con `rustup component add rustfmt clippy`.

El workflow `validation` ejecuta estas verificaciones en el job `lint`, además
del job de tests, en pushes y Pull Requests hacia `main` y `testing`.

---

## Autenticación Simulada (MOCK_AUTH)

Mientras el servicio de autenticación define el JWT, los endpoints de registro (`POST /events`) usan tokens simulados con formato `mock:<uuid-del-partner>`:

```bash
curl -X POST http://localhost:3000/api/v1/events \
  -H "Authorization: Bearer mock:11111111-1111-1111-1111-111111111111" \
  -H "Content-Type: application/json" \
  -d '{"name": "Concierto", "artist": "Artista", "date": "2026-12-01T21:00:00Z", "venue_id": "<uuid>"}'
```

Con `MOCK_AUTH=true` (valor por defecto) se precargan en memoria dos partners de desarrollo:

| Partner | ID | Rol |
| --- | --- | --- |
| `dev_organizer` | `11111111-1111-1111-1111-111111111111` | `organizer` |
| `dev_venue_owner` | `22222222-2222-2222-2222-222222222222` | `venue_owner` |

Con `MOCK_AUTH=false` los partners se consultan en la tabla `partners` de la base de datos (el token sigue siendo simulado).

Respuestas esperadas: sin token o con token inválido → `401`; partner inexistente → `401`; venue owner → `403`.

---

## Flujo Full Docker (Solo para Testing Final)

Si necesitas simular cómo se comportará la aplicación en un servidor de Producción, puedes meter ambos componentes (Base de Datos y API) dentro de Docker.

1. **Asegúrate de liberar el puerto:**
   Detén cualquier `cargo run` que tengas en tu terminal presionando `Ctrl + C`.

2. **Construye y Levanta la infraestructura completa:**
   Ejecuta el siguiente comando. Es vital agregar `--build` para que Docker sepa que debe leer tu código nuevo y re-compilar la imagen.

   ```bash
   docker-compose up -d --build
   ```

---

## Errores Comunes

* **Error: `Address already in use` (Puerto 3000 ocupado)**
  *Por qué ocurre:* Estás intentando usar `cargo run` y `docker-compose up` al mismo tiempo. Solo uno puede usar el puerto 3000.
  *Solución:* Decide cuál quieres usar. Si quieres usar el local, apaga el de Docker ejecutando `docker-compose stop event-api`.

* **Error de conexión a la Base de Datos (`Connection refused`)**
  *Por qué ocurre:* Tu código local está intentando buscar una base de datos y no la encuentra.
  *Solución:* Asegúrate de que encendiste el contenedor con `docker-compose up -d event-db` y que las contraseñas del `docker-compose.yml` coincidan con las de tu archivo `.env`.
