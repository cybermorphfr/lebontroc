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
        .route("/proposals/{id}/accept", post(handlers::accept_proposal))
        .route("/proposals/{id}/counter", post(handlers::counter_proposal))
        .route("/me/proposals", get(handlers::my_proposals))
        .route("/trades/{id}", get(handlers::get_trade))
        .route("/trades/{id}/pay", post(handlers::pay_trade))
        .route("/trades/{id}/confirm", post(handlers::confirm_trade))
        .route("/trades/{id}/cancel", post(handlers::cancel_trade))
        .route("/trades/{id}/relays", get(handlers::trade_relays))
        .route("/trades/{id}/shipping", post(handlers::configure_shipping))
        .route("/shipments/{id}/drop", post(handlers::drop_parcel))
        .route("/shipments/{id}/pickup", post(handlers::pickup_parcel))
        .route("/shipments/{id}/confirm", post(handlers::confirm_parcel))
        .route("/shipments/{id}/report", post(handlers::report_parcel))
}
