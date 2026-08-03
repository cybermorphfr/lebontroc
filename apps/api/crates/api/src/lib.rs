//! Couche HTTP de l'API Lebontroc : routes Axum minces, contrat OpenAPI
//! généré par utoipa. La logique métier vit dans `domain`, l'IO dans `infra`.

pub mod health;
pub mod openapi;

use axum::http::Request;
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

/// État partagé des handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub version: String,
}

/// Construit le routeur complet de l'API.
///
/// Traefik retire le préfixe `/api` en amont : les routes vivent à la racine.
pub fn router(state: AppState) -> Router {
    let trace_layer = TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
        let request_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "http_request",
            method = %request.method(),
            uri = %request.uri(),
            request_id = %request_id,
        )
    });

    Router::new()
        .route("/health", get(health::health))
        .route(
            "/openapi.json",
            get(|| async { Json(openapi::ApiDoc::openapi()) }),
        )
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(trace_layer)
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
}
