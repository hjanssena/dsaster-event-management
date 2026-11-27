//! Request extractors that turn every rejection into a JSON `400` (ADR-010).
//!
//! Axum's own `Json`, `Path` and `Query` answer `400`, `415` or `422` with a
//! plain-text body. Handlers use these wrappers instead, so a request the
//! service cannot parse always gets `400` with `{"error": "..."}`.

use axum::{
    extract::{
        FromRequest, FromRequestParts,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::core::error::AppError;

/// JSON body. Invalid JSON, a wrong type, a missing field or a missing
/// `Content-Type: application/json` get a JSON `400`.
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct ApiJson<T>(pub T);

/// Path parameters. A malformed value (such as a bad UUID) gets a JSON `400`.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct ApiPath<T>(pub T);

/// Query parameters. A malformed value gets a JSON `400`.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct ApiQuery<T>(pub T);

impl<T: Serialize> IntoResponse for ApiJson<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        AppError::BadRequest(rejection.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        AppError::BadRequest(rejection.body_text())
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        AppError::BadRequest(rejection.body_text())
    }
}
