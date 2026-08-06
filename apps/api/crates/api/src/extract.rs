//! Extracteur `CurrentUser` : présence dans la signature d'un handler = route
//! protégée ; `Option<CurrentUser>` pour les routes mixtes.

use std::convert::Infallible;

use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use uuid::Uuid;

use crate::auth::cookies::access_cookie_name;
use crate::auth::jwt::decode_access;
use crate::error::ApiError;
use crate::AppState;

#[derive(Debug, Clone, Copy)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(access_cookie_name(state.config.cookie_secure))
            .or_else(|| jar.get("lbt_access"))
            .map(|c| c.value().to_owned())
            .ok_or_else(|| ApiError::unauthorized("Connecte-toi pour continuer."))?;
        let (user_id, session_id) = decode_access(&state.config, &token)
            .ok_or_else(|| ApiError::unauthorized("Ta session a expiré. Reconnecte-toi."))?;
        Ok(CurrentUser {
            user_id,
            session_id,
        })
    }
}

/// Accès au panneau d'administration. Deux chemins :
///
/// 1. **la session** d'un utilisateur dont le rôle est `admin` ou
///    `super_admin` — c'est la voie normale, celle qui trace l'auteur ;
/// 2. **la clé de service** `X-Admin-Token` (exploitation, scripts) : elle
///    vaut super-admin mais n'a pas d'identité, et le journal l'inscrit
///    comme « service ».
///
/// Une basic auth Traefik protège le chemin `/admin` par-dessus.
#[derive(Debug, Clone)]
pub struct AdminActor {
    /// `None` pour la clé de service.
    pub user_id: Option<Uuid>,
    pub role: String,
    pub is_master: bool,
}

impl AdminActor {
    /// Vérifie le niveau super-admin, sinon 403.
    pub fn require_super(&self) -> Result<(), ApiError> {
        if domain::admin::est_super_admin(&self.role) {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "super_admin_requis",
                "Cette action est réservée aux super-administrateurs.",
            ))
        }
    }
}

/// La clé de service a-t-elle été présentée, et est-elle la bonne ?
fn cle_de_service_valide(parts: &Parts, attendu: &str) -> bool {
    let fourni = parts
        .headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    // Comparaison en temps constant : pas d'oracle sur la clé.
    fourni.len() == attendu.len()
        && fourni
            .bytes()
            .zip(attendu.as_bytes().iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

impl FromRequestParts<AppState> for AdminActor {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if cle_de_service_valide(parts, &state.config.admin_token) {
            return Ok(AdminActor {
                user_id: None,
                role: domain::admin::ROLE_SUPER_ADMIN.to_string(),
                is_master: false,
            });
        }
        // Sans session : 401 (identifie-toi). Session sans le rôle : 403.
        let user =
            <CurrentUser as FromRequestParts<AppState>>::from_request_parts(parts, state).await?;
        let refus = || ApiError::forbidden("acces_refuse", "Accès réservé à l'administration.");
        let info = infra::admin_repo::role_of(&state.pool, user.user_id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(refus)?;
        if !domain::admin::peut_administrer(&info.role) {
            return Err(refus());
        }
        Ok(AdminActor {
            user_id: Some(user.user_id),
            role: info.role,
            is_master: info.is_master,
        })
    }
}

impl OptionalFromRequestParts<AppState> for CurrentUser {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <CurrentUser as FromRequestParts<AppState>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}
