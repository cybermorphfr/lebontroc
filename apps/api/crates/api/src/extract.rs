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
