//! Back-office (F6.1) : recherche transverse, file des signalements,
//! journal d'audit — derrière AdminActor (session + rôle, ou clé de
//! service) et un garde-fou de débit.

pub mod handlers;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;

use crate::error::ApiError;
use crate::AppState;

/// Fenêtre glissante en mémoire : 120 requêtes / 60 s par origine. À
/// l'échelle d'une équipe d'administration, dépasser ce débit est un
/// script fou ou une clé volée — pas un usage légitime.
const FENETRE: Duration = Duration::from_secs(60);
const MAX_REQUETES: usize = 120;

#[derive(Clone, Default)]
struct Debit(Arc<Mutex<HashMap<String, Vec<Instant>>>>);

async fn limiter(
    axum::extract::State(debit): axum::extract::State<Debit>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // L'origine : le cookie de session s'il existe, sinon l'adresse.
    let origine = request
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|c| c.chars().take(64).collect::<String>())
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<std::net::SocketAddr>>()
                .map(|c| c.0.ip().to_string())
        })
        .unwrap_or_else(|| "inconnu".to_string());
    {
        let mut table = debit.0.lock().expect("verrou débit admin");
        let maintenant = Instant::now();
        let historique = table.entry(origine).or_default();
        historique.retain(|t| maintenant.duration_since(*t) < FENETRE);
        if historique.len() >= MAX_REQUETES {
            return Err(ApiError::too_many(
                "trop_de_requetes",
                "Trop de requêtes d'administration — réessaie dans une minute.",
            ));
        }
        historique.push(maintenant);
        // La table ne garde que les origines actives.
        table.retain(|_, v| !v.is_empty());
    }
    Ok(next.run(request).await)
}

pub fn router() -> Router<AppState> {
    let debit = Debit::default();
    Router::new()
        .route("/admin/search", get(handlers::admin_search))
        .route("/admin/reports", get(handlers::admin_list_reports))
        .route(
            "/admin/reports/{id}/close",
            post(handlers::admin_close_report),
        )
        .route("/admin/audit", get(handlers::admin_list_audit))
        .route("/admin/kpis", get(handlers::admin_kpis))
        .route("/admin/dashboard", get(handlers::admin_dashboard))
        .route("/admin/staff", get(handlers::admin_list_staff))
        .route("/admin/users/{pseudo}/role", post(handlers::admin_set_role))
        .layer(middleware::from_fn_with_state(debit, limiter))
}
