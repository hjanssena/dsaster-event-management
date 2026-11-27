pub mod auth;
pub mod event_api;
pub mod extract;
pub mod health_api;
pub mod middleware;

use axum::{Router, routing::get};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::AppState;
use crate::config::HttpConfig;
use crate::openapi;

/// The service's router with its middleware. `main.rs` and the tests both use it,
/// so the tests exercise the same request guard, tracing and CORS as production.
pub fn build_router(state: AppState, http: &HttpConfig) -> Router {
    let routes = Router::new()
        .merge(openapi::swagger_ui())
        .route("/health", get(health_api::health_check))
        .nest("/api/v1/events", event_api::routes())
        .nest("/events", event_api::routes()) // Compatibility alias: no new routes here
        .with_state(state);

    // Layers added later wrap earlier ones: CORS wraps everything, including a
    // timeout response from the request guard.
    middleware::apply(routes, http)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
