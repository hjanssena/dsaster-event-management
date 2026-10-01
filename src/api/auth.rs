use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::core::error::AppError;
use crate::model::partner;
use crate::AppState;

/// Extractor de Axum: partner autenticado a partir del header `Authorization: Bearer <token>`.
/// Rechaza con 401 si el token falta, es inválido o el partner no existe.
pub struct AuthenticatedPartner(pub partner::Model);

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedPartner {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts)
            .ok_or_else(|| AppError::Unauthorized("Missing bearer token".to_string()))?;

        let partner = state.auth_service.authenticate(token).await?;
        Ok(AuthenticatedPartner(partner))
    }
}

/// Obtiene el token del header Authorization (el esquema "Bearer" no distingue mayúsculas)
fn bearer_token(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();

    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}
