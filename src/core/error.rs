use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use sea_orm::{ConnAcquireErr, DbErr, RuntimeErr};
use serde_json::json;
use thiserror::Error;
use tracing::{error, warn};

/// SQLSTATE `query_canceled`, raised when a statement exceeds `statement_timeout`.
const SQLSTATE_QUERY_CANCELED: &str = "57014";

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DbErr),

    #[error("Internal server error")]
    InternalServerError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// `409`: the request conflicts with the current state, such as an event locked
    /// because its sales opened (C5).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// `503`: Event Service or a dependency cannot answer (ADR-008).
    /// `retry_after` is the wait in seconds when the cause knows it, such as an open
    /// circuit breaker; when `None`, the request middleware adds `HTTP_RETRY_AFTER_SECONDS`.
    #[error("Service unavailable: {message}")]
    ServiceUnavailable {
        message: String,
        retry_after: Option<u64>,
    },
}

impl AppError {
    /// A `503` whose wait is unknown; the middleware adds the default `Retry-After`.
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        AppError::ServiceUnavailable {
            message: message.into(),
            retry_after: None,
        }
    }
}

/// Whether a database error means the database could not answer in time or could not
/// be reached, as opposed to a bug or a rejected statement (design §2).
pub(crate) fn is_transient_db_error(err: &DbErr) -> bool {
    match err {
        DbErr::ConnectionAcquire(ConnAcquireErr::Timeout | ConnAcquireErr::ConnectionClosed) => {
            true
        }
        DbErr::Conn(_) => true,
        DbErr::Query(RuntimeErr::SqlxError(sqlx_err))
        | DbErr::Exec(RuntimeErr::SqlxError(sqlx_err)) => is_transient_sqlx_error(sqlx_err),
        _ => false,
    }
}

fn is_transient_sqlx_error(err: &sea_orm::sqlx::Error) -> bool {
    use sea_orm::sqlx::Error;
    match err {
        Error::Database(db_err) => db_err.code().as_deref() == Some(SQLSTATE_QUERY_CANCELED),
        Error::PoolTimedOut | Error::PoolClosed | Error::Io(_) => true,
        _ => false,
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, retry_after) = match self {
            AppError::DatabaseError(err) if is_transient_db_error(&err) => {
                warn!("Database unavailable: {:?}", err);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service temporarily unavailable".to_string(),
                    None,
                )
            }
            AppError::DatabaseError(err) => {
                error!("Database error occurred: {:?}", err);
                // No exponemos el error real de la BD al cliente por seguridad
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    None,
                )
            }
            AppError::InternalServerError(msg) => {
                error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    None,
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg, None),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg, None),
            AppError::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg, None),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg, None),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg, None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg, None),
            AppError::ServiceUnavailable {
                message,
                retry_after,
            } => {
                warn!("Service unavailable: {}", message);
                (StatusCode::SERVICE_UNAVAILABLE, message, retry_after)
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        let mut response = (status, body).into_response();
        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ErrorResponseDto {
    /// Message explaining why the request failed. Use the HTTP status to determine the error category.
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use sea_orm::sqlx::error::{DatabaseError, ErrorKind};
    use std::borrow::Cow;

    async fn parts(error: AppError) -> (StatusCode, Option<String>, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .map(|v| v.to_str().unwrap().to_string());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, retry_after, serde_json::from_slice(&body).unwrap())
    }

    /// A Postgres error carrying only a SQLSTATE, for classification tests.
    #[derive(Debug)]
    struct FakePgError(&'static str);

    impl std::fmt::Display for FakePgError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "fake postgres error {}", self.0)
        }
    }

    impl std::error::Error for FakePgError {}

    impl DatabaseError for FakePgError {
        fn message(&self) -> &str {
            "fake"
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(self.0))
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    fn pg_query_error(code: &'static str) -> DbErr {
        DbErr::Query(RuntimeErr::SqlxError(sea_orm::sqlx::Error::Database(
            Box::new(FakePgError(code)),
        )))
    }

    #[tokio::test]
    async fn conflict_is_409_with_json_body() {
        let (status, retry_after, body) = parts(AppError::Conflict("Event locked".into())).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(retry_after, None);
        assert_eq!(body["error"], "Event locked");
    }

    #[tokio::test]
    async fn service_unavailable_with_known_wait_sets_retry_after() {
        let (status, retry_after, body) = parts(AppError::ServiceUnavailable {
            message: "Venue Service unavailable".into(),
            retry_after: Some(12),
        })
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(retry_after.as_deref(), Some("12"));
        assert_eq!(body["error"], "Venue Service unavailable");
    }

    #[tokio::test]
    async fn service_unavailable_without_wait_leaves_header_to_middleware() {
        let (status, retry_after, _) = parts(AppError::service_unavailable("down")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(retry_after, None);
    }

    #[tokio::test]
    async fn pool_timeout_is_503() {
        let err = DbErr::ConnectionAcquire(ConnAcquireErr::Timeout);
        let (status, _, body) = parts(AppError::DatabaseError(err)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "Service temporarily unavailable");
    }

    #[tokio::test]
    async fn connection_failure_is_503() {
        let err = DbErr::Conn(RuntimeErr::Internal("connection refused".into()));
        let (status, _, body) = parts(AppError::DatabaseError(err)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(!body["error"].as_str().unwrap().contains("refused"));
    }

    #[tokio::test]
    async fn statement_timeout_is_503() {
        let (status, _, _) = parts(AppError::DatabaseError(pg_query_error("57014"))).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn other_database_errors_stay_500_without_details() {
        for err in [
            pg_query_error("23505"),
            DbErr::Custom("secret table name".into()),
            DbErr::RecordNotFound("events".into()),
        ] {
            let (status, retry_after, body) = parts(AppError::DatabaseError(err)).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(retry_after, None);
            assert_eq!(body["error"], "Internal server error");
        }
    }

    #[tokio::test]
    async fn not_found_uses_json_body() {
        let (status, _, body) = parts(AppError::NotFound("Event not found".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Event not found");
    }
}
