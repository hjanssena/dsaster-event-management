use crate::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};

/// Endpoint de Health Check
/// Se utiliza para monitoreo (ej. Kubernetes, Docker Healthcheck, balanceadores de carga).
/// Devuelve un HTTP 200 OK con un JSON confirmando que el servicio está vivo.

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (
            status = 200,
            description = "The service is up and responding correctly", 
            body = Value,
            example = json!({
                "status": "ok",
                "message": "Event Management API is running normally",
                "version": "0.1.0"
            })
        )
    )
)]

pub async fn health_check(State(_state): State<AppState>) -> Json<Value> {
    // Aquí en el futuro se podría agregar lógica para verificar si la DB responde.
    // Por ahora, si Axum puede procesar esta petición, significa que el servicio está vivo.
    Json(json!({
        "status": "ok",
        "message": "Event Management API is running normally",
        "version": "0.1.0"
    }))
}
