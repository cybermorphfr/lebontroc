//! Handlers des propositions de troc : composer, boîtes, vue, refus.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Duration;
use domain::trade as regles;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::telemetry;
use crate::trade::dto::{CreateProposalRequest, ProposalItemResponse, ProposalResponse};
use crate::AppState;

fn map_trade_error(error: regles::TradeError) -> ApiError {
    match error {
        regles::TradeError::ObjetsManquants => ApiError::bad_request(
            "objets_manquants",
            "Choisis au moins un objet de chaque côté.",
        ),
        regles::TradeError::TropDObjets => {
            ApiError::bad_request("trop_dobjets", "Dix objets par côté maximum.")
        }
        regles::TradeError::SoulteTropHaute(plafond) => ApiError::bad_request(
            "soulte_trop_haute",
            format!(
                "La soulte est plafonnée à {} € — 50 % de la valeur du meilleur objet.",
                plafond / 100
            ),
        ),
        regles::TradeError::SoulteIncoherente => ApiError::bad_request(
            "soulte_incoherente",
            "Indique un montant et qui l'ajoute — ou pas de soulte du tout.",
        ),
        regles::TradeError::TransitionInterdite => ApiError::bad_request(
            "transition_interdite",
            "Cette proposition n'est plus ouverte.",
        ),
    }
}

/// Assemble les réponses d'un lot de propositions (objets + photos).
pub(crate) async fn proposal_responses(
    state: &AppState,
    proposals: Vec<infra::trade_repo::Proposal>,
    viewer_id: Uuid,
) -> Result<Vec<ProposalResponse>, ApiError> {
    let ids: Vec<Uuid> = proposals.iter().map(|p| p.id).collect();
    let mut items_by_proposal: HashMap<Uuid, Vec<infra::trade_repo::ProposalItem>> = HashMap::new();
    for item in infra::trade_repo::proposal_items(&state.pool, &ids).await? {
        items_by_proposal
            .entry(item.proposal_id)
            .or_default()
            .push(item);
    }

    Ok(proposals
        .into_iter()
        .map(|proposal| {
            let items = items_by_proposal.remove(&proposal.id).unwrap_or_default();
            let (offered, requested): (Vec<_>, Vec<_>) =
                items.into_iter().partition(|i| i.side == "offert");
            let to_response = |items: Vec<infra::trade_repo::ProposalItem>| {
                items
                    .into_iter()
                    .map(|i| ProposalItemResponse {
                        item_id: i.item_id,
                        title: i.title,
                        value_cents: i.value_cents_snapshot,
                        photo_url: i.s3_key.as_deref().map(|k| state.photos.public_url(k)),
                    })
                    .collect()
            };
            ProposalResponse {
                id: proposal.id,
                status: proposal.status,
                is_proposer: proposal.proposer_id == viewer_id,
                proposer_pseudo: proposal.proposer_pseudo,
                recipient_pseudo: proposal.recipient_pseudo,
                cash_cents: proposal.cash_cents,
                cash_direction: proposal.cash_direction,
                message: proposal.message,
                created_at: proposal.created_at,
                expires_at: proposal.expires_at,
                offered: to_response(offered),
                requested: to_response(requested),
            }
        })
        .collect())
}

async fn proposal_response(
    state: &AppState,
    id: Uuid,
    viewer_id: Uuid,
) -> Result<ProposalResponse, ApiError> {
    let proposal = infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))?;
    let mut responses = proposal_responses(state, vec![proposal], viewer_id).await?;
    Ok(responses.remove(0))
}

/// Envoyer une proposition « ça contre ça ».
#[utoipa::path(
    post,
    path = "/proposals",
    tag = "trade",
    request_body = CreateProposalRequest,
    responses(
        (status = 201, description = "Proposition envoyée", body = ProposalResponse),
        (status = 400, description = "Composition invalide", body = crate::error::ErrorResponse),
        (status = 403, description = "E-mail non vérifié", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_proposal(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateProposalRequest>,
) -> Result<(StatusCode, Json<ProposalResponse>), ApiError> {
    crate::catalog::handlers::require_verified(&state, user).await?;

    let indisponible = || {
        ApiError::bad_request(
            "objet_indisponible",
            "Un des objets n'est plus disponible — recharge la page.",
        )
    };

    // Mes objets : à moi, disponibles.
    let offered = infra::catalog_repo::items_by_ids(&state.pool, &body.offered_item_ids).await?;
    if offered.len() != body.offered_item_ids.len()
        || offered
            .iter()
            .any(|i| i.owner_id != user.user_id || i.status != "disponible")
    {
        return Err(indisponible());
    }

    // Ses objets : tous au même troqueur, qui n'est pas moi, disponibles.
    let requested =
        infra::catalog_repo::items_by_ids(&state.pool, &body.requested_item_ids).await?;
    if requested.len() != body.requested_item_ids.len()
        || requested.iter().any(|i| i.status != "disponible")
    {
        return Err(indisponible());
    }
    let Some(recipient_id) = requested.first().map(|i| i.owner_id) else {
        return Err(map_trade_error(regles::TradeError::ObjetsManquants));
    };
    if requested.iter().any(|i| i.owner_id != recipient_id) {
        return Err(ApiError::bad_request(
            "destinataires_multiples",
            "Une proposition vise les objets d'un seul troqueur.",
        ));
    }
    if recipient_id == user.user_id {
        return Err(ApiError::bad_request(
            "troc_avec_soi",
            "Troquer avec soi-même, c'est ranger — choisis un autre troqueur.",
        ));
    }

    let cash_direction = body.cash_direction.as_deref().unwrap_or("aucune");
    let offered_values: Vec<i32> = offered.iter().map(|i| i.value_cents).collect();
    let requested_values: Vec<i32> = requested.iter().map(|i| i.value_cents).collect();
    regles::valider_proposition(
        &offered_values,
        &requested_values,
        body.cash_cents,
        cash_direction,
    )
    .map_err(map_trade_error)?;

    let message = body
        .message
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(|m| m.chars().take(500).collect::<String>());

    let mut items: Vec<(Uuid, &str, i32)> = Vec::new();
    for item in &offered {
        items.push((item.id, "offert", item.value_cents));
    }
    for item in &requested {
        items.push((item.id, "demande", item.value_cents));
    }
    let expires_at = chrono::Utc::now() + Duration::days(regles::EXPIRATION_JOURS);
    let id = infra::trade_repo::create_proposal(
        &state.pool,
        user.user_id,
        recipient_id,
        body.cash_cents,
        cash_direction,
        message.as_deref(),
        expires_at,
        &items,
    )
    .await?;

    telemetry::track(
        &state,
        "proposal_sent",
        Some(user.user_id),
        json!({
            "proposal_id": id,
            "items_offered_count": offered.len(),
            "items_requested_count": requested.len(),
            "cash_amount": body.cash_cents,
            "cash_direction": cash_direction,
            "has_message": message.is_some(),
        }),
    )
    .await;

    let response = proposal_response(&state, id, user.user_id).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

#[derive(Deserialize)]
pub struct InboxQuery {
    /// `recues` (défaut) ou `envoyees`.
    pub r#box: Option<String>,
}

/// Mes propositions, reçues ou envoyées.
#[utoipa::path(
    get,
    path = "/me/proposals",
    tag = "trade",
    params(("box" = Option<String>, Query, description = "recues (défaut) ou envoyees")),
    responses((status = 200, description = "Mes propositions", body = [ProposalResponse]))
)]
pub async fn my_proposals(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<ProposalResponse>>, ApiError> {
    let received = query.r#box.as_deref() != Some("envoyees");
    let proposals = infra::trade_repo::list_proposals(&state.pool, user.user_id, received).await?;
    Ok(Json(
        proposal_responses(&state, proposals, user.user_id).await?,
    ))
}

/// Détail d'une proposition. La première ouverture par le destinataire la
/// passe à `vue` (le proposant sait qu'elle a été regardée).
#[utoipa::path(
    get,
    path = "/proposals/{id}",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    responses(
        (status = 200, description = "Proposition", body = ProposalResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_proposal(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProposalResponse>, ApiError> {
    let proposal = infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .filter(|p| p.proposer_id == user.user_id || p.recipient_id == user.user_id)
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))?;

    if proposal.recipient_id == user.user_id && regles::peut_marquer_vue(&proposal.status) {
        infra::trade_repo::mark_viewed(&state.pool, id).await?;
        telemetry::track(
            &state,
            "proposal_viewed",
            Some(user.user_id),
            json!({"proposal_id": id}),
        )
        .await;
    }
    Ok(Json(proposal_response(&state, id, user.user_id).await?))
}

/// Refuser une proposition (destinataire uniquement, proposition ouverte).
#[utoipa::path(
    post,
    path = "/proposals/{id}/refuse",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    responses(
        (status = 200, description = "Proposition refusée", body = ProposalResponse),
        (status = 400, description = "Plus ouverte", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn refuse_proposal(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProposalResponse>, ApiError> {
    let proposal = infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .filter(|p| p.recipient_id == user.user_id)
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))?;
    regles::peut_refuser(&proposal.status).map_err(map_trade_error)?;
    infra::trade_repo::refuse_proposal(&state.pool, id, user.user_id).await?;
    Ok(Json(proposal_response(&state, id, user.user_id).await?))
}

/// Expire les propositions en retard et notifie chaque proposant (e-mail +
/// télémétrie). Utilisée par la tâche de fond et testable directement.
pub async fn expire_and_notify(state: &AppState) -> usize {
    let expired = match infra::trade_repo::expire_overdue(&state.pool).await {
        Ok(expired) => expired,
        Err(error) => {
            tracing::error!(%error, "expiration des propositions en échec");
            return 0;
        }
    };
    let count = expired.len();
    for proposal in expired {
        if let Err(error) = state
            .mailer
            .send_proposal_expired(
                &proposal.proposer_email,
                &proposal.proposer_pseudo,
                &proposal.recipient_pseudo,
            )
            .await
        {
            tracing::error!(%error, proposal_id = %proposal.id, "e-mail d'expiration en échec");
        }
        telemetry::track(
            state,
            "proposal_expired",
            None,
            json!({"proposal_id": proposal.id}),
        )
        .await;
    }
    count
}
