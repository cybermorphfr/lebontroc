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

/// Accès aux endpoints d'administration (F5.2) : header `X-Admin-Token`
/// comparé au token d'environnement — doublé d'une basic auth Traefik sur
/// le chemin. Un vrai rôle en base attendra le back-office (F6.1).
#[derive(Debug, Clone, Copy)]
pub struct AdminAuth;

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let provided = parts
            .headers
            .get("x-admin-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        // Comparaison en temps constant : pas d'oracle sur le token.
        let expected = state.config.admin_token.as_bytes();
        let ok = provided.len() == expected.len()
            && provided
                .bytes()
                .zip(expected.iter())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0;
        if !ok {
            return Err(ApiError::unauthorized("Accès réservé à l'administration."));
        }
        Ok(AdminAuth)
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
