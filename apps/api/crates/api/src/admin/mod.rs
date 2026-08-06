//! Back-office (F6.1) : recherche transverse, file des signalements,
//! journal d'audit — derrière AdminAuth (token + basic auth Traefik).

pub mod handlers;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/search", get(handlers::admin_search))
        .route("/admin/reports", get(handlers::admin_list_reports))
        .route(
            "/admin/reports/{id}/close",
            post(handlers::admin_close_report),
        )
        .route("/admin/audit", get(handlers::admin_list_audit))
        .route("/admin/kpis", get(handlers::admin_kpis))
        .route("/admin/staff", get(handlers::admin_list_staff))
        .route("/admin/users/{pseudo}/role", post(handlers::admin_set_role))
}
