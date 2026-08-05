//! Handlers du back-office (F6.1/F6.2). Chaque action mutante est
//! journalisée dans `admin_audit` (immuable) + télémétrie `admin_action`.

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::AdminAuth;
use crate::telemetry;
use crate::AppState;

/// Journalise une action admin (audit immuable + télémétrie).
pub async fn record_admin_action(
    state: &AppState,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<&str>,
) {
    if let Err(error) =
        infra::admin_repo::record_audit(&state.pool, action, target_type, target_id, details).await
    {
        tracing::error!(%error, action, "journal d'audit en échec");
    }
    telemetry::track(
        state,
        "admin_action",
        None,
        json!({"action_type": action, "target_type": target_type}),
    )
    .await;
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminSearchResponse {
    pub users: Vec<AdminUserDto>,
    pub items: Vec<AdminItemDto>,
    pub trades: Vec<AdminTradeDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminUserDto {
    pub id: Uuid,
    pub pseudo: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub restricted_until: Option<DateTime<Utc>>,
    pub banned_at: Option<DateTime<Utc>>,
    /// Score de fiabilité interne (jamais public).
    pub score: i32,
}

#[derive(Serialize, ToSchema)]
pub struct AdminItemDto {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub owner_pseudo: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminTradeDto {
    pub id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    pub created_at: DateTime<Utc>,
}

/// Recherche transverse : utilisateurs, objets, trocs (pseudo, e-mail,
/// titre ou UUID de troc).
#[utoipa::path(
    get,
    path = "/admin/search",
    tag = "admin",
    params(("q" = String, Query, description = "Pseudo, e-mail, titre ou UUID")),
    responses((status = 200, description = "Résultats", body = AdminSearchResponse))
)]
pub async fn admin_search(
    State(state): State<AppState>,
    _admin: AdminAuth,
    Query(query): Query<SearchQuery>,
) -> Result<Json<AdminSearchResponse>, ApiError> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(AdminSearchResponse {
            users: Vec::new(),
            items: Vec::new(),
            trades: Vec::new(),
        }));
    }
    let mut users = Vec::new();
    for hit in infra::admin_repo::search_users(&state.pool, q).await? {
        users.push(AdminUserDto {
            score: infra::dispute_repo::reliability_score(&state.pool, hit.id).await?,
            id: hit.id,
            pseudo: hit.pseudo,
            email: hit.email,
            created_at: hit.created_at,
            restricted_until: hit.restricted_until,
            banned_at: hit.banned_at,
        });
    }
    let items = infra::admin_repo::search_items(&state.pool, q)
        .await?
        .into_iter()
        .map(|i| AdminItemDto {
            id: i.id,
            title: i.title,
            status: i.status,
            owner_pseudo: i.owner_pseudo,
        })
        .collect();
    let trades = infra::admin_repo::search_trades(&state.pool, q)
        .await?
        .into_iter()
        .map(|t| AdminTradeDto {
            id: t.id,
            status: t.status,
            delivery_mode: t.delivery_mode,
            proposer_pseudo: t.proposer_pseudo,
            recipient_pseudo: t.recipient_pseudo,
            created_at: t.created_at,
        })
        .collect();
    Ok(Json(AdminSearchResponse {
        users,
        items,
        trades,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct AdminReportDto {
    pub id: Uuid,
    pub reporter_pseudo: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub comment: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ReportsQuery {
    pub status: Option<String>,
}

/// File des signalements.
#[utoipa::path(
    get,
    path = "/admin/reports",
    tag = "admin",
    params(("status" = Option<String>, Query, description = "nouveau | traite")),
    responses((status = 200, description = "Signalements", body = [AdminReportDto]))
)]
pub async fn admin_list_reports(
    State(state): State<AppState>,
    _admin: AdminAuth,
    Query(query): Query<ReportsQuery>,
) -> Result<Json<Vec<AdminReportDto>>, ApiError> {
    let reports = infra::admin_repo::list_reports(&state.pool, query.status.as_deref()).await?;
    Ok(Json(
        reports
            .into_iter()
            .map(|r| AdminReportDto {
                id: r.id,
                reporter_pseudo: r.reporter_pseudo,
                target_type: r.target_type,
                target_id: r.target_id,
                reason: r.reason,
                comment: r.comment,
                status: r.status,
                outcome: r.outcome,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct CloseReportRequest {
    /// `fonde` (un signalement fondé sur un utilisateur pèse +2 au score)
    /// ou `rejete`.
    pub outcome: String,
}

/// Clore un signalement. `fonde` sur un utilisateur → score + sanctions.
#[utoipa::path(
    post,
    path = "/admin/reports/{id}/close",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Identifiant du signalement")),
    request_body = CloseReportRequest,
    responses(
        (status = 204, description = "Clos"),
        (status = 400, description = "Issue invalide ou déjà traité", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_close_report(
    State(state): State<AppState>,
    _admin: AdminAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<CloseReportRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !matches!(body.outcome.as_str(), "fonde" | "rejete") {
        return Err(ApiError::bad_request(
            "issue_invalide",
            "L'issue est fonde ou rejete.",
        ));
    }
    let Some(report) = infra::admin_repo::close_report(&state.pool, id, &body.outcome).await?
    else {
        return Err(ApiError::bad_request(
            "deja_traite",
            "Ce signalement n'existe pas ou est déjà traité.",
        ));
    };
    // Un signalement d'utilisateur avéré alimente le score de fiabilité.
    if body.outcome == "fonde" && report.target_type == "utilisateur" {
        if let Err(error) = infra::admin_repo::record_scoring_event(
            &state.pool,
            report.target_id,
            "signalement_fonde",
            &format!("signalement {} fondé", report.reason),
        )
        .await
        {
            tracing::error!(%error, "journal signalement fondé en échec");
        }
        crate::dispute::handlers::apply_score_sanctions(&state, report.target_id).await;
    }
    record_admin_action(
        &state,
        "report_closed",
        &report.target_type,
        &report.target_id.to_string(),
        Some(&format!("{} → {}", report.reason, body.outcome)),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct AuditDto {
    pub id: i64,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Journal d'audit (immuable, 200 dernières actions).
#[utoipa::path(
    get,
    path = "/admin/audit",
    tag = "admin",
    responses((status = 200, description = "Journal", body = [AuditDto]))
)]
pub async fn admin_list_audit(
    State(state): State<AppState>,
    _admin: AdminAuth,
) -> Result<Json<Vec<AuditDto>>, ApiError> {
    let entries = infra::admin_repo::list_audit(&state.pool).await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|e| AuditDto {
                id: e.id,
                action: e.action,
                target_type: e.target_type,
                target_id: e.target_id,
                details: e.details,
                created_at: e.created_at,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct KpisDto {
    pub signups: i64,
    pub items_published: i64,
    pub proposals_sent: i64,
    pub trades_created: i64,
    pub trades_finalized: i64,
    pub trades_with_cash: i64,
    pub disputes_opened: i64,
}

/// KPI des 7 derniers jours (F6.2) — la même source que l'e-mail hebdo.
#[utoipa::path(
    get,
    path = "/admin/kpis",
    tag = "admin",
    responses((status = 200, description = "KPI 7 jours", body = KpisDto))
)]
pub async fn admin_kpis(
    State(state): State<AppState>,
    _admin: AdminAuth,
) -> Result<Json<KpisDto>, ApiError> {
    let kpis = infra::analytics::weekly_kpis(&state.pool).await?;
    Ok(Json(KpisDto {
        signups: kpis.signups,
        items_published: kpis.items_published,
        proposals_sent: kpis.proposals_sent,
        trades_created: kpis.trades_created,
        trades_finalized: kpis.trades_finalized,
        trades_with_cash: kpis.trades_with_cash,
        disputes_opened: kpis.disputes_opened,
    }))
}
