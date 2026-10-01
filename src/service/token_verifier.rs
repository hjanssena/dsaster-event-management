use async_trait::async_trait;
use uuid::Uuid;

use crate::core::error::AppError;

/// Trait para verificar el token del partner y obtener su identificador (claim `sub`)
#[async_trait]
pub trait TokenVerifier: Send + Sync {
    /// Verifica el token y retorna el ID del partner. Un token inválido retorna AppError::Unauthorized.
    async fn verify(&self, token: &str) -> Result<Uuid, AppError>;
}

/// Prefijo de los tokens simulados: `mock:<uuid-del-partner>`
pub const MOCK_TOKEN_PREFIX: &str = "mock:";

/// Verificador simulado mientras el servicio de autenticación define el JWT (algoritmo, claims, llaves).
/// Acepta tokens con formato `mock:<uuid>` sin firma ni expiración.
#[derive(Clone, Default)]
pub struct MockTokenVerifier;

impl MockTokenVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Construye un token simulado para un partner (útil en pruebas y desarrollo local)
    pub fn token_for(partner_id: Uuid) -> String {
        format!("{}{}", MOCK_TOKEN_PREFIX, partner_id)
    }
}

#[async_trait]
impl TokenVerifier for MockTokenVerifier {
    async fn verify(&self, token: &str) -> Result<Uuid, AppError> {
        token
            .strip_prefix(MOCK_TOKEN_PREFIX)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| AppError::Unauthorized("Invalid token".to_string()))
    }
}
