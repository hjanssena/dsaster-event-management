use axum::{extract::State, Json};
use serde_json::{json, Value};
use crate::AppState;

/// Endpoint de Health Check
/// Se utiliza para monitoreo (ej. Kubernetes, Docker Healthcheck, balanceadores de carga).
/// Devuelve un HTTP 200 OK con un JSON confirmando que el servicio está vivo.
pub async fn health_check(
    State(_state): State<AppState>,
) -> Json<Value> {
    // Aquí en el futuro se podría agregar lógica para verificar si la DB responde.
    // Por ahora, si Axum puede procesar esta petición, significa que el servicio está vivo.
    Json(json!({
        "status": "ok",
        "message": "Event Management API is running normally",
        "version": "0.1.0"
    }))
}

