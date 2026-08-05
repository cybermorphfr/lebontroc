//! Handlers des propositions de troc : composer, boîtes, vue, refus.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Duration;
use domain::payment as paiement;
use domain::shipping as expedition;
use domain::trade as regles;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::messaging::ws::broadcast_event;
use crate::telemetry;
use crate::trade::dto::{
    AcceptProposalRequest, ConfigureShippingRequest, ConfirmTradeRequest, CreateProposalRequest,
    PayTradeRequest, PaymentInfo, ProposalItemResponse, ProposalResponse, RelayResponse,
    ReviewInfo, ReviewReplyRequest, ShipmentInfo, SubmitReviewRequest, TradeDetailResponse,
    TradeResponse, TradeReviews,
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

    // F5.2 — blocage (dans un sens ou l'autre) : message neutre, jamais
    // « tu es bloqué ». Restriction : plus de nouvelles propositions.
    if infra::dispute_repo::is_blocked_either_way(&state.pool, user.user_id, recipient_id).await? {
        return Err(ApiError::forbidden(
            "propositions_fermees",
            "Ce troqueur n'accepte pas de nouvelles propositions.",
        ));
    }
    let sanctions = infra::dispute_repo::sanction_state(&state.pool, user.user_id).await?;
    if sanctions
        .restricted_until
        .is_some_and(|until| until > chrono::Utc::now())
    {
        return Err(ApiError::forbidden(
            "compte_restreint",
            "Ton compte est restreint : pas de nouvelles propositions pour le moment. \
             Tes trocs en cours continuent normalement.",
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
    // Qui doit la soulte, le cas échéant.
    let cash_payer_id = if apercu.cash_cents > 0 {
        match paiement::payeur(&apercu.cash_direction) {
            Some(paiement::Payeur::Proposant) => Some(apercu.proposer_id),
            Some(paiement::Payeur::Destinataire) => Some(apercu.recipient_id),
            None => {
                return Err(ApiError::internal(
                    "soulte sans direction — donnée corrompue",
                ));
            }
        }
    } else {
        None
    };

    // F4.2/F4.3 : dès qu'il y a de l'argent en jeu (soulte et/ou frais
    // d'envoi), le troc naît en attente de paiement — préautorisations
    // séquestrées jusqu'à l'aboutissement.
    let mut payments: Vec<infra::payment_repo::NewPayment> = Vec::new();
    if body.delivery_mode == "envoi" {
        // Chaque partie paie son transport (fixé au choix du format), les
        // frais de service, et sa soulte éventuelle. 24 h pour les deux.
        for (payer_id, other_id) in [
            (apercu.proposer_id, apercu.recipient_id),
            (apercu.recipient_id, apercu.proposer_id),
        ] {
            let cash = if cash_payer_id == Some(payer_id) {
                apercu.cash_cents
            } else {
                0
            };
            payments.push(infra::payment_repo::NewPayment {
                payer_id,
                beneficiary_id: other_id,
                amount_cents: expedition::SERVICE_CENTS + cash,
                fees_cents: paiement::commission_cents(cash, state.config.payment_fees_bps),
                service_cents: expedition::SERVICE_CENTS,
                provider: state.payments.name().to_string(),
                deadline: chrono::Utc::now()
                    + Duration::minutes(paiement::DELAI_PAIEMENT_AUTRE_MINUTES),
            });
        }
    } else if let Some(payer_id) = cash_payer_id {
        let beneficiary_id = if payer_id == apercu.proposer_id {
            apercu.recipient_id
        } else {
            apercu.proposer_id
        };
        let delai = paiement::delai_paiement_minutes(payer_id == user.user_id);
        payments.push(infra::payment_repo::NewPayment {
            payer_id,
            beneficiary_id,
            amount_cents: apercu.cash_cents,
            fees_cents: paiement::commission_cents(
                apercu.cash_cents,
                state.config.payment_fees_bps,
            ),
            service_cents: 0,
            provider: state.payments.name().to_string(),
            deadline: chrono::Utc::now() + Duration::minutes(delai),
        });
    }

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
        &payments,
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

            // L'autre partie n'est pas devant l'écran : la prévenir qu'un
            // règlement l'attend (soulte en main propre, envoi à préparer).
            let other_id = if apercu.proposer_id == user.user_id {
                apercu.recipient_id
            } else {
                apercu.proposer_id
            };
            let other_must_act =
                body.delivery_mode == "envoi" || payments.iter().any(|p| p.payer_id == other_id);
            if other_must_act {
                match infra::auth_repo::find_user_by_id(&state.pool, other_id).await {
                    Ok(Some(other)) => {
                        let my_pseudo = if apercu.proposer_id == user.user_id {
                            &apercu.proposer_pseudo
                        } else {
                            &apercu.recipient_pseudo
                        };
                        let sent = if body.delivery_mode == "envoi" {
                            state
                                .mailer
                                .send_shipping_setup(&other.email, &other.pseudo, my_pseudo)
                                .await
                        } else {
                            let amount = payments
                                .iter()
                                .find(|p| p.payer_id == other_id)
                                .map(|p| p.amount_cents)
                                .unwrap_or(0);
                            state
                                .mailer
                                .send_payment_due(
                                    &other.email,
                                    &other.pseudo,
                                    my_pseudo,
                                    amount,
                                    paiement::DELAI_PAIEMENT_AUTRE_MINUTES / 60,
                                )
                                .await
                        };
                        if let Err(error) = sent {
                            tracing::error!(%error, trade_id = %trade.id,
                                "e-mail de règlement à effectuer en échec");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => tracing::error!(%error, "autre partie introuvable"),
                }
            }
            if !payments.is_empty() {
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

fn payment_info(p: &infra::payment_repo::Payment, user_id: Uuid) -> PaymentInfo {
    PaymentInfo {
        status: p.status.clone(),
        amount_cents: p.amount_cents,
        shipping_cents: p.shipping_cents,
        service_cents: p.service_cents,
        cash_cents: p.cash_cents(),
        fees_cents: p.fees_cents,
        net_cents: paiement::net_beneficiaire_cents(p.cash_cents(), p.fees_cents),
        i_am_payer: p.payer_id == user_id,
        deadline: p.deadline,
        failure_reason: p.failure_reason.clone(),
        secure_mode_url: None,
    }
}

fn trade_detail_response(
    trade: infra::trade_repo::TradeDetail,
    payments: &[infra::payment_repo::Payment],
    shipments: &[infra::shipping_repo::Shipment],
    user_id: Uuid,
) -> TradeDetailResponse {
    let is_proposer = trade.proposer_id == user_id;
    // MON paiement d'abord (mode envoi : chacun le sien) ; à défaut celui
    // de l'autre (main propre avec soulte, vue du bénéficiaire).
    let mine = payments.iter().find(|p| p.payer_id == user_id);
    let shown = mine.or_else(|| payments.first());
    let other_payment_status = payments
        .iter()
        .find(|p| p.payer_id != user_id)
        .filter(|_| mine.is_some())
        .map(|p| p.status.clone());
    let shipment_infos = shipments
        .iter()
        .map(|s| {
            let i_am_sender = s.sender_id == user_id;
            ShipmentInfo {
                id: s.id,
                i_am_sender,
                status: s.status.clone(),
                format: s.format.clone(),
                relay_code: s.relay_code.clone(),
                relay_name: s.relay_name.clone(),
                relay_address: s.relay_address.clone(),
                drop_code: if i_am_sender {
                    s.drop_code.clone()
                } else {
                    None
                },
                dropped_at: s.dropped_at,
                arrived_at: s.arrived_at,
                picked_up_at: s.picked_up_at,
                confirmed_at: s.confirmed_at,
                confirmation_deadline: s
                    .picked_up_at
                    .map(|t| t + Duration::hours(expedition::CONFIRMATION_HEURES)),
                issue_reason: s.issue_reason.clone(),
            }
        })
        .collect();
    // Le code de remise (main propre) n'a pas cours avant séquestre.
    let code_active = trade.status != "attente_paiement" && trade.delivery_mode == "main_propre";
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
        payment: shown.map(|p| payment_info(p, user_id)),
        other_payment_status,
        shipments: shipment_infos,
        reviews: TradeReviews {
            mine: None,
            received: None,
        },
        dispute: None,
    }
}

/// Vue complète du troc : paiements, colis et évaluations compris.
pub(crate) async fn full_trade_response(
    state: &AppState,
    trade: infra::trade_repo::TradeDetail,
    user_id: Uuid,
) -> Result<TradeDetailResponse, ApiError> {
    let payments = infra::payment_repo::payments_for_trade(&state.pool, trade.id).await?;
    let shipments = if trade.delivery_mode == "envoi" {
        infra::shipping_repo::shipments_for_trade(&state.pool, trade.id).await?
    } else {
        Vec::new()
    };
    let reviews = if trade.status == "finalise" {
        infra::review_repo::reviews_for_trade(&state.pool, trade.id).await?
    } else {
        Vec::new()
    };
    let mut response = trade_detail_response(trade, &payments, &shipments, user_id);
    let to_info = |r: &infra::review_repo::Review| ReviewInfo {
        id: r.id,
        rating: r.rating,
        comment: r.comment.clone(),
        published: r.published_at.is_some(),
        reply: r.reply.clone(),
        created_at: r.created_at,
    };
    response.reviews = TradeReviews {
        // Ma note : toujours visible pour moi. Celle de l'autre : seulement
        // une fois publiée (embargo anti-représailles).
        mine: reviews
            .iter()
            .find(|r| r.reviewer_id == user_id)
            .map(to_info),
        received: reviews
            .iter()
            .find(|r| r.reviewee_id == user_id && r.published_at.is_some())
            .map(to_info),
    };
    response.dispute = infra::dispute_repo::dispute_for_trade(&state.pool, response.id)
        .await?
        .map(|d| crate::dispute::dto::DisputeInfo {
            opened_by_me: d.opened_by == Some(user_id),
            can_respond: d.status == "ouvert"
                && d.opened_by != Some(user_id)
                && d.response.is_none(),
            id: d.id,
            reason: d.reason,
            description: d.description,
            status: d.status,
            response: d.response,
            outcome: d.outcome,
            opened_at: d.opened_at,
        });
    Ok(response)
}

pub(crate) async fn participant_trade(
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

/// Annule le troc si un de ses paiements a dépassé la date limite :
/// paiements en retard `expire`, préautorisations déjà posées libérées,
/// troc `annule`, objets libérés, parties notifiées. Retourne `true` si
/// l'annulation vient d'avoir lieu.
async fn expire_unpaid_if_overdue(state: &AppState, trade_id: Uuid) -> sqlx::Result<bool> {
    let payments = infra::payment_repo::payments_for_trade(&state.pool, trade_id).await?;
    let now = chrono::Utc::now();
    if !payments
        .iter()
        .any(|p| paiement::peut_expirer(&p.status) && p.deadline < now)
    {
        return Ok(false);
    }
    let (parties, released) =
        infra::payment_repo::expire_unpaid_trade(&state.pool, trade_id).await?;
    if parties.is_empty() {
        return Ok(false);
    }
    // Mode envoi : l'autre partie avait pu payer — relâcher sa préautorisation.
    for payment in &released {
        notify_released_preauth(state, payment).await;
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
    tracing::info!(%trade_id, "troc annulé : règlement jamais effectué");
    Ok(true)
}

/// Relâche une préautorisation côté PSP et prévient le payeur — utilisé
/// chaque fois qu'un paiement séquestré vient d'être marqué `annule`.
async fn notify_released_preauth(state: &AppState, payment: &infra::payment_repo::Payment) {
    if let Some(provider_ref) = payment.provider_ref.as_deref() {
        if let Err(error) = state.payments.cancel(provider_ref).await {
            tracing::error!(%error, trade_id = %payment.trade_id,
                "libération de la préautorisation PSP en échec");
        }
    }
    if payment.escrowed_at.is_none() {
        return;
    }
    telemetry::track(
        state,
        "payment_refunded",
        None,
        json!({"trade_id": payment.trade_id, "amount_cents": payment.amount_cents,
               "method": "preauth_cancel"}),
    )
    .await;
    if let Ok(Some(payer)) = infra::auth_repo::find_user_by_id(&state.pool, payment.payer_id).await
    {
        if let Err(error) = state
            .mailer
            .send_payment_cancelled_payer(&payer.email, &payer.pseudo, payment.amount_cents)
            .await
        {
            tracing::error!(%error, trade_id = %payment.trade_id, "e-mail de libération en échec");
        }
    }
}

/// Préautoriser mon règlement (soulte F4.2 et/ou frais d'envoi F4.3) —
/// réservé au payeur, tant que la date limite n'est pas dépassée. Bêta
/// fermée : PSP simulé, aucune carte réelle.
#[utoipa::path(
    post,
    path = "/trades/{id}/pay",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = PayTradeRequest,
    responses(
        (status = 200, description = "Règlement séquestré (ou déjà séquestré)", body = TradeDetailResponse),
        (status = 400, description = "Refus, délai dépassé ou rien à payer", body = crate::error::ErrorResponse),
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
    let Some(payment) =
        infra::payment_repo::payment_for_payer(&state.pool, id, user.user_id).await?
    else {
        return Err(ApiError::bad_request(
            "pas_le_payeur",
            "Tu n'as rien à régler sur ce troc.",
        ));
    };
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
    // Mode envoi : le format fixe le transport — pas de paiement à l'aveugle.
    if trade.delivery_mode == "envoi" && payment.shipping_cents == 0 {
        return Err(ApiError::bad_request(
            "envoi_non_configure",
            "Choisis d'abord le format de ton colis et ton point relais.",
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

    let reference = format!("payment-{}", payment.id);
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
            match infra::payment_repo::escrow_payment(&state.pool, payment.id, Some(&provider_ref))
                .await?
            {
                infra::payment_repo::EscrowOutcome::NotPending => {
                    return Err(ApiError::bad_request(
                        "troc_clos",
                        "Ce troc n'attend plus de paiement.",
                    ))
                }
                infra::payment_repo::EscrowOutcome::AlreadyEscrowed => {}
                infra::payment_repo::EscrowOutcome::Escrowed {
                    trade_activated,
                    evictions,
                } => {
                    telemetry::track(
                        &state,
                        "payment_escrowed",
                        Some(user.user_id),
                        json!({"trade_id": id, "amount_cents": payment.amount_cents}),
                    )
                    .await;
                    notify_evictions(&state, evictions).await;
                    // La soulte séquestrée mérite un e-mail au bénéficiaire ;
                    // les simples frais d'envoi, non.
                    if payment.cash_cents() > 0 {
                        match infra::auth_repo::find_user_by_id(&state.pool, payment.beneficiary_id)
                            .await
                        {
                            Ok(Some(beneficiary)) => {
                                let payer_pseudo = infra::auth_repo::find_user_by_id(
                                    &state.pool,
                                    payment.payer_id,
                                )
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
                                        payment.cash_cents(),
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
                    }
                    let trade = participant_trade(&state, id, user.user_id).await?;
                    // Mode envoi : le troc activé peut générer ses étiquettes.
                    if trade.delivery_mode == "envoi" && trade_activated {
                        ensure_labels(&state, id).await;
                    }
                    broadcast_event(
                        &state,
                        [trade.proposer_id, trade.recipient_id],
                        json!({"type": "trade_updated", "proposal_id": trade.proposal_id,
                               "trade_id": id}),
                    );
                    tracing::info!(trade_id = %id, "règlement séquestré");
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

/// Génère les étiquettes des colis prêts (troc actif, payé, format et
/// relais connus) — idempotent, appelé après chaque événement déclencheur.
async fn ensure_labels(state: &AppState, trade_id: Uuid) {
    let ready = match infra::shipping_repo::shipments_ready_for_label(&state.pool, trade_id).await {
        Ok(ready) => ready,
        Err(error) => {
            tracing::error!(%error, %trade_id, "recherche des colis à étiqueter en échec");
            return;
        }
    };
    for shipment in ready {
        let reference = format!("shipment-{}", shipment.id);
        let (Some(format), Some(relay_code)) = (&shipment.format, &shipment.relay_code) else {
            continue;
        };
        match state
            .shipping
            .create_label(infra::shipping::LabelRequest {
                reference: &reference,
                format,
                relay_code,
            })
            .await
        {
            Ok(label) => {
                match infra::shipping_repo::mark_labeled(
                    &state.pool,
                    shipment.id,
                    state.shipping.name(),
                    &label.provider_ref,
                    &label.drop_code,
                )
                .await
                {
                    Ok(true) => {
                        telemetry::track(
                            state,
                            "shipping_label_generated",
                            None,
                            json!({"trade_id": trade_id, "shipment_id": shipment.id,
                                   "format": format}),
                        )
                        .await;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        tracing::error!(%error, shipment_id = %shipment.id,
                            "enregistrement d'étiquette en échec")
                    }
                }
            }
            Err(error) => tracing::error!(%error, shipment_id = %shipment.id,
                "génération d'étiquette en échec"),
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
    // Les codes croisés sont le rituel de la main propre ; en mode envoi,
    // la finalisation passe par la réception des deux colis.
    let apercu = participant_trade(&state, id, user.user_id).await?;
    if apercu.delivery_mode == "envoi" {
        return Err(ApiError::bad_request(
            "mode_envoi",
            "Ce troc se finalise à la réception des colis, pas par codes.",
        ));
    }
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
                // F5.2 : la capture main propre n'est plus immédiate — la
                // maintenance capturera 48 h après la remise, sauf litige
                // ouvert dans la fenêtre (choix Brian).
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

/// Capture un règlement séquestré (échange abouti) : PSP, transition SQL,
/// e-mails, télémétrie. En cas d'échec PSP le paiement reste `sequestre` —
/// la maintenance horaire retentera.
pub(crate) async fn capture_payment(state: &AppState, payment: &infra::payment_repo::Payment) {
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
            "capture du règlement en échec — la maintenance retentera");
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
    // Le transfert de soulte mérite ses e-mails ; la simple capture des
    // frais d'envoi, non (l'e-mail de finalisation suffit).
    let cash = payment.cash_cents();
    if cash > 0 {
        let net = paiement::net_beneficiaire_cents(cash, payment.fees_cents);
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
                .send_payment_released_payer(&payer.email, &payer.pseudo, &beneficiary.pseudo, cash)
                .await
            {
                tracing::error!(%error, "e-mail de transfert (payeur) en échec");
            }
        }
    }
    tracing::info!(trade_id = %payment.trade_id, "règlement capturé");
}

/// Libère les paiements d'un troc annulé (préautorisations relâchées) et
/// prévient les payeurs. Idempotent — sans effet si rien n'était en cours.
pub(crate) async fn release_payments_if_any(state: &AppState, trade_id: Uuid) {
    let payments = match infra::payment_repo::cancel_payments_for_trade(&state.pool, trade_id).await
    {
        Ok(payments) => payments,
        Err(error) => {
            tracing::error!(%error, %trade_id, "libération des paiements en échec");
            return;
        }
    };
    for payment in &payments {
        notify_released_preauth(state, payment).await;
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
        infra::trade_repo::CancelOutcome::ParcelsMoving => Err(ApiError::bad_request(
            "colis_en_route",
            "Un colis a déjà voyagé : le troc ne peut plus être annulé à l'amiable. \
             Signale un problème à la réception si besoin.",
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
            release_payments_if_any(&state, id).await;
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
    /// Rappels de dépôt envoyés (F4.3, J+2 et J+4).
    pub drop_reminders: usize,
    /// Trocs envoi annulés (aucun dépôt J+5).
    pub shipping_cancelled: usize,
    /// Trocs envoi gelés pour examen (dépôt partiel J+5, zombie J+21).
    pub shipping_frozen: usize,
    /// Colis confirmés automatiquement (72 h après retrait).
    pub auto_confirmed: usize,
    /// Évaluations orphelines publiées à J+14 (F5.1).
    pub reviews_published: usize,
    /// Dossiers passés en examen faute de réponse sous 72 h (F5.2).
    pub disputes_escalated: usize,
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
                    release_payments_if_any(state, party.trade_id).await;
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
    maintain_shipping(state, &mut report).await;
    report.auto_confirmed = auto_confirm_shipments(state).await;

    // F5.1 — les notes orphelines sont publiées J+14 après la finalisation.
    match infra::review_repo::publish_overdue_reviews(
        &state.pool,
        domain::review::PUBLICATION_JOURS,
    )
    .await
    {
        Ok(published) => {
            if published > 0 {
                telemetry::track(
                    state,
                    "review_published",
                    None,
                    json!({"count": published, "reason": "deadline"}),
                )
                .await;
            }
            report.reviews_published = published as usize;
        }
        Err(error) => tracing::error!(%error, "publication des évaluations J+14 en échec"),
    }

    // F5.2 — sans réponse contradictoire sous 72 h, le dossier part en examen.
    match infra::dispute_repo::escalate_unanswered_disputes(
        &state.pool,
        domain::dispute::REPONSE_HEURES,
    )
    .await
    {
        Ok(escalated) => report.disputes_escalated = escalated as usize,
        Err(error) => tracing::error!(%error, "escalade des dossiers 72 h en échec"),
    }

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

    // Captures dues : main propre 48 h après la remise (fenêtre litige
    // F5.2), et rattrapage des captures envoi échouées.
    let mut captures = 0;
    match infra::payment_repo::payments_to_capture(
        &state.pool,
        domain::dispute::FENETRE_MAIN_PROPRE_HEURES,
    )
    .await
    {
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

// ————— Envoi croisé (F4.3) —————

/// Les points relais proches de chez moi (pour recevoir l'autre colis).
#[utoipa::path(
    get,
    path = "/trades/{id}/relays",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    responses(
        (status = 200, description = "Relais proposés", body = [RelayResponse]),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn trade_relays(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<RelayResponse>>, ApiError> {
    participant_trade(&state, id, user.user_id).await?;
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    let relays = state
        .shipping
        .search_relays(&me.postal_code)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(
        relays
            .into_iter()
            .map(|r| RelayResponse {
                code: r.code,
                name: r.name,
                address: r.address,
            })
            .collect(),
    ))
}

/// Configurer mon envoi : le format de MON colis (fixe mon transport) et le
/// relais où je recevrai le sien. Modifiable tant que je n'ai pas payé.
#[utoipa::path(
    post,
    path = "/trades/{id}/shipping",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = ConfigureShippingRequest,
    responses(
        (status = 200, description = "Envoi configuré", body = TradeDetailResponse),
        (status = 400, description = "Format ou relais invalide, ou trop tard", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn configure_shipping(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ConfigureShippingRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let trade = participant_trade(&state, id, user.user_id).await?;
    if trade.delivery_mode != "envoi" {
        return Err(ApiError::bad_request(
            "mode_main_propre",
            "Ce troc se fait en main propre — rien à expédier.",
        ));
    }
    let Some(transport) = expedition::transport_cents(&body.format) else {
        return Err(ApiError::bad_request(
            "format_inconnu",
            "Choisis un format S (≤ 1 kg), M (≤ 3 kg) ou L (≤ 10 kg).",
        ));
    };
    // Le relais vient de la liste proposée — on revalide et on récupère
    // son nom et son adresse au passage.
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    let relays = state
        .shipping
        .search_relays(&me.postal_code)
        .await
        .map_err(ApiError::internal)?;
    let Some(relay) = relays.iter().find(|r| r.code == body.relay_code) else {
        return Err(ApiError::bad_request(
            "relais_inconnu",
            "Ce point relais ne fait pas partie de ceux proposés — recharge la liste.",
        ));
    };
    let configured = infra::shipping_repo::configure_my_shipping(
        &state.pool,
        id,
        user.user_id,
        &body.format,
        transport,
        &relay.code,
        &relay.name,
        &relay.address,
    )
    .await?;
    if !configured {
        return Err(ApiError::bad_request(
            "envoi_fige",
            "Ton envoi n'est plus modifiable (déjà payé ou étiquette générée).",
        ));
    }
    let trade = participant_trade(&state, id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Vérifie que je suis bien une partie du colis, et le retourne.
async fn my_shipment(
    state: &AppState,
    shipment_id: Uuid,
    user_id: Uuid,
) -> Result<infra::shipping_repo::Shipment, ApiError> {
    infra::shipping_repo::get_shipment(&state.pool, shipment_id)
        .await?
        .filter(|s| s.sender_id == user_id || s.recipient_id == user_id)
        .ok_or_else(|| ApiError::not_found("Ce colis n'existe pas."))
}

/// J'ai déposé mon colis au point relais. Le simulateur de la bêta le fait
/// « arriver » immédiatement chez le destinataire.
#[utoipa::path(
    post,
    path = "/shipments/{id}/drop",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du colis")),
    responses(
        (status = 200, description = "Dépôt enregistré", body = TradeDetailResponse),
        (status = 400, description = "Pas d'étiquette ou déjà déposé", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn drop_parcel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let shipment = my_shipment(&state, id, user.user_id).await?;
    if !infra::shipping_repo::mark_dropped(&state.pool, id, user.user_id).await? {
        return Err(ApiError::bad_request(
            "depot_impossible",
            "Ce colis n'a pas d'étiquette à déposer (ou l'est déjà).",
        ));
    }
    telemetry::track(
        &state,
        "parcel_dropped",
        Some(user.user_id),
        json!({"trade_id": shipment.trade_id, "shipment_id": id}),
    )
    .await;

    // Suivi : le simulateur répond « arrivé en relais » immédiatement ; le
    // transporteur réel avancera par webhooks/polling.
    let status = state
        .shipping
        .tracking_status(shipment.provider_ref.as_deref().unwrap_or(""))
        .await;
    if let Ok(infra::shipping::TrackingStatus::ArrivedAtRelay) = status {
        if infra::shipping_repo::mark_arrived(&state.pool, id).await? {
            telemetry::track(
                &state,
                "parcel_delivered",
                None,
                json!({"trade_id": shipment.trade_id, "shipment_id": id}),
            )
            .await;
            if let Ok(Some(recipient)) =
                infra::auth_repo::find_user_by_id(&state.pool, shipment.recipient_id).await
            {
                let sender_pseudo =
                    infra::auth_repo::find_user_by_id(&state.pool, shipment.sender_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|u| u.pseudo)
                        .unwrap_or_default();
                if let Err(error) = state
                    .mailer
                    .send_parcel_arrived(
                        &recipient.email,
                        &recipient.pseudo,
                        &sender_pseudo,
                        shipment.relay_name.as_deref().unwrap_or("point relais"),
                    )
                    .await
                {
                    tracing::error!(%error, shipment_id = %id, "e-mail d'arrivée en échec");
                }
            }
        }
    }
    broadcast_event(
        &state,
        [shipment.sender_id, shipment.recipient_id],
        json!({"type": "trade_updated", "trade_id": shipment.trade_id}),
    );
    let trade = participant_trade(&state, shipment.trade_id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// J'ai récupéré le colis au relais — la fenêtre de 72 h démarre.
#[utoipa::path(
    post,
    path = "/shipments/{id}/pickup",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du colis")),
    responses(
        (status = 200, description = "Retrait enregistré", body = TradeDetailResponse),
        (status = 400, description = "Colis pas encore arrivé", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn pickup_parcel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let shipment = my_shipment(&state, id, user.user_id).await?;
    if !infra::shipping_repo::mark_picked_up(&state.pool, id, user.user_id).await? {
        return Err(ApiError::bad_request(
            "retrait_impossible",
            "Ce colis n'est pas encore arrivé au relais (ou est déjà retiré).",
        ));
    }
    telemetry::track(
        &state,
        "parcel_picked_up",
        Some(user.user_id),
        json!({"trade_id": shipment.trade_id, "shipment_id": id}),
    )
    .await;
    broadcast_event(
        &state,
        [shipment.sender_id, shipment.recipient_id],
        json!({"type": "trade_updated", "trade_id": shipment.trade_id}),
    );
    let trade = participant_trade(&state, shipment.trade_id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Tout est OK : je confirme le colis reçu. Les deux confirmations
/// finalisent le troc et capturent les règlements (Gherkin F4.3).
#[utoipa::path(
    post,
    path = "/shipments/{id}/confirm",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du colis")),
    responses(
        (status = 200, description = "Réception confirmée", body = TradeDetailResponse),
        (status = 400, description = "Colis pas encore retiré", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn confirm_parcel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let shipment = my_shipment(&state, id, user.user_id).await?;
    if !infra::shipping_repo::confirm_shipment(&state.pool, id, user.user_id).await? {
        return Err(ApiError::bad_request(
            "confirmation_impossible",
            "Retire d'abord le colis au relais avant de confirmer.",
        ));
    }
    telemetry::track(
        &state,
        "delivery_confirmed",
        Some(user.user_id),
        json!({"trade_id": shipment.trade_id, "shipment_id": id, "auto": false}),
    )
    .await;
    maybe_finalize_shipping(&state, shipment.trade_id).await;
    broadcast_event(
        &state,
        [shipment.sender_id, shipment.recipient_id],
        json!({"type": "trade_updated", "trade_id": shipment.trade_id}),
    );
    let trade = participant_trade(&state, shipment.trade_id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Finalise le troc envoi si les deux colis sont confirmés : troc
/// `finalise`, objets `troque`, règlements capturés, e-mails aux deux.
async fn maybe_finalize_shipping(state: &AppState, trade_id: Uuid) {
    match infra::shipping_repo::finalize_shipping_trade(&state.pool, trade_id).await {
        Ok(false) => {}
        Ok(true) => {
            telemetry::track(
                state,
                "trade_finalized",
                None,
                json!({"trade_id": trade_id, "delivery_mode": "envoi"}),
            )
            .await;
            match infra::payment_repo::payments_for_trade(&state.pool, trade_id).await {
                Ok(payments) => {
                    for payment in payments {
                        if paiement::peut_capturer(&payment.status) {
                            capture_payment(state, &payment).await;
                        }
                    }
                }
                Err(error) => tracing::error!(%error, %trade_id, "paiements introuvables"),
            }
            if let Ok(Some(trade)) = infra::trade_repo::get_trade(&state.pool, trade_id).await {
                for (me, other) in [
                    (trade.proposer_id, trade.recipient_id),
                    (trade.recipient_id, trade.proposer_id),
                ] {
                    let user = infra::auth_repo::find_user_by_id(&state.pool, me)
                        .await
                        .ok()
                        .flatten();
                    let other_pseudo = infra::auth_repo::find_user_by_id(&state.pool, other)
                        .await
                        .ok()
                        .flatten()
                        .map(|u| u.pseudo)
                        .unwrap_or_default();
                    if let Some(user) = user {
                        if let Err(error) = state
                            .mailer
                            .send_trade_finalized_shipping(&user.email, &user.pseudo, &other_pseudo)
                            .await
                        {
                            tracing::error!(%error, %trade_id, "e-mail de finalisation en échec");
                        }
                    }
                }
                broadcast_event(
                    state,
                    [trade.proposer_id, trade.recipient_id],
                    json!({"type": "trade_updated", "trade_id": trade_id}),
                );
            }
            tracing::info!(%trade_id, "troc envoi finalisé");
        }
        Err(error) => tracing::error!(%error, %trade_id, "finalisation envoi en échec"),
    }
}

/// Prévient les parties et l'admin qu'un troc vient d'être gelé.
async fn notify_frozen_trade(state: &AppState, trade_id: Uuid, details: &str) {
    if let Err(error) = state
        .mailer
        .send_admin_dispute(&state.config.admin_email, &trade_id.to_string(), details)
        .await
    {
        tracing::error!(%error, %trade_id, "e-mail admin de litige en échec");
    }
    if let Ok(Some(trade)) = infra::trade_repo::get_trade(&state.pool, trade_id).await {
        for (me, other) in [
            (trade.proposer_id, trade.recipient_id),
            (trade.recipient_id, trade.proposer_id),
        ] {
            let user = infra::auth_repo::find_user_by_id(&state.pool, me)
                .await
                .ok()
                .flatten();
            let other_pseudo = infra::auth_repo::find_user_by_id(&state.pool, other)
                .await
                .ok()
                .flatten()
                .map(|u| u.pseudo)
                .unwrap_or_default();
            if let Some(user) = user {
                if let Err(error) = state
                    .mailer
                    .send_shipping_failed(&user.email, &user.pseudo, &other_pseudo, true)
                    .await
                {
                    tracing::error!(%error, %trade_id, "e-mail de gel en échec");
                }
            }
        }
    }
}

/// Auto-confirmation des colis retirés depuis plus de 72 h — boucle rapide
/// (10 min), car elle peut finaliser un troc et capturer des règlements.
pub async fn auto_confirm_shipments(state: &AppState) -> usize {
    let trade_ids = match infra::shipping_repo::claim_auto_confirmations(
        &state.pool,
        expedition::CONFIRMATION_HEURES,
    )
    .await
    {
        Ok(ids) => ids,
        Err(error) => {
            tracing::error!(%error, "auto-confirmation des colis en échec");
            return 0;
        }
    };
    let count = trade_ids.len();
    for trade_id in &trade_ids {
        telemetry::track(
            state,
            "delivery_confirmed",
            None,
            json!({"trade_id": trade_id, "auto": true}),
        )
        .await;
        maybe_finalize_shipping(state, *trade_id).await;
    }
    count
}

/// Maintenance horaire de l'envoi : rappels de dépôt J+2/J+4, échec de
/// dépôt J+5 (annulation ou gel), filet J+21.
async fn maintain_shipping(state: &AppState, report: &mut MaintenanceReport) {
    // Rappels de dépôt.
    for (level, days) in expedition::RAPPEL_DEPOT_JOURS.iter().enumerate() {
        match infra::shipping_repo::claim_drop_reminders(&state.pool, *days, level as i32 + 1).await
        {
            Ok(reminders) => {
                for reminder in &reminders {
                    if let Err(error) = state
                        .mailer
                        .send_drop_reminder(
                            &reminder.email,
                            &reminder.pseudo,
                            &reminder.other_pseudo,
                            reminder.level == 2,
                        )
                        .await
                    {
                        tracing::error!(%error, shipment_id = %reminder.shipment_id,
                            "rappel de dépôt en échec");
                    }
                }
                report.drop_reminders += reminders.len();
            }
            Err(error) => tracing::error!(%error, "rappels de dépôt en échec"),
        }
    }

    // Échec de dépôt J+5 : aucun colis parti → annulation propre ; un seul
    // parti → gel + journal des défaillances (F5.2 sanctionnera).
    match infra::shipping_repo::overdue_drop_trades(&state.pool, expedition::ECHEC_DEPOT_JOURS)
        .await
    {
        Ok(trades) => {
            for (trade_id, dropped_count) in trades {
                if dropped_count == 0 {
                    // Personne n'a expédié : annulation simple, pas un litige.
                    match infra::shipping_repo::cancel_shipping_trade(&state.pool, trade_id).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(error) => {
                            tracing::error!(%error, %trade_id, "annulation J+5 en échec");
                            continue;
                        }
                    }
                    release_payments_if_any(state, trade_id).await;
                    notify_shipping_cancelled(state, trade_id).await;
                    report.shipping_cancelled += 1;
                } else {
                    let culprit = infra::shipping_repo::undropped_sender(&state.pool, trade_id)
                        .await
                        .ok()
                        .flatten();
                    match infra::shipping_repo::freeze_trade(&state.pool, trade_id).await {
                        Ok(true) => {}
                        _ => continue,
                    }
                    if let Err(error) = infra::shipping_repo::record_dispute_event(
                        &state.pool,
                        trade_id,
                        "non_depot",
                        culprit,
                        Some("un seul colis expédié à J+5"),
                    )
                    .await
                    {
                        tracing::error!(%error, %trade_id, "journal de litige en échec");
                    }
                    // F5.2 : dossier système + sanctions automatiques du
                    // fautif (le non-dépôt pèse 5 points).
                    if let Err(error) = infra::dispute_repo::open_dispute(
                        &state.pool,
                        trade_id,
                        None,
                        "non_depot",
                        "Dossier créé automatiquement : un seul colis déposé à J+5.",
                    )
                    .await
                    {
                        tracing::error!(%error, %trade_id, "dossier système J+5 en échec");
                    }
                    if let Some(culprit) = culprit {
                        crate::dispute::handlers::apply_score_sanctions(state, culprit).await;
                    }
                    release_payments_if_any(state, trade_id).await;
                    notify_frozen_trade(state, trade_id, "un seul colis expédié à J+5").await;
                    report.shipping_frozen += 1;
                }
            }
        }
        Err(error) => tracing::error!(%error, "recherche des dépôts en retard en échec"),
    }

    // Filet J+21 : trocs envoi zombies → gel pour examen manuel.
    match infra::shipping_repo::stale_shipping_trades(&state.pool, expedition::GEL_TROC_ENVOI_JOURS)
        .await
    {
        Ok(trade_ids) => {
            for trade_id in trade_ids {
                match infra::shipping_repo::freeze_trade(&state.pool, trade_id).await {
                    Ok(true) => {
                        if let Err(error) = infra::shipping_repo::record_dispute_event(
                            &state.pool,
                            trade_id,
                            "troc_envoi_bloque",
                            None,
                            Some("toujours ouvert à J+21"),
                        )
                        .await
                        {
                            tracing::error!(%error, %trade_id, "journal de litige en échec");
                        }
                        if let Err(error) = infra::dispute_repo::open_dispute(
                            &state.pool,
                            trade_id,
                            None,
                            "non_depot",
                            "Dossier créé automatiquement : troc envoi toujours ouvert à J+21.",
                        )
                        .await
                        {
                            tracing::error!(%error, %trade_id, "dossier système J+21 en échec");
                        }
                        notify_frozen_trade(state, trade_id, "troc envoi toujours ouvert à J+21")
                            .await;
                        report.shipping_frozen += 1;
                    }
                    Ok(false) => {}
                    Err(error) => tracing::error!(%error, %trade_id, "gel J+21 en échec"),
                }
            }
        }
        Err(error) => tracing::error!(%error, "recherche des trocs envoi bloqués en échec"),
    }
}

/// E-mails d'annulation J+5 (aucun colis déposé).
async fn notify_shipping_cancelled(state: &AppState, trade_id: Uuid) {
    telemetry::track(
        state,
        "trade_auto_cancelled",
        None,
        json!({"trade_id": trade_id, "reason": "aucun_depot"}),
    )
    .await;
    if let Ok(Some(trade)) = infra::trade_repo::get_trade(&state.pool, trade_id).await {
        for (me, other) in [
            (trade.proposer_id, trade.recipient_id),
            (trade.recipient_id, trade.proposer_id),
        ] {
            let user = infra::auth_repo::find_user_by_id(&state.pool, me)
                .await
                .ok()
                .flatten();
            let other_pseudo = infra::auth_repo::find_user_by_id(&state.pool, other)
                .await
                .ok()
                .flatten()
                .map(|u| u.pseudo)
                .unwrap_or_default();
            if let Some(user) = user {
                if let Err(error) = state
                    .mailer
                    .send_shipping_failed(&user.email, &user.pseudo, &other_pseudo, false)
                    .await
                {
                    tracing::error!(%error, %trade_id, "e-mail d'annulation envoi en échec");
                }
            }
        }
    }
}

// ————— Évaluations (F5.1) —————

/// Noter l'autre partie d'un troc finalisé (1–5 + commentaire). Publication
/// simultanée : la note reste sous embargo tant que l'autre n'a pas noté —
/// ou jusqu'à J+14 (Gherkin anti-représailles).
#[utoipa::path(
    post,
    path = "/trades/{id}/review",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = SubmitReviewRequest,
    responses(
        (status = 200, description = "Note enregistrée", body = TradeDetailResponse),
        (status = 400, description = "Note invalide ou troc non finalisé", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse),
        (status = 409, description = "Déjà noté", body = crate::error::ErrorResponse)
    )
)]
pub async fn submit_review(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SubmitReviewRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let trade = participant_trade(&state, id, user.user_id).await?;
    let map_review_error = |error: domain::review::ReviewError| match error {
        domain::review::ReviewError::NoteHorsBornes => {
            ApiError::bad_request("note_invalide", "La note va de 1 à 5 étoiles.")
        }
        domain::review::ReviewError::CommentaireTropLong => ApiError::bad_request(
            "commentaire_trop_long",
            "Le commentaire est limité à 500 caractères.",
        ),
        domain::review::ReviewError::TrocNonFinalise => {
            ApiError::bad_request("troc_non_finalise", "On ne note qu'un troc finalisé.")
        }
    };
    domain::review::peut_noter(&trade.status).map_err(map_review_error)?;
    domain::review::valider_note(body.rating).map_err(map_review_error)?;
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if let Some(comment) = comment {
        domain::review::valider_commentaire(comment).map_err(map_review_error)?;
    }
    let reviewee_id = if trade.proposer_id == user.user_id {
        trade.recipient_id
    } else {
        trade.proposer_id
    };
    match infra::review_repo::submit_review(
        &state.pool,
        id,
        user.user_id,
        reviewee_id,
        body.rating,
        comment,
    )
    .await?
    {
        infra::review_repo::SubmitOutcome::AlreadyReviewed => {
            return Err(ApiError::conflict(
                "deja_note",
                "Tu as déjà noté ce troc — une seule évaluation par échange.",
            ))
        }
        infra::review_repo::SubmitOutcome::Created { published, .. } => {
            telemetry::track(
                &state,
                "review_submitted",
                Some(user.user_id),
                json!({"trade_id": id, "rating": body.rating, "has_comment": comment.is_some()}),
            )
            .await;
            if published {
                telemetry::track(
                    &state,
                    "review_published",
                    None,
                    json!({"trade_id": id, "reason": "both_submitted"}),
                )
                .await;
            }
            broadcast_event(
                &state,
                [trade.proposer_id, trade.recipient_id],
                json!({"type": "trade_updated", "trade_id": id}),
            );
        }
    }
    let trade = participant_trade(&state, id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Répondre publiquement (une seule fois) à une évaluation publiée reçue.
#[utoipa::path(
    post,
    path = "/reviews/{id}/reply",
    tag = "trade",
    params(("id" = Uuid, Path, description = "Identifiant de l'évaluation")),
    request_body = ReviewReplyRequest,
    responses(
        (status = 204, description = "Réponse publiée"),
        (status = 400, description = "Réponse invalide, déjà répondue ou non publiée", body = crate::error::ErrorResponse)
    )
)]
pub async fn reply_review(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewReplyRequest>,
) -> Result<StatusCode, ApiError> {
    let reply = body.reply.trim();
    if reply.is_empty() || reply.chars().count() > domain::review::COMMENTAIRE_MAX {
        return Err(ApiError::bad_request(
            "reponse_invalide",
            "Une réponse fait entre 1 et 500 caractères.",
        ));
    }
    if !infra::review_repo::reply_to_review(&state.pool, id, user.user_id, reply).await? {
        return Err(ApiError::bad_request(
            "reponse_impossible",
            "Cette évaluation n'existe pas, n'est pas publiée, ou a déjà sa réponse.",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
