use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

use eventManagement_api::core::error::AppError;
use eventManagement_api::service::{HttpVenueClient, VenueClient};

#[derive(Clone)]
struct MockVenueServerState {
    expected_token: String,
    existing_venues: Arc<HashSet<Uuid>>,
    forbidden_venue: Uuid,
    error_venue: Uuid,
}

async fn handle_get_venue(
    State(state): State<MockVenueServerState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> StatusCode {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    if auth != Some(&format!("Bearer {}", state.expected_token)) {
        return StatusCode::UNAUTHORIZED;
    }

    if id == state.forbidden_venue {
        return StatusCode::FORBIDDEN;
    }
    if id == state.error_venue {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if state.existing_venues.contains(&id) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn spawn_mock_venue_server(
    expected_token: String,
    existing_venues: HashSet<Uuid>,
    forbidden_venue: Uuid,
    error_venue: Uuid,
) -> String {
    let state = MockVenueServerState {
        expected_token,
        existing_venues: Arc::new(existing_venues),
        forbidden_venue,
        error_venue,
    };
    let app = Router::new()
        .route("/venues/:id", get(handle_get_venue))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{}", addr)
}

#[tokio::test]
async fn test_http_venue_client_verifies_existing_and_missing_venues_with_bearer_token() {
    let valid_venue = Uuid::new_v4();
    let missing_venue = Uuid::new_v4();
    let forbidden_venue = Uuid::new_v4();
    let error_venue = Uuid::new_v4();
    let token = "valid-venue-jwt".to_string();

    let base_url = spawn_mock_venue_server(
        token.clone(),
        HashSet::from([valid_venue]),
        forbidden_venue,
        error_venue,
    )
    .await;

    let client = HttpVenueClient::with_token(base_url.clone(), Some(token));
    assert!(client.verify_venue_exists(valid_venue).await.unwrap());
    assert!(!client.verify_venue_exists(missing_venue).await.unwrap());

    let forbidden_err = client
        .verify_venue_exists(forbidden_venue)
        .await
        .unwrap_err();
    assert!(matches!(forbidden_err, AppError::InternalServerError(_)));

    let upstream_err = client.verify_venue_exists(error_venue).await.unwrap_err();
    assert!(matches!(upstream_err, AppError::InternalServerError(_)));

    let unauthenticated_client = HttpVenueClient::new(base_url);
    let auth_err = unauthenticated_client
        .verify_venue_exists(valid_venue)
        .await
        .unwrap_err();
    assert!(matches!(auth_err, AppError::InternalServerError(_)));
}

#[tokio::test]
async fn test_http_venue_client_connection_error_returns_internal_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let client = HttpVenueClient::new(format!("http://{}", addr));
    let err = client
        .verify_venue_exists(Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InternalServerError(_)));
}
