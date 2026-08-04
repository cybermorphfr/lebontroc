//! Module troc : propositions « ça contre ça », soulte plafonnée, expiration.

pub mod dto;
pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/proposals", post(handlers::create_proposal))
        .route("/proposals/{id}", get(handlers::get_proposal))
        .route("/proposals/{id}/refuse", post(handlers::refuse_proposal))
        .route("/me/proposals", get(handlers::my_proposals))
}
