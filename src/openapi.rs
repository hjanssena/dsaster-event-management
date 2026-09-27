use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::health_api::health_check, // <-- Poner directamente health_check
    ),
    components(
        schemas()
    ),
    tags(
        (name = "eventManagement_api", description = "Event Management API Endpoints")
    )
)]
pub struct ApiDoc;