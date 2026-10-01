use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::health_api::health_check,
        crate::api::event_api::get_event_by_id,
        crate::api::event_api::get_events,
    ),
    components(
        schemas(
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
    tags(
        (name = "events", description = "Endpoints de consulta y gestión de eventos"),
        (name = "health", description = "Monitoreo y disponibilidad del servicio")
    )
)]
pub struct ApiDoc;