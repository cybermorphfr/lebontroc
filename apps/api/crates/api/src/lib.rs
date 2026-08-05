//! Couche HTTP de l'API Lebontroc : routes Axum minces, contrat OpenAPI
//! généré par utoipa. La logique métier vit dans `domain`, l'IO dans `infra`.

pub mod auth;
pub mod catalog;
pub mod config;
pub mod error;
pub mod extract;
pub mod health;
pub mod messaging;
pub mod openapi;
pub mod telemetry;
pub mod trade;

use std::sync::Arc;

use axum::http::Request;
use axum::routing::get;
use axum::{Json, Router};
use infra::email::EmailSender;
use infra::payment::{FakePaymentProvider, PaymentProvider};
use infra::s3::PhotoStore;
use infra::search::{PgSearchRepository, SearchRepository};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::config::AppConfig;

/// État partagé des handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub version: String,
    pub config: Arc<AppConfig>,
    pub mailer: EmailSender,
    pub photos: PhotoStore,
    /// Recherche derrière un trait : Postgres au MVP, Meilisearch en V2.
    pub search: Arc<dyn SearchRepository>,
    /// Diffusion temps réel (WebSocket) — broker en mémoire, mono-processus.
    pub events: broadcast::Sender<messaging::ws::WsEvent>,
    /// PSP derrière un trait : simulateur en bêta fermée, Mangopay sandbox
    /// dès que les clés existent — sans toucher aux handlers.
    pub payments: Arc<dyn PaymentProvider>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        version: String,
        config: AppConfig,
        mailer: EmailSender,
        photos: PhotoStore,
    ) -> Self {
        let search = Arc::new(PgSearchRepository::new(pool.clone()));
        let (events, _) = broadcast::channel(256);
        Self {
            pool,
            version,
            config: Arc::new(config),
            mailer,
            photos,
            search,
            events,
            payments: Arc::new(FakePaymentProvider::new()),
        }
    }

    /// État prêt à l'emploi pour les tests d'intégration : cookies non Secure,
    /// e-mails capturés en mémoire.
    pub fn for_tests(
        pool: PgPool,
    ) -> (
        Self,
        std::sync::Arc<std::sync::Mutex<Vec<infra::email::CapturedEmail>>>,
    ) {
        let (mailer, emails) = EmailSender::capture();
        let config = AppConfig::new(
            "secret-de-test-secret-de-test-secret",
            false,
            "http://localhost:3000".to_string(),
            "sel-de-test".to_string(),
        );
        (
            Self::new(
                pool,
                "0.1.0+test".to_string(),
                config,
                mailer,
                PhotoStore::mock(),
            ),
            emails,
        )
    }
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
        .merge(auth::router())
        .merge(catalog::router())
        .merge(trade::router())
        .merge(messaging::router())
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(trace_layer)
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
}
