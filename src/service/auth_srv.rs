use std::sync::Arc;

use crate::core::error::AppError;
use crate::model::partner;
use crate::repository::PartnerRepository;
use crate::service::TokenVerifier;

/// Capa de autenticación: resuelve el token del partner a un partner existente
#[derive(Clone)]
pub struct AuthService {
    token_verifier: Arc<dyn TokenVerifier>,
    partner_repo: Arc<dyn PartnerRepository>,
}

impl AuthService {
    pub fn new(
        token_verifier: Arc<dyn TokenVerifier>,
        partner_repo: Arc<dyn PartnerRepository>,
    ) -> Self {
        Self {
            token_verifier,
            partner_repo,
        }
    }

    /// Verifica el token y valida que el partner exista (VE-08).
    /// La identidad se obtiene solo del token, nunca del body de la petición.
    pub async fn authenticate(&self, token: &str) -> Result<partner::Model, AppError> {
        let partner_id = self.token_verifier.verify(token).await?;

        self.partner_repo
            .find_by_id(partner_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Partner not found".to_string()))
    }
}
