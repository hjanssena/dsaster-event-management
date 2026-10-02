use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

/// Serves Swagger UI and the same generated specification used by generate_openapi.
pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Event Management API",
        description = r#"Browse events and create events for an authenticated organizer.

### Try a request

1. Expand an endpoint and select **Try it out**.
2. Fill in its parameters or edit the JSON request body.
3. Select **Execute**. Swagger sends a real request to this service.
4. Read the response status and body below the form. A successful POST saves a new event.

### Authenticate to create an event

GET requests and the health check are public. **POST /api/v1/events** requires an organizer token.

1. Select **Authorize** at the top of this page.
2. In the `bearer_auth` value, enter your token **without the `Bearer ` prefix**.
3. Select **Authorize**, then **Close**. Swagger adds `Authorization: Bearer <token>` to protected requests.
4. To switch accounts, open **Authorize**, select **Logout**, and enter another token.

**Current development authentication:** this service accepts `mock:<partner UUID>` tokens. With the default `MOCK_AUTH=true`, use this organizer token:

`mock:11111111-1111-1111-1111-111111111111`

To try a role rejection (HTTP 403), use the venue owner token:

`mock:22222222-2222-2222-2222-222222222222`

These are development tokens without signatures or expiration. There is no login endpoint in this service. When `MOCK_AUTH=false`, the verifier still expects a mock token, but the partner must exist in the database with the organizer role.

### Create an event

Required fields: `name`, `artist`, `date`, and `venue_id`. Use a future date with a timezone. The server gets the organizer from your token and sets the initial status to `Scheduled`.

```json
{
  "name": "Summer music festival",
  "artist": "The Example Band",
  "date": "2099-07-15T20:00:00Z",
  "venue_id": "33333333-3333-3333-3333-333333333333",
  "description": "An evening of live music",
  "age_policy": "All ages",
  "event_type": "Concert",
  "terms": "Tickets are required for entry."
}
```

Replace the example values with your event details. `venueId` is also accepted for `venue_id`. With the default `MOCK_VENUE_SERVICE=true`, any nonzero venue UUID is accepted. When it is false, use a venue registered in Venue Management.

### Understand the fields

Select **Schema** beside **Example Value** to read each request or response field description. You can also expand a model under **Schemas** at the bottom of this page. Fields marked as required must be included. Optional request fields can be omitted or set to `null`. Dates use RFC 3339 with a timezone, such as `2099-07-15T20:00:00Z`; identifiers use UUIDs.

### Understand errors

- **400:** check that the name and artist are nonempty and the event date is in the future. Malformed path or query values can also return 400.
- **401:** authorize with a token in the expected format for an existing partner.
- **403:** use a partner with the organizer role.
- **404:** check that the requested event ID exists.
- **422:** use a valid venue ID accepted by Venue Management.
- **500:** the service could not complete the request; check the service logs.

Application errors return an `error` message. Request parsing failures can use a different response format.
"#
    ),
    paths(
        crate::api::health_api::health_check,
        crate::api::event_api::get_event_by_id,
        crate::api::event_api::get_events,
        crate::api::event_api::create_event,
    ),
    components(
        schemas(
            crate::model::dtos::CreateEventRequestDto,
            crate::model::dtos::EventConfirmationDto,
            crate::model::dtos::EventResponseDto,
            crate::model::dtos::EventSummaryDto,
            crate::model::dtos::PaginatedEventSummaryResponse,
            crate::model::dtos::EventScheduleDto,
            crate::model::dtos::EventPricingTierDto,
            crate::model::dtos::EventSaleDto,
            crate::model::dtos::EventMediaDto,
            crate::core::error::ErrorResponseDto,
        )
    ),
    modifiers(&BearerAuth),
    tags(
        (name = "events", description = "Endpoints de consulta y gestión de eventos"),
        (name = "health", description = "Monitoreo y disponibilidad del servicio")
    )
)]
pub struct ApiDoc;

/// Registra el esquema de seguridad Bearer usado por los endpoints de registro
struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}
