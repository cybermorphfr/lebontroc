//! Module messagerie : WebSocket temps réel, messages par proposition,
//! modération anti-contournement, relance des non-lus.

pub mod dto;
pub mod handlers;
pub mod ws;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ws", get(ws::ws_upgrade))
        .route("/me/conversations", get(handlers::my_conversations))
        .route(
            "/proposals/{id}/messages",
            get(handlers::list_messages).post(handlers::send_message),
        )
        .route("/proposals/{id}/read", post(handlers::mark_read))
}
