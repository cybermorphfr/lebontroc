//! Centre de notifications et préférences e-mail (F5.3).

pub mod dto;
pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notifications", get(handlers::list_notifications))
        .route(
            "/notifications/unread-count",
            get(handlers::get_unread_count),
        )
        .route("/notifications/{id}/read", post(handlers::mark_read))
        .route("/notifications/read-all", post(handlers::mark_all_read))
        .route(
            "/me/preferences/notifications",
            get(handlers::get_email_prefs).put(handlers::put_email_prefs),
        )
}
