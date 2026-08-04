//! Module authentification : routes, cookies, JWT, hashing, tokens opaques.

pub mod cookies;
pub mod dto;
pub mod handlers;
pub mod jwt;
pub mod password;
pub mod tokens;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(handlers::signup))
        .route("/auth/login", post(handlers::login))
        .route("/auth/refresh", post(handlers::refresh))
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/verify-email", get(handlers::verify_email))
        .route(
            "/auth/resend-verification",
            post(handlers::resend_verification),
        )
        .route(
            "/auth/sessions",
            get(handlers::list_sessions).delete(handlers::revoke_other_sessions),
        )
        .route("/auth/sessions/{id}", delete(handlers::revoke_session))
        .route("/me", get(handlers::me).patch(handlers::update_me))
        .route("/analytics/track", post(handlers::track_event))
}
