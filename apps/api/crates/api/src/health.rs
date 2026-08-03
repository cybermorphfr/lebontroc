//! Endpoint `GET /health` — statut de l'API, version du build, état de la base.

use axum::extract::State;
use axum::Json;
use domain::health::{overall_status, DependencyStatus, HealthStatus};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

/// Réponse du endpoint de santé.
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Statut global de l'API.
    #[schema(example = "ok")]
    pub status: ApiStatus,
    /// Version du build (crate + SHA court du commit).
    #[schema(example = "0.1.0+abc1234")]
    pub version: String,
    /// État de la connexion PostgreSQL.
    #[schema(example = "ok")]
    pub db: DbStatus,
}

#[derive(Serialize, ToSchema, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ApiStatus {
    Ok,
    Degraded,
}

#[derive(Serialize, ToSchema, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DbStatus {
    Ok,
    Unreachable,
}

impl From<HealthStatus> for ApiStatus {
    fn from(value: HealthStatus) -> Self {
        match value {
            HealthStatus::Ok => ApiStatus::Ok,
            HealthStatus::Degraded => ApiStatus::Degraded,
        }
    }
}

impl From<DependencyStatus> for DbStatus {
    fn from(value: DependencyStatus) -> Self {
        match value {
            DependencyStatus::Ok => DbStatus::Ok,
            DependencyStatus::Unreachable => DbStatus::Unreachable,
        }
    }
}

/// Statut de l'API et de ses dépendances.
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "Statut de l'API", body = HealthResponse)
    )
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let db = if infra::db::ping(&state.pool).await {
        DependencyStatus::Ok
    } else {
        DependencyStatus::Unreachable
    };

    Json(HealthResponse {
        status: overall_status(&[db]).into(),
        version: state.version.clone(),
        db: db.into(),
    })
}
