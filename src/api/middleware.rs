//! Request guard (ADR-008): the incoming request time limit, the default
//! `Retry-After` on every `503`, and the request deadline for outgoing calls.
//!
//! `tower_http::timeout::TimeoutLayer` is not used: it answers `408` with an
//! empty body, not `503` with `Retry-After` and the JSON error body.

use std::time::Duration;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use tokio::time::Instant;

use crate::config::HttpConfig;
use crate::core::error::AppError;

tokio::task_local! {
    /// Arrival time plus `HTTP_REQUEST_TIMEOUT_MS`, while a request is handled.
    static REQUEST_DEADLINE: Instant;
}

#[derive(Clone, Copy, Debug)]
struct GuardConfig {
    request_timeout: Duration,
    retry_after_seconds: u64,
}

/// Wraps a router in the request guard. `api::build_router` uses it for the
/// service; tests use it to wrap test-only routes in the same middleware.
pub fn apply<S>(router: Router<S>, http: &HttpConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let config = GuardConfig {
        request_timeout: http.request_timeout,
        retry_after_seconds: http.retry_after_seconds,
    };
    router.layer(middleware::from_fn_with_state(config, request_guard))
}

/// The current request's deadline, or `None` outside a request (for example in
/// a background job). Outgoing calls stop retrying when the time left is
/// shorter than one attempt (ADR-008).
pub fn request_deadline() -> Option<Instant> {
    REQUEST_DEADLINE.try_with(|deadline| *deadline).ok()
}

async fn request_guard(
    State(config): State<GuardConfig>,
    request: Request,
    next: Next,
) -> Response {
    let deadline = Instant::now() + config.request_timeout;

    // On timeout the handler's future is dropped, so any open transaction rolls back.
    let mut response = match tokio::time::timeout_at(
        deadline,
        REQUEST_DEADLINE.scope(deadline, next.run(request)),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => AppError::service_unavailable("Request timed out").into_response(),
    };

    if response.status() == StatusCode::SERVICE_UNAVAILABLE
        && !response.headers().contains_key(RETRY_AFTER)
    {
        response
            .headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from(config.retry_after_seconds));
    }
    response
}
