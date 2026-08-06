//! Module authentification : routes, cookies, JWT, hashing, tokens opaques.

pub mod cookies;
pub mod dto;
pub mod handlers;
pub mod jwt;
pub mod password;
pub mod tokens;
pub mod totp;

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
        .route(
            "/me",
            get(handlers::me)
                .patch(handlers::update_me)
                .delete(handlers::delete_my_account),
        )
        .route("/me/export", get(handlers::export_my_data))
        .route("/me/totp", get(totp::totp_status))
        .route("/me/totp/start", post(totp::totp_start))
        .route("/me/totp/enable", post(totp::totp_enable))
        .route("/me/totp/disable", post(totp::totp_disable))
        .route("/auth/totp/verify", post(totp::totp_verify))
        .route(
            "/admin/users/{pseudo}/reset-2fa",
            post(totp::admin_reset_totp),
        )
        .route("/analytics/track", post(handlers::track_event))
}
