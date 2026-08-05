//! Signalements, blocages, litiges et sanctions (F5.2).

pub mod dto;
pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/trades/{id}/dispute", post(handlers::open_dispute))
        .route(
            "/trades/{id}/dispute/presign",
            post(handlers::presign_dispute_photos),
        )
        .route("/disputes/{id}/respond", post(handlers::respond_dispute))
        .route("/reports", post(handlers::create_report))
        .route(
            "/users/{pseudo}/block",
            post(handlers::block_user).delete(handlers::unblock_user),
        )
        .route("/me/blocks", get(handlers::my_blocks))
        .route("/admin/disputes", get(handlers::admin_list_disputes))
        .route("/admin/disputes/{id}", get(handlers::admin_get_dispute))
        .route(
            "/admin/disputes/{id}/resolve",
            post(handlers::admin_resolve_dispute),
        )
        .route(
            "/admin/users/{pseudo}/lift-sanctions",
            post(handlers::admin_lift_sanctions),
        )
}
