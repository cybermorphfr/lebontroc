//! Handlers des propositions de troc : composer, boîtes, vue, refus.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Duration;
use domain::payment as paiement;
use domain::trade as regles;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::messaging::ws::broadcast_event;
use crate::telemetry;
use crate::trade::dto::{
    AcceptProposalRequest, ConfirmTradeRequest, CreateProposalRequest, PayTradeRequest,
    PaymentInfo, ProposalItemResponse, ProposalResponse, TradeDetailResponse, TradeResponse,
};
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
    let mut trades_by_proposal: HashMap<Uuid, infra::trade_repo::Trade> =
        infra::trade_repo::trades_for_proposals(&state.pool, &ids)
            .await?
            .into_iter()
            .map(|t| (t.proposal_id, t))
            .collect();
    let superseded: HashMap<Uuid, Uuid> = infra::trade_repo::superseded_by(&state.pool, &ids)
        .await?
        .into_iter()
        .collect();

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
                counter_of: proposal.counter_of,
                superseded_by: superseded.get(&proposal.id).copied(),
                trade: trades_by_proposal
                    .remove(&proposal.id)
                    .map(|t| TradeResponse {
                        id: t.id,
                        status: t.status,
                        delivery_mode: t.delivery_mode,
                    }),
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
    telemetry::track(
        &state,
        "proposal_declined",
        Some(user.user_id),
        json!({"proposal_id": id}),
    )
    .await;
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

/// Accepter une proposition — LA transaction critique : objets réservés,
/// concurrentes caduques, proposants évincés notifiés. Idempotente (une
/// proposition ne crée qu'un troc) : le double clic renvoie le même troc.
#[utoipa::path(
    post,
    path = "/proposals/{id}/accept",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    request_body = AcceptProposalRequest,
    responses(
        (status = 200, description = "Troc créé (ou déjà créé)", body = ProposalResponse),
        (status = 400, description = "Plus ouverte ou mode invalide", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse),
        (status = 409, description = "Un objet vient d'être réservé ailleurs", body = crate::error::ErrorResponse)
    )
)]
pub async fn accept_proposal(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AcceptProposalRequest>,
) -> Result<Json<ProposalResponse>, ApiError> {
    regles::valider_mode_remise(&body.delivery_mode)
        .map_err(|_| ApiError::bad_request("remise_inconnue", "Choisis main propre ou envoi."))?;

    let apercu = infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .filter(|p| p.recipient_id == user.user_id)
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))?;
    // Garde de la bêta fermée : l'envoi croisé arrive en F4.3.
    if body.delivery_mode == "envoi" {
        return Err(ApiError::bad_request(
            "envoi_indisponible",
            "L'envoi arrive bientôt — pendant la bêta, choisis la remise en main propre.",
        ));
    }

    // Avec soulte (F4.2) : le troc naît en attente de paiement — le payeur
    // préautorise la soulte, séquestrée jusqu'à la remise.
    let payment = if apercu.cash_cents > 0 {
        let payer_id = match paiement::payeur(&apercu.cash_direction) {
            Some(paiement::Payeur::Proposant) => apercu.proposer_id,
            Some(paiement::Payeur::Destinataire) => apercu.recipient_id,
            None => {
                return Err(ApiError::internal(
                    "soulte sans direction — donnée corrompue",
                ));
            }
        };
        let beneficiary_id = if payer_id == apercu.proposer_id {
            apercu.recipient_id
        } else {
            apercu.proposer_id
        };
        let delai = paiement::delai_paiement_minutes(payer_id == user.user_id);
        Some(infra::payment_repo::NewPayment {
            payer_id,
            beneficiary_id,
            amount_cents: apercu.cash_cents,
            fees_cents: paiement::commission_cents(
                apercu.cash_cents,
                state.config.payment_fees_bps,
            ),
            provider: state.payments.name().to_string(),
            deadline: chrono::Utc::now() + Duration::minutes(delai),
        })
    } else {
        None
    };

    // Codes de confirmation du rendez-vous, générés à l'acceptation.
    let code = || {
        format!(
            "{:06}",
            rand::Rng::gen_range(&mut rand::thread_rng(), 0..1_000_000)
        )
    };
    let codes = (code(), code());
    let outcome = infra::trade_repo::accept_proposal(
        &state.pool,
        id,
        user.user_id,
        &body.delivery_mode,
        (&codes.0, &codes.1),
        payment.as_ref(),
    )
    .await?;
    let trade = match outcome {
        infra::trade_repo::AcceptOutcome::NotFound => {
            return Err(ApiError::not_found("Cette proposition n'existe pas."))
        }
        // Selon le timing de la course, la perdante voit soit l'objet déjà
        // réservé, soit sa proposition déjà caduque : même réponse claire.
        infra::trade_repo::AcceptOutcome::NotOpen(status) if status == "caduque" => {
            return Err(ApiError::conflict(
                "objet_deja_reserve",
                "Trop tard : un des objets vient d'être réservé dans un autre troc. \
                 La proposition n'est plus valable.",
            ))
        }
        infra::trade_repo::AcceptOutcome::NotOpen(_) => {
            return Err(map_trade_error(regles::TradeError::TransitionInterdite))
        }
        infra::trade_repo::AcceptOutcome::ItemsUnavailable => {
            // Peut être temporaire : un troc en attente de paiement libère
            // ses objets si la soulte n'est jamais réglée.
            return Err(ApiError::conflict(
                "objet_deja_reserve",
                "Trop tard : un des objets vient d'être réservé dans un autre troc.",
            ));
        }
        infra::trade_repo::AcceptOutcome::AlreadyAccepted(trade) => trade,
        infra::trade_repo::AcceptOutcome::Accepted(trade, evictions) => {
            telemetry::track(
                &state,
                "proposal_accepted",
                Some(user.user_id),
                json!({"proposal_id": id}),
            )
            .await;
            telemetry::track(
                &state,
                "trade_created",
                Some(user.user_id),
                json!({
                    "trade_id": trade.id,
                    "delivery_mode": trade.delivery_mode,
                    "has_cash": trade.cash_cents > 0,
                }),
            )
            .await;
            telemetry::track(
                &state,
                "trade_meetup_code_generated",
                Some(user.user_id),
                json!({"trade_id": trade.id}),
            )
            .await;
            notify_evictions(&state, evictions).await;

            // Avec soulte : prévenir le payeur s'il n'est pas devant l'écran.
            if let Some(payment) = &payment {
                if payment.payer_id != user.user_id {
                    match infra::auth_repo::find_user_by_id(&state.pool, payment.payer_id).await {
                        Ok(Some(payer)) => {
                            let other = if payment.payer_id == apercu.proposer_id {
                                &apercu.recipient_pseudo
                            } else {
                                &apercu.proposer_pseudo
                            };
                            if let Err(error) = state
                                .mailer
                                .send_payment_due(
                                    &payer.email,
                                    &payer.pseudo,
                                    other,
                                    payment.amount_cents,
                                    paiement::DELAI_PAIEMENT_AUTRE_MINUTES / 60,
                                )
                                .await
                            {
                                tracing::error!(%error, trade_id = %trade.id,
                                    "e-mail de soulte à régler en échec");
                            }
                        }
                        Ok(None) => {}
                        Err(error) => tracing::error!(%error, "payeur introuvable"),
                    }
                }
                broadcast_event(
                    &state,
                    [apercu.proposer_id, apercu.recipient_id],
                    json!({"type": "trade_updated", "proposal_id": id, "trade_id": trade.id}),
                );
            }
            trade
        }
    };
    tracing::info!(trade_id = %trade.id, proposal_id = %id, "troc accepté");
    Ok(Json(proposal_response(&state, id, user.user_id).await?))
}

/// Notifie les proposants dont la proposition vient d'être rendue caduque.
async fn notify_evictions(state: &AppState, evictions: Vec<infra::trade_repo::Eviction>) {
    for eviction in evictions {
        if let Err(error) = state
            .mailer
            .send_proposal_invalidated(&eviction.proposer_email, &eviction.proposer_pseudo)
            .await
        {
            tracing::error!(%error, proposal_id = %eviction.proposal_id,
                "e-mail d'éviction en échec");
        }
        telemetry::track(
            state,
            "proposal_invalidated",
            None,
            json!({"proposal_id": eviction.proposal_id}),
        )
        .await;
    }
}

/// Contre-proposer : la proposition ouverte est remplacée par une nouvelle
/// aux rôles inversés, chaînée par `counter_of` ; la conversation suit.
#[utoipa::path(
    post,
    path = "/proposals/{id}/counter",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Proposition à remplacer")),
    request_body = CreateProposalRequest,
    responses(
        (status = 201, description = "Contre-proposition envoyée", body = ProposalResponse),
        (status = 400, description = "Composition invalide ou plus ouverte", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn counter_proposal(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateProposalRequest>,
) -> Result<(StatusCode, Json<ProposalResponse>), ApiError> {
    let old = infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .filter(|p| p.recipient_id == user.user_id)
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))?;
    regles::peut_contre_proposer(&old.status).map_err(map_trade_error)?;

    let indisponible = || {
        ApiError::bad_request(
            "objet_indisponible",
            "Un des objets n'est plus disponible — recharge la page.",
        )
    };
    // Je contre-propose : mes objets contre ceux du proposant initial.
    let offered = infra::catalog_repo::items_by_ids(&state.pool, &body.offered_item_ids).await?;
    if offered.len() != body.offered_item_ids.len()
        || offered
            .iter()
            .any(|i| i.owner_id != user.user_id || i.status != "disponible")
    {
        return Err(indisponible());
    }
    let requested =
        infra::catalog_repo::items_by_ids(&state.pool, &body.requested_item_ids).await?;
    if requested.len() != body.requested_item_ids.len()
        || requested
            .iter()
            .any(|i| i.owner_id != old.proposer_id || i.status != "disponible")
    {
        return Err(indisponible());
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
    let Some(new_id) = infra::trade_repo::counter_proposal(
        &state.pool,
        id,
        user.user_id,
        old.proposer_id,
        body.cash_cents,
        cash_direction,
        message.as_deref(),
        expires_at,
        &items,
    )
    .await?
    else {
        return Err(map_trade_error(regles::TradeError::TransitionInterdite));
    };

    telemetry::track(
        &state,
        "proposal_countered",
        Some(user.user_id),
        json!({
            "old_proposal_id": id,
            "proposal_id": new_id,
            "items_offered_count": offered.len(),
            "items_requested_count": requested.len(),
            "cash_amount": body.cash_cents,
        }),
    )
    .await;

    let response = proposal_response(&state, new_id, user.user_id).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

// ————— Remise en main propre (F4.1) —————

fn trade_detail_response(
    trade: infra::trade_repo::TradeDetail,
    payment: Option<&infra::payment_repo::Payment>,
    user_id: Uuid,
) -> TradeDetailResponse {
    let is_proposer = trade.proposer_id == user_id;
    let payment_info = payment.map(|p| PaymentInfo {
        status: p.status.clone(),
        amount_cents: p.amount_cents,
        fees_cents: p.fees_cents,
        net_cents: paiement::net_beneficiaire_cents(p.amount_cents, p.fees_cents),
        i_am_payer: p.payer_id == user_id,
        deadline: p.deadline,
        failure_reason: p.failure_reason.clone(),
        secure_mode_url: None,
    });
    // Le code de remise n'a pas cours tant que la soulte n'est pas séquestrée.
    let code_active = trade.status != "attente_paiement";
    TradeDetailResponse {
        id: trade.id,
        proposal_id: trade.proposal_id,
        status: trade.status,
        delivery_mode: trade.delivery_mode,
        my_code: if !code_active {
            None
        } else if is_proposer {
            trade.proposer_code
        } else {
            trade.recipient_code
        },
        i_confirmed: if is_proposer {
            trade.proposer_confirmed_at.is_some()
        } else {
            trade.recipient_confirmed_at.is_some()
        },
        other_confirmed: if is_proposer {
            trade.recipient_confirmed_at.is_some()
        } else {
            trade.proposer_confirmed_at.is_some()
        },
        finalized_at: trade.finalized_at,
        cancelled_at: trade.cancelled_at,
        cancel_requested_by_me: trade.cancel_requested_by == Some(user_id),
        cancel_requested_by_other: trade
            .cancel_requested_by
            .map(|by| by != user_id)
            .unwrap_or(false),
        accepted_at: trade.created_at,
        payment: payment_info,
    }
}

/// Vue complète du troc, paiement compris.
async fn full_trade_response(
    state: &AppState,
    trade: infra::trade_repo::TradeDetail,
    user_id: Uuid,
) -> Result<TradeDetailResponse, ApiError> {
    let payment = infra::payment_repo::payment_for_trade(&state.pool, trade.id).await?;
    Ok(trade_detail_response(trade, payment.as_ref(), user_id))
}

async fn participant_trade(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
) -> Result<infra::trade_repo::TradeDetail, ApiError> {
    infra::trade_repo::get_trade(&state.pool, id)
        .await?
        .filter(|t| t.proposer_id == user_id || t.recipient_id == user_id)
        .ok_or_else(|| ApiError::not_found("Ce troc n'existe pas."))
}

/// L'écran de rendez-vous d'un troc (participants uniquement).
#[utoipa::path(
    get,
    path = "/trades/{id}",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    responses(
        (status = 200, description = "Détail du troc", body = TradeDetailResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_trade(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let mut trade = participant_trade(&state, id, user.user_id).await?;
    // Expiration paresseuse : consulter un troc au paiement en retard
    // l'annule tout de suite — sans attendre la maintenance horaire.
    if trade.status == "attente_paiement" && expire_unpaid_if_overdue(&state, trade.id).await? {
        trade = participant_trade(&state, id, user.user_id).await?;
    }
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Annule le troc si son paiement a dépassé la date limite : paiement
/// `expire`, troc `annule`, objets libérés, parties notifiées. Retourne
/// `true` si l'annulation vient d'avoir lieu.
async fn expire_unpaid_if_overdue(state: &AppState, trade_id: Uuid) -> sqlx::Result<bool> {
    let Some(payment) = infra::payment_repo::payment_for_trade(&state.pool, trade_id).await? else {
        return Ok(false);
    };
    if !paiement::peut_expirer(&payment.status) || payment.deadline > chrono::Utc::now() {
        return Ok(false);
    }
    let parties = infra::payment_repo::expire_unpaid_trade(&state.pool, trade_id).await?;
    if parties.is_empty() {
        return Ok(false);
    }
    for party in &parties {
        if let Err(error) = state
            .mailer
            .send_trade_payment_expired(&party.email, &party.pseudo, &party.other_pseudo)
            .await
        {
            tracing::error!(%error, %trade_id, "e-mail d'expiration de paiement en échec");
        }
    }
    telemetry::track(
        state,
        "payment_failed",
        None,
        json!({"trade_id": trade_id, "failure_reason": "delai_depasse"}),
    )
    .await;
    tracing::info!(%trade_id, "troc annulé : soulte jamais réglée");
    Ok(true)
}

/// Préautoriser la soulte (F4.2) — réservé au payeur, tant que la date
/// limite n'est pas dépassée. Bêta fermée : PSP simulé, aucune carte réelle.
#[utoipa::path(
    post,
    path = "/trades/{id}/pay",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = PayTradeRequest,
    responses(
        (status = 200, description = "Soulte séquestrée (ou déjà séquestrée)", body = TradeDetailResponse),
        (status = 400, description = "Refus, délai dépassé ou pas de soulte", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn pay_trade(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PayTradeRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let trade = participant_trade(&state, id, user.user_id).await?;
    let Some(payment) = infra::payment_repo::payment_for_trade(&state.pool, id).await? else {
        return Err(ApiError::bad_request(
            "aucun_paiement",
            "Ce troc ne comporte pas de soulte.",
        ));
    };
    if payment.payer_id != user.user_id {
        return Err(ApiError::bad_request(
            "pas_le_payeur",
            "C'est à l'autre partie de régler la soulte.",
        ));
    }
    // Idempotence : un double clic renvoie simplement l'état séquestré.
    if matches!(payment.status.as_str(), "sequestre" | "capture") {
        return Ok(Json(
            full_trade_response(&state, trade, user.user_id).await?,
        ));
    }
    if trade.status != "attente_paiement" || !paiement::peut_tenter_preautorisation(&payment.status)
    {
        return Err(ApiError::bad_request(
            "troc_clos",
            "Ce troc n'attend plus de paiement.",
        ));
    }
    if payment.deadline < chrono::Utc::now() {
        expire_unpaid_if_overdue(&state, id).await?;
        return Err(ApiError::bad_request(
            "paiement_expire",
            "Le délai de paiement est dépassé : le troc a été annulé et les objets libérés.",
        ));
    }

    let digits: String = body
        .card_number
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    if !paiement::carte_plausible(&digits) {
        return Err(ApiError::bad_request(
            "carte_invalide",
            "Ce numéro de carte ne ressemble pas à un numéro valide.",
        ));
    }

    infra::payment_repo::record_attempt(&state.pool, payment.id).await?;
    telemetry::track(
        &state,
        "payment_initiated",
        Some(user.user_id),
        json!({"trade_id": id, "amount_cents": payment.amount_cents}),
    )
    .await;

    let reference = format!("trade-{id}");
    let outcome = state
        .payments
        .preauthorize(infra::payment::PreauthRequest {
            reference: &reference,
            amount_cents: payment.amount_cents,
            card_number: &digits,
        })
        .await
        .map_err(ApiError::internal)?;

    match outcome {
        infra::payment::PreauthOutcome::Failed { reason } => {
            infra::payment_repo::mark_failed(&state.pool, payment.id, &reason).await?;
            telemetry::track(
                &state,
                "payment_failed",
                Some(user.user_id),
                json!({"trade_id": id, "failure_reason": reason}),
            )
            .await;
            let message = match reason.as_str() {
                "provision_insuffisante" => {
                    "Provision insuffisante : la banque n'a pas pu bloquer le montant. \
                     Réessaie avec une autre carte."
                }
                _ => "Ta banque a refusé la préautorisation. Vérifie ta carte et réessaie.",
            };
            Err(ApiError::bad_request("paiement_refuse", message))
        }
        infra::payment::PreauthOutcome::Pending {
            provider_ref: _,
            secure_mode_url,
        } => {
            // Flux 3DS d'un PSP réel : le front ouvre l'URL, le webhook ou le
            // polling terminera la transition. Jamais émis par le simulateur.
            let trade = participant_trade(&state, id, user.user_id).await?;
            let mut response = full_trade_response(&state, trade, user.user_id).await?;
            if let Some(info) = response.payment.as_mut() {
                info.secure_mode_url = Some(secure_mode_url);
            }
            Ok(Json(response))
        }
        infra::payment::PreauthOutcome::Escrowed { provider_ref } => {
            match infra::payment_repo::escrow_payment(&state.pool, id, Some(&provider_ref)).await? {
                infra::payment_repo::EscrowOutcome::NotPending => {
                    return Err(ApiError::bad_request(
                        "troc_clos",
                        "Ce troc n'attend plus de paiement.",
                    ))
                }
                infra::payment_repo::EscrowOutcome::AlreadyEscrowed => {}
                infra::payment_repo::EscrowOutcome::Escrowed { evictions } => {
                    telemetry::track(
                        &state,
                        "payment_escrowed",
                        Some(user.user_id),
                        json!({"trade_id": id, "amount_cents": payment.amount_cents}),
                    )
                    .await;
                    notify_evictions(&state, evictions).await;
                    match infra::auth_repo::find_user_by_id(&state.pool, payment.beneficiary_id)
                        .await
                    {
                        Ok(Some(beneficiary)) => {
                            let payer_pseudo =
                                infra::auth_repo::find_user_by_id(&state.pool, payment.payer_id)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|u| u.pseudo)
                                    .unwrap_or_default();
                            if let Err(error) = state
                                .mailer
                                .send_payment_escrowed(
                                    &beneficiary.email,
                                    &beneficiary.pseudo,
                                    &payer_pseudo,
                                    payment.amount_cents,
                                )
                                .await
                            {
                                tracing::error!(%error, trade_id = %id,
                                    "e-mail de séquestre en échec");
                            }
                        }
                        Ok(None) => {}
                        Err(error) => tracing::error!(%error, "bénéficiaire introuvable"),
                    }
                    let trade = participant_trade(&state, id, user.user_id).await?;
                    broadcast_event(
                        &state,
                        [trade.proposer_id, trade.recipient_id],
                        json!({"type": "trade_updated", "proposal_id": trade.proposal_id,
                               "trade_id": id}),
                    );
                    tracing::info!(trade_id = %id, "soulte séquestrée");
                    return Ok(Json(
                        full_trade_response(&state, trade, user.user_id).await?,
                    ));
                }
            }
            let trade = participant_trade(&state, id, user.user_id).await?;
            Ok(Json(
                full_trade_response(&state, trade, user.user_id).await?,
            ))
        }
    }
}

/// Au rendez-vous : je saisis (ou scanne) le code de l'autre. Deux
/// confirmations → troc finalisé, objets troqués (Gherkin F4.1).
#[utoipa::path(
    post,
    path = "/trades/{id}/confirm",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = ConfirmTradeRequest,
    responses(
        (status = 200, description = "Confirmation enregistrée", body = TradeDetailResponse),
        (status = 400, description = "Code invalide ou troc clos", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn confirm_trade(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ConfirmTradeRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let code: String = body.code.chars().filter(|c| c.is_ascii_digit()).collect();
    let outcome = infra::trade_repo::confirm_trade(&state.pool, id, user.user_id, &code).await?;
    match outcome {
        infra::trade_repo::ConfirmOutcome::NotFound => {
            Err(ApiError::not_found("Ce troc n'existe pas."))
        }
        infra::trade_repo::ConfirmOutcome::NotActive(_) => Err(ApiError::bad_request(
            "troc_clos",
            "Ce troc n'est plus en attente de confirmation.",
        )),
        infra::trade_repo::ConfirmOutcome::WrongCode => Err(ApiError::bad_request(
            "code_invalide",
            "Ce n'est pas le bon code — vérifie avec l'autre partie (6 chiffres).",
        )),
        infra::trade_repo::ConfirmOutcome::Confirmed { finalized } => {
            if finalized {
                let trade = participant_trade(&state, id, user.user_id).await?;
                let days = (chrono::Utc::now() - trade.created_at).num_days();
                telemetry::track(
                    &state,
                    "trade_finalized",
                    Some(user.user_id),
                    json!({"trade_id": id, "days_since_accept": days}),
                )
                .await;
                // Remise confirmée par les deux codes : capturer la soulte.
                if let Some(payment) =
                    infra::payment_repo::payment_for_trade(&state.pool, id).await?
                {
                    if paiement::peut_capturer(&payment.status) {
                        capture_payment(&state, &payment).await;
                    }
                }
                return Ok(Json(
                    full_trade_response(&state, trade, user.user_id).await?,
                ));
            }
            let trade = participant_trade(&state, id, user.user_id).await?;
            Ok(Json(
                full_trade_response(&state, trade, user.user_id).await?,
            ))
        }
    }
}

/// Capture une soulte séquestrée (remise confirmée) : PSP, transition SQL,
/// e-mails, télémétrie. En cas d'échec PSP le paiement reste `sequestre` —
/// la maintenance horaire retentera.
async fn capture_payment(state: &AppState, payment: &infra::payment_repo::Payment) {
    if let Err(error) = state
        .payments
        .capture(
            payment.provider_ref.as_deref().unwrap_or(""),
            payment.amount_cents,
            payment.fees_cents,
        )
        .await
    {
        tracing::error!(%error, trade_id = %payment.trade_id,
            "capture de la soulte en échec — la maintenance retentera");
        return;
    }
    match infra::payment_repo::mark_captured(&state.pool, payment.id).await {
        Ok(true) => {}
        Ok(false) => return, // déjà capturé par un autre chemin
        Err(error) => {
            tracing::error!(%error, trade_id = %payment.trade_id, "transition capture en échec");
            return;
        }
    }
    telemetry::track(
        state,
        "payment_released",
        None,
        json!({"trade_id": payment.trade_id, "amount_cents": payment.amount_cents,
               "fees_cents": payment.fees_cents}),
    )
    .await;
    let net = paiement::net_beneficiaire_cents(payment.amount_cents, payment.fees_cents);
    let payer = infra::auth_repo::find_user_by_id(&state.pool, payment.payer_id)
        .await
        .ok()
        .flatten();
    let beneficiary = infra::auth_repo::find_user_by_id(&state.pool, payment.beneficiary_id)
        .await
        .ok()
        .flatten();
    if let (Some(payer), Some(beneficiary)) = (payer, beneficiary) {
        if let Err(error) = state
            .mailer
            .send_payment_released_beneficiary(
                &beneficiary.email,
                &beneficiary.pseudo,
                &payer.pseudo,
                net,
            )
            .await
        {
            tracing::error!(%error, "e-mail de transfert (bénéficiaire) en échec");
        }
        if let Err(error) = state
            .mailer
            .send_payment_released_payer(
                &payer.email,
                &payer.pseudo,
                &beneficiary.pseudo,
                payment.amount_cents,
            )
            .await
        {
            tracing::error!(%error, "e-mail de transfert (payeur) en échec");
        }
    }
    tracing::info!(trade_id = %payment.trade_id, "soulte capturée");
}

/// Libère le paiement d'un troc annulé (préautorisation relâchée) et prévient
/// le payeur. Idempotent — sans effet si rien n'était séquestré.
async fn release_payment_if_any(state: &AppState, trade_id: Uuid) {
    let payment = match infra::payment_repo::cancel_payment_for_trade(&state.pool, trade_id).await {
        Ok(payment) => payment,
        Err(error) => {
            tracing::error!(%error, %trade_id, "libération du paiement en échec");
            return;
        }
    };
    let Some(payment) = payment else { return };
    if let Some(provider_ref) = payment.provider_ref.as_deref() {
        if let Err(error) = state.payments.cancel(provider_ref).await {
            tracing::error!(%error, %trade_id, "libération de la préautorisation PSP en échec");
        }
    }
    // On ne prévient le payeur que si des fonds étaient réellement bloqués.
    if payment.escrowed_at.is_some() {
        telemetry::track(
            state,
            "payment_refunded",
            None,
            json!({"trade_id": trade_id, "amount_cents": payment.amount_cents,
                   "method": "preauth_cancel"}),
        )
        .await;
        if let Ok(Some(payer)) =
            infra::auth_repo::find_user_by_id(&state.pool, payment.payer_id).await
        {
            if let Err(error) = state
                .mailer
                .send_payment_cancelled_payer(&payer.email, &payer.pseudo, payment.amount_cents)
                .await
            {
                tracing::error!(%error, %trade_id, "e-mail de libération en échec");
            }
        }
    }
}

/// Annulation d'un commun accord : la première demande, l'autre confirme.
#[utoipa::path(
    post,
    path = "/trades/{id}/cancel",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    responses(
        (status = 200, description = "Demande enregistrée ou troc annulé", body = TradeDetailResponse),
        (status = 400, description = "Troc clos", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn cancel_trade(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let outcome = infra::trade_repo::request_cancel(&state.pool, id, user.user_id).await?;
    match outcome {
        infra::trade_repo::CancelOutcome::NotFound => {
            Err(ApiError::not_found("Ce troc n'existe pas."))
        }
        infra::trade_repo::CancelOutcome::NotActive(_) => Err(ApiError::bad_request(
            "troc_clos",
            "Ce troc n'est plus annulable.",
        )),
        infra::trade_repo::CancelOutcome::Pending => {
            let trade = participant_trade(&state, id, user.user_id).await?;
            Ok(Json(
                full_trade_response(&state, trade, user.user_id).await?,
            ))
        }
        infra::trade_repo::CancelOutcome::Cancelled => {
            telemetry::track(
                &state,
                "trade_cancelled_mutual",
                Some(user.user_id),
                json!({"trade_id": id}),
            )
            .await;
            release_payment_if_any(&state, id).await;
            let trade = participant_trade(&state, id, user.user_id).await?;
            Ok(Json(
                full_trade_response(&state, trade, user.user_id).await?,
            ))
        }
    }
}

/// Bilan de la maintenance horaire des trocs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MaintenanceReport {
    pub reminded: usize,
    pub cancelled: usize,
    /// Trocs annulés faute de paiement dans les temps (F4.2).
    pub payments_expired: usize,
    /// Captures retentées avec succès après un échec à la remise (F4.2).
    pub captures_retried: usize,
}

/// Maintenance des trocs : relance J+7, annulation automatique J+14
/// (Gherkin « rendez-vous fantôme »), expiration des paiements en retard,
/// rattrapage des captures. Tâche horaire, testable directement.
pub async fn maintain_trades(state: &AppState) -> MaintenanceReport {
    let mut report = MaintenanceReport::default();
    let mut reminded = 0;
    match infra::trade_repo::claim_meetup_reminders(&state.pool).await {
        Ok(parties) => {
            for party in &parties {
                if let Err(error) = state
                    .mailer
                    .send_trade_reminder(&party.email, &party.pseudo, &party.other_pseudo)
                    .await
                {
                    tracing::error!(%error, trade_id = %party.trade_id, "relance troc en échec");
                }
            }
            reminded = parties.len();
        }
        Err(error) => tracing::error!(%error, "relance des trocs en échec"),
    }

    let mut cancelled = 0;
    match infra::trade_repo::auto_cancel_stale_trades(&state.pool).await {
        Ok(parties) => {
            let mut seen: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
            for party in &parties {
                if seen.insert(party.trade_id) {
                    telemetry::track(
                        state,
                        "trade_auto_cancelled",
                        None,
                        json!({"trade_id": party.trade_id}),
                    )
                    .await;
                    // Un troc J+14 peut porter une soulte séquestrée : libérer.
                    release_payment_if_any(state, party.trade_id).await;
                }
                if let Err(error) = state
                    .mailer
                    .send_trade_auto_cancelled(&party.email, &party.pseudo, &party.other_pseudo)
                    .await
                {
                    tracing::error!(%error, trade_id = %party.trade_id, "e-mail d'annulation en échec");
                }
            }
            cancelled = seen.len();
        }
        Err(error) => tracing::error!(%error, "annulation automatique des trocs en échec"),
    }

    let (payments_expired, captures_retried) = maintain_payments(state).await;
    report.payments_expired = payments_expired;
    report.captures_retried = captures_retried;
    report.reminded = reminded;
    report.cancelled = cancelled;
    report
}

/// Maintenance des paiements (F4.2), plus fréquente que celle des trocs —
/// la date limite courte (30 min) ne peut pas attendre une tâche horaire.
/// Retourne (trocs annulés faute de paiement, captures retentées).
pub async fn maintain_payments(state: &AppState) -> (usize, usize) {
    // Trocs jamais payés : la date limite est passée, on annule.
    let mut expired = 0;
    match infra::payment_repo::overdue_unpaid_trades(&state.pool).await {
        Ok(trade_ids) => {
            for trade_id in trade_ids {
                match expire_unpaid_if_overdue(state, trade_id).await {
                    Ok(true) => expired += 1,
                    Ok(false) => {}
                    Err(error) => {
                        tracing::error!(%error, %trade_id, "expiration de paiement en échec")
                    }
                }
            }
        }
        Err(error) => tracing::error!(%error, "recherche des paiements en retard en échec"),
    }

    // Captures qui avaient échoué à la remise : on retente.
    let mut captures = 0;
    match infra::payment_repo::payments_to_capture(&state.pool).await {
        Ok(payments) => {
            for payment in payments {
                capture_payment(state, &payment).await;
                captures += 1;
            }
        }
        Err(error) => tracing::error!(%error, "recherche des captures à retenter en échec"),
    }
    (expired, captures)
}
