//! Signalements, blocages, dossiers de litige et sanctions (F5.2).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::dispute::dto::{
    AdminDisputeDetail, AdminDisputePayment, AdminDisputePhoto, AdminDisputeSummary,
    BlockedListResponse, OpenDisputeRequest, ReportRequest, ResolveDisputeRequest,
    ResolveDisputeResponse, RespondDisputeRequest,
};
use crate::error::ApiError;
use crate::extract::{AdminActor, CurrentUser};
use crate::telemetry;
use crate::trade::dto::TradeDetailResponse;
use crate::trade::handlers::{
    capture_payment, full_trade_response, participant_trade, release_payments_if_any,
};
use crate::AppState;
use domain::dispute as regles;
use domain::payment as paiement;

fn map_dispute_error(error: regles::DisputeError) -> ApiError {
    match error {
        regles::DisputeError::MotifInconnu => {
            ApiError::bad_request("motif_inconnu", "Ce motif de litige n'existe pas.")
        }
        regles::DisputeError::DescriptionInvalide => ApiError::bad_request(
            "description_invalide",
            "Décris le problème (1000 caractères max).",
        ),
        regles::DisputeError::TropDePieces => {
            ApiError::bad_request("trop_de_pieces", "5 photos maximum par dossier.")
        }
        regles::DisputeError::HorsFenetre => ApiError::bad_request(
            "hors_fenetre",
            "Ce troc ne peut pas (ou plus) faire l'objet d'un dossier : \
             la fenêtre de signalement est close.",
        ),
    }
}

async fn attach_verified_photos(
    state: &AppState,
    dispute_id: Uuid,
    uploader_id: Uuid,
    keys: &[String],
) -> Result<(), ApiError> {
    let mut valid = Vec::new();
    for key in keys.iter().take(regles::PIECES_MAX) {
        if key.starts_with("litiges/") && state.dispute_photos.object_exists(key).await {
            valid.push(key.clone());
        }
    }
    infra::dispute_repo::attach_photos(&state.pool, dispute_id, uploader_id, &valid).await?;
    Ok(())
}

/// Notifie l'autre partie et l'admin à l'ouverture d'un dossier.
async fn notify_dispute_opened(state: &AppState, trade_id: Uuid, opener_id: Uuid, reason: &str) {
    let summary = infra::dispute_repo::trade_summary(&state.pool, trade_id).await;
    let Ok(Some(trade)) = summary else { return };
    let other_id = if trade.proposer_id == opener_id {
        trade.recipient_id
    } else {
        trade.proposer_id
    };
    let opener = infra::auth_repo::find_user_by_id(&state.pool, opener_id).await;
    let other = infra::auth_repo::find_user_by_id(&state.pool, other_id).await;
    if let (Ok(Some(opener)), Ok(Some(other))) = (opener, other) {
        crate::notification::handlers::notify(
            state,
            other_id,
            "litige",
            "Un dossier est ouvert sur ton troc".to_string(),
            format!(
                "{} a signalé un problème ({reason}). Tu as 72 h pour donner ta version.",
                opener.pseudo
            ),
            "/trocs".to_string(),
        )
        .await;
        if let Err(error) = state
            .mailer
            .send_dispute_opened(&other.email, &other.pseudo, &opener.pseudo, reason)
            .await
        {
            tracing::error!(%error, "e-mail dossier ouvert non parti");
        }
        if let Err(error) = state
            .mailer
            .send_admin_dispute(
                &state.config.admin_email,
                &trade_id.to_string(),
                &format!("dossier {reason} ouvert par {}", opener.pseudo),
            )
            .await
        {
            tracing::error!(%error, "alerte admin dossier non partie");
        }
        crate::admin::handlers::notify_admins(
            state,
            "⚖️ Nouveau dossier de litige".to_string(),
            format!("{} a ouvert un dossier ({reason}).", opener.pseudo),
            "/admin/litiges".to_string(),
        )
        .await;
    }
}

/// Presign des pièces d'un dossier (bucket PRIVÉ) — 5 fichiers, 5 Mo max.
#[utoipa::path(
    post,
    path = "/trades/{id}/dispute/presign",
    tag = "dispute",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = crate::catalog::dto::PresignRequest,
    responses(
        (status = 200, description = "URL de PUT présignées", body = [crate::catalog::dto::PresignedPhoto]),
        (status = 400, description = "Fichier refusé", body = crate::error::ErrorResponse)
    )
)]
pub async fn presign_dispute_photos(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<crate::catalog::dto::PresignRequest>,
) -> Result<Json<Vec<crate::catalog::dto::PresignedPhoto>>, ApiError> {
    participant_trade(&state, id, user.user_id).await?;
    if body.files.is_empty() || body.files.len() > regles::PIECES_MAX {
        return Err(ApiError::bad_request(
            "trop_de_pieces",
            "5 photos maximum par dossier.",
        ));
    }
    let mut result = Vec::with_capacity(body.files.len());
    for file in &body.files {
        let extension = match file.content_type.as_str() {
            "image/webp" => "webp",
            "image/jpeg" => "jpg",
            _ => {
                return Err(ApiError::bad_request(
                    "type_refuse",
                    "On n'a pas réussi à lire cette image. Essaie un autre format (JPG, WebP).",
                ))
            }
        };
        if file.size <= 0 || file.size > 5 * 1024 * 1024 {
            return Err(ApiError::bad_request(
                "fichier_trop_lourd",
                "Cette photo dépasse 5 Mo après compression. Réessaie avec une autre.",
            ));
        }
        let photo_id = Uuid::new_v4();
        let key = format!("litiges/{id}/{photo_id}.{extension}");
        let upload_url = state
            .dispute_photos
            .presign_put(&key, &file.content_type, i64::from(file.size))
            .await
            .map_err(ApiError::internal)?;
        result.push(crate::catalog::dto::PresignedPhoto {
            photo_id,
            upload_url,
        });
    }
    Ok(Json(result))
}

/// Ouvrir le dossier de litige du troc (un seul par troc). Gèle le troc
/// s'il est en cours ; suspend la capture s'il est finalisé (fenêtre 48 h
/// main propre). Gherkin F5.2 : la soulte reste séquestrée.
#[utoipa::path(
    post,
    path = "/trades/{id}/dispute",
    tag = "dispute",
    params(("id" = Uuid, Path, description = "Identifiant du troc")),
    request_body = OpenDisputeRequest,
    responses(
        (status = 200, description = "Dossier ouvert", body = TradeDetailResponse),
        (status = 400, description = "Motif ou fenêtre invalide", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse),
        (status = 409, description = "Un dossier existe déjà", body = crate::error::ErrorResponse)
    )
)]
pub async fn open_dispute(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<OpenDisputeRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let trade = participant_trade(&state, id, user.user_id).await?;
    // Un seul dossier par troc — le conflit prime sur la fenêtre (le gel
    // du dossier existant fermerait sinon la fenêtre et masquerait le 409).
    if infra::dispute_repo::dispute_for_trade(&state.pool, id)
        .await?
        .is_some()
    {
        return Err(ApiError::conflict(
            "dossier_existant",
            "Un dossier est déjà ouvert sur ce troc.",
        ));
    }
    regles::valider_ouverture(&body.reason, &body.description, body.photo_keys.len())
        .map_err(map_dispute_error)?;
    // Mode envoi : le dossier s'adosse au colis entrant encore contestable.
    let incoming_shipment = if trade.delivery_mode == "envoi" {
        infra::shipping_repo::shipments_for_trade(&state.pool, id)
            .await?
            .into_iter()
            .find(|s| {
                s.recipient_id == user.user_id && matches!(s.status.as_str(), "arrive" | "retire")
            })
    } else {
        None
    };
    let incoming_receivable = incoming_shipment.is_some();
    regles::fenetre_ouverture(
        &body.reason,
        &trade.delivery_mode,
        &trade.status,
        trade.created_at,
        trade.finalized_at,
        incoming_receivable,
        chrono::Utc::now(),
    )
    .map_err(map_dispute_error)?;

    let dispute = infra::dispute_repo::open_dispute(
        &state.pool,
        id,
        Some(user.user_id),
        &body.reason,
        body.description.trim(),
    )
    .await?
    .ok_or_else(|| {
        ApiError::conflict(
            "dossier_existant",
            "Un dossier est déjà ouvert sur ce troc.",
        )
    })?;

    // Troc en cours → gel (capture et auto-confirmation suspendues). En
    // mode envoi, le colis contesté passe aussi en incident (flux F4.3).
    if trade.status == "accepte" {
        match &incoming_shipment {
            Some(shipment) => {
                infra::shipping_repo::report_issue(
                    &state.pool,
                    shipment.id,
                    user.user_id,
                    &body.reason,
                )
                .await?;
            }
            None => {
                infra::shipping_repo::freeze_trade(&state.pool, id).await?;
            }
        }
    }
    attach_verified_photos(&state, dispute.id, user.user_id, &body.photo_keys).await?;
    telemetry::track(
        &state,
        "dispute_opened",
        Some(user.user_id),
        json!({"trade_id": id, "reason": body.reason, "trade_status": trade.status}),
    )
    .await;
    notify_dispute_opened(&state, id, user.user_id, &body.reason).await;
    crate::messaging::ws::broadcast_event(
        &state,
        [trade.proposer_id, trade.recipient_id],
        json!({"type": "trade_updated", "trade_id": id}),
    );
    let trade = participant_trade(&state, id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Verser sa version au dossier (contradictoire, une seule fois, 72 h).
#[utoipa::path(
    post,
    path = "/disputes/{id}/respond",
    tag = "dispute",
    params(("id" = Uuid, Path, description = "Identifiant du dossier")),
    request_body = RespondDisputeRequest,
    responses(
        (status = 200, description = "Version enregistrée", body = TradeDetailResponse),
        (status = 400, description = "Dossier clos ou déjà répondu", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn respond_dispute(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<RespondDisputeRequest>,
) -> Result<Json<TradeDetailResponse>, ApiError> {
    let dispute = infra::dispute_repo::dispute_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce dossier n'existe pas."))?;
    let trade = participant_trade(&state, dispute.trade_id, user.user_id).await?;
    if dispute.opened_by == Some(user.user_id) {
        return Err(ApiError::bad_request(
            "deja_ta_version",
            "Le dossier porte déjà ta version — c'est à l'autre partie de répondre.",
        ));
    }
    let response = body.response.trim();
    if response.is_empty() || response.chars().count() > regles::DESCRIPTION_MAX {
        return Err(ApiError::bad_request(
            "description_invalide",
            "Décris ta version (1000 caractères max).",
        ));
    }
    if body.photo_keys.len() > regles::PIECES_MAX {
        return Err(ApiError::bad_request(
            "trop_de_pieces",
            "5 photos maximum par dossier.",
        ));
    }
    if !infra::dispute_repo::respond_to_dispute(&state.pool, id, response).await? {
        return Err(ApiError::bad_request(
            "dossier_clos",
            "Ce dossier n'attend plus de réponse.",
        ));
    }
    attach_verified_photos(&state, id, user.user_id, &body.photo_keys).await?;
    crate::messaging::ws::broadcast_event(
        &state,
        [trade.proposer_id, trade.recipient_id],
        json!({"type": "trade_updated", "trade_id": trade.id}),
    );
    let trade = participant_trade(&state, dispute.trade_id, user.user_id).await?;
    Ok(Json(
        full_trade_response(&state, trade, user.user_id).await?,
    ))
}

/// Signaler un objet, un utilisateur ou un message (motifs typés).
/// Enregistré + alerte admin ; le traitement outillé arrive en F6.1.
#[utoipa::path(
    post,
    path = "/reports",
    tag = "dispute",
    request_body = ReportRequest,
    responses(
        (status = 201, description = "Signalement enregistré"),
        (status = 400, description = "Cible ou motif invalide", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_report(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<ReportRequest>,
) -> Result<StatusCode, ApiError> {
    let motifs = regles::motifs_signalement(&body.target_type).ok_or_else(|| {
        ApiError::bad_request(
            "cible_invalide",
            "On signale un objet, un utilisateur ou un message.",
        )
    })?;
    if !motifs.contains(&body.reason.as_str()) {
        return Err(ApiError::bad_request(
            "motif_inconnu",
            "Ce motif ne correspond pas à ce type de signalement.",
        ));
    }
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if body.reason == "autre" && comment.is_none() {
        return Err(ApiError::bad_request(
            "precision_requise",
            "Pour « autre », dis-nous en deux mots ce qui ne va pas.",
        ));
    }
    if comment.is_some_and(|c| c.chars().count() > regles::DESCRIPTION_MAX) {
        return Err(ApiError::bad_request(
            "description_invalide",
            "1000 caractères max.",
        ));
    }
    infra::dispute_repo::create_report(
        &state.pool,
        user.user_id,
        &body.target_type,
        body.target_id,
        &body.reason,
        comment,
    )
    .await?;
    telemetry::track(
        &state,
        "report_submitted",
        Some(user.user_id),
        json!({"target_type": body.target_type, "reason": body.reason}),
    )
    .await;
    if let Err(error) = state
        .mailer
        .send_admin_dispute(
            &state.config.admin_email,
            &body.target_id.to_string(),
            &format!("signalement {} ({})", body.target_type, body.reason),
        )
        .await
    {
        tracing::error!(%error, "alerte admin signalement non partie");
    }
    crate::admin::handlers::notify_admins(
        &state,
        "🚩 Nouveau signalement".to_string(),
        format!("{} · {}", body.target_type, body.reason),
        "/admin/signalements".to_string(),
    )
    .await;
    Ok(StatusCode::CREATED)
}

/// Bloquer un troqueur : plus de propositions ni de messages dans les deux
/// sens, profils masqués du feed et de la recherche. Les trocs en cours
/// continuent. L'autre n'est pas prévenu.
#[utoipa::path(
    post,
    path = "/users/{pseudo}/block",
    tag = "dispute",
    params(("pseudo" = String, Path, description = "Pseudo à bloquer")),
    responses(
        (status = 204, description = "Bloqué"),
        (status = 400, description = "Auto-blocage", body = crate::error::ErrorResponse),
        (status = 404, description = "Inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn block_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(pseudo): Path<String>,
) -> Result<StatusCode, ApiError> {
    let target = infra::auth_repo::find_user_by_pseudo(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
    if target.id == user.user_id {
        return Err(ApiError::bad_request(
            "auto_blocage",
            "Se bloquer soi-même, c'est de la méditation — pas du troc.",
        ));
    }
    if infra::dispute_repo::block_user(&state.pool, user.user_id, target.id).await? {
        telemetry::track(&state, "user_blocked", Some(user.user_id), json!({})).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/users/{pseudo}/block",
    tag = "dispute",
    params(("pseudo" = String, Path, description = "Pseudo à débloquer")),
    responses(
        (status = 204, description = "Débloqué"),
        (status = 404, description = "Inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn unblock_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(pseudo): Path<String>,
) -> Result<StatusCode, ApiError> {
    let target = infra::auth_repo::find_user_by_pseudo(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
    infra::dispute_repo::unblock_user(&state.pool, user.user_id, target.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Mes blocages (écran réglages).
#[utoipa::path(
    get,
    path = "/me/blocks",
    tag = "dispute",
    responses((status = 200, description = "Pseudos bloqués", body = BlockedListResponse))
)]
pub async fn my_blocks(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<BlockedListResponse>, ApiError> {
    let pseudos = infra::dispute_repo::blocked_pseudos(&state.pool, user.user_id).await?;
    Ok(Json(BlockedListResponse { pseudos }))
}

// ————— Score et sanctions automatiques —————

/// Journalise un événement négatif puis applique la sanction du score
/// atteint (choix Brian : seuils automatiques 5/10/15, e-mail admin à
/// chaque déclenchement, levée possible via l'admin).
pub async fn apply_negative_event(
    state: &AppState,
    trade_id: Uuid,
    user_id: Uuid,
    event_type: &str,
    details: &str,
) -> regles::Sanction {
    if let Err(error) = infra::shipping_repo::record_dispute_event(
        &state.pool,
        trade_id,
        event_type,
        Some(user_id),
        Some(details),
    )
    .await
    {
        tracing::error!(%error, "journalisation dispute_event en échec");
        return regles::Sanction::Aucune;
    }
    apply_score_sanctions(state, user_id).await
}

/// (Ré)évalue le score d'un utilisateur et applique la sanction due.
pub async fn apply_score_sanctions(state: &AppState, user_id: Uuid) -> regles::Sanction {
    let score = match infra::dispute_repo::reliability_score(&state.pool, user_id).await {
        Ok(score) => score,
        Err(error) => {
            tracing::error!(%error, "calcul du score de fiabilité en échec");
            return regles::Sanction::Aucune;
        }
    };
    let sanction = regles::sanction_pour_score(score);
    let Ok(Some(target)) = infra::auth_repo::find_user_by_id(&state.pool, user_id).await else {
        return sanction;
    };
    let Ok(current) = infra::dispute_repo::sanction_state(&state.pool, user_id).await else {
        return sanction;
    };
    let (sanction_text, admin_text) = match sanction {
        regles::Sanction::Aucune => return sanction,
        regles::Sanction::Avertissement => (
            "Plusieurs incidents ont été constatés sur tes trocs. \
             Prends soin des échanges en cours : au prochain incident avéré, \
             ton compte pourra être restreint."
                .to_string(),
            None,
        ),
        regles::Sanction::Restriction => {
            if current.banned_at.is_some()
                || current
                    .restricted_until
                    .is_some_and(|until| until > chrono::Utc::now())
            {
                return sanction;
            }
            if let Err(error) =
                infra::dispute_repo::restrict_user(&state.pool, user_id, regles::RESTRICTION_JOURS)
                    .await
            {
                tracing::error!(%error, "restriction en échec");
                return sanction;
            }
            (
                "Suite à plusieurs incidents avérés, ton compte est restreint \
                 pendant 30 jours : tes trocs en cours continuent, mais tu ne \
                 peux plus faire de nouvelles propositions."
                    .to_string(),
                Some(format!("restriction 30 j automatique de {}", target.pseudo)),
            )
        }
        regles::Sanction::Bannissement => {
            if current.banned_at.is_some() {
                return sanction;
            }
            if let Err(error) = infra::dispute_repo::ban_user(&state.pool, user_id).await {
                tracing::error!(%error, "bannissement en échec");
                return sanction;
            }
            (
                "Suite à des incidents graves et répétés, ton compte Lebontroc \
                 est suspendu. Tes trocs en cours sont gelés."
                    .to_string(),
                Some(format!("BANNISSEMENT automatique de {}", target.pseudo)),
            )
        }
    };
    crate::notification::handlers::notify(
        state,
        user_id,
        "litige",
        "Important — au sujet de ton compte".to_string(),
        sanction_text.clone(),
        "/trocs".to_string(),
    )
    .await;
    if let Err(error) = state
        .mailer
        .send_sanction(&target.email, &target.pseudo, &sanction_text)
        .await
    {
        tracing::error!(%error, "e-mail de sanction non parti");
    }
    if let Some(admin_text) = admin_text {
        if let Err(error) = state
            .mailer
            .send_admin_dispute(&state.config.admin_email, "score", &admin_text)
            .await
        {
            tracing::error!(%error, "alerte admin sanction non partie");
        }
        crate::admin::handlers::notify_admins(
            state,
            "🚨 Sanction automatique".to_string(),
            admin_text.clone(),
            "/admin".to_string(),
        )
        .await;
    }
    sanction
}

// ————— Administration (X-Admin-Token) —————

#[derive(Deserialize)]
pub struct DisputeListQuery {
    pub status: Option<String>,
}

/// File des dossiers (admin).
#[utoipa::path(
    get,
    path = "/admin/disputes",
    tag = "admin",
    params(("status" = Option<String>, Query, description = "Filtre de statut")),
    responses((status = 200, description = "Dossiers", body = [AdminDisputeSummary]))
)]
pub async fn admin_list_disputes(
    State(state): State<AppState>,
    _admin: AdminActor,
    Query(query): Query<DisputeListQuery>,
) -> Result<Json<Vec<AdminDisputeSummary>>, ApiError> {
    let disputes = infra::dispute_repo::list_disputes(&state.pool, query.status.as_deref()).await?;
    let mut result = Vec::with_capacity(disputes.len());
    for dispute in disputes {
        let opened_by_pseudo = match dispute.opened_by {
            Some(id) => infra::auth_repo::find_user_by_id(&state.pool, id)
                .await?
                .map(|u| u.pseudo),
            None => None,
        };
        result.push(AdminDisputeSummary {
            id: dispute.id,
            trade_id: dispute.trade_id,
            reason: dispute.reason,
            status: dispute.status,
            opened_at: dispute.opened_at,
            opened_by_pseudo,
        });
    }
    Ok(Json(result))
}

/// Dossier complet : pièces présignées, paiements, scores (admin).
#[utoipa::path(
    get,
    path = "/admin/disputes/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Identifiant du dossier")),
    responses(
        (status = 200, description = "Dossier", body = AdminDisputeDetail),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_get_dispute(
    State(state): State<AppState>,
    _admin: AdminActor,
    Path(id): Path<Uuid>,
) -> Result<Json<AdminDisputeDetail>, ApiError> {
    let dispute = infra::dispute_repo::dispute_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce dossier n'existe pas."))?;
    let trade = infra::dispute_repo::trade_summary(&state.pool, dispute.trade_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Troc introuvable."))?;
    let proposer = infra::auth_repo::find_user_by_id(&state.pool, trade.proposer_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Partie introuvable."))?;
    let recipient = infra::auth_repo::find_user_by_id(&state.pool, trade.recipient_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Partie introuvable."))?;
    let mut photos = Vec::new();
    for photo in infra::dispute_repo::photos_for_dispute(&state.pool, id).await? {
        let url = state
            .dispute_photos
            .presign_get(&photo.s3_key)
            .await
            .map_err(ApiError::internal)?;
        let uploader_pseudo = if photo.uploader_id == proposer.id {
            proposer.pseudo.clone()
        } else {
            recipient.pseudo.clone()
        };
        photos.push(AdminDisputePhoto {
            uploader_pseudo,
            url,
        });
    }
    let mut payments = Vec::new();
    for payment in infra::payment_repo::payments_for_trade(&state.pool, dispute.trade_id).await? {
        let payer_pseudo = if payment.payer_id == proposer.id {
            proposer.pseudo.clone()
        } else {
            recipient.pseudo.clone()
        };
        payments.push(AdminDisputePayment {
            payer_pseudo,
            amount_cents: payment.amount_cents,
            status: payment.status,
        });
    }
    let opened_by_pseudo = dispute.opened_by.map(|id| {
        if id == proposer.id {
            proposer.pseudo.clone()
        } else {
            recipient.pseudo.clone()
        }
    });
    Ok(Json(AdminDisputeDetail {
        id: dispute.id,
        trade_id: dispute.trade_id,
        trade_status: trade.status,
        delivery_mode: trade.delivery_mode,
        cash_cents: trade.cash_cents,
        reason: dispute.reason,
        description: dispute.description,
        status: dispute.status,
        response: dispute.response,
        opened_by_pseudo,
        proposer_score: infra::dispute_repo::reliability_score(&state.pool, proposer.id).await?,
        recipient_score: infra::dispute_repo::reliability_score(&state.pool, recipient.id).await?,
        proposer_pseudo: proposer.pseudo,
        recipient_pseudo: recipient.pseudo,
        photos,
        payments,
        outcome: dispute.outcome,
        admin_note: dispute.admin_note,
        opened_at: dispute.opened_at,
    }))
}

/// Trancher un dossier (admin) : l'issue s'applique via les traits PSP —
/// capture, libération ou rejet — et la partie en tort alimente le score.
#[utoipa::path(
    post,
    path = "/admin/disputes/{id}/resolve",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Identifiant du dossier")),
    request_body = ResolveDisputeRequest,
    responses(
        (status = 200, description = "Dossier tranché", body = ResolveDisputeResponse),
        (status = 400, description = "Issue invalide", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse),
        (status = 409, description = "Déjà tranché", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_resolve_dispute(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(id): Path<Uuid>,
    Json(body): Json<ResolveDisputeRequest>,
) -> Result<Json<ResolveDisputeResponse>, ApiError> {
    if !matches!(body.outcome.as_str(), "capture" | "liberation" | "rejet") {
        return Err(ApiError::bad_request(
            "issue_invalide",
            "L'issue est capture, liberation ou rejet.",
        ));
    }
    let dispute = infra::dispute_repo::dispute_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce dossier n'existe pas."))?;
    let trade = infra::dispute_repo::trade_summary(&state.pool, dispute.trade_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Troc introuvable."))?;
    let penalized = match body.penalized_pseudo.as_deref() {
        Some(pseudo) => {
            let user = infra::auth_repo::find_user_by_pseudo(&state.pool, pseudo)
                .await?
                .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
            if user.id != trade.proposer_id && user.id != trade.recipient_id {
                return Err(ApiError::bad_request(
                    "hors_troc",
                    "La partie pénalisée doit être une partie du troc.",
                ));
            }
            Some(user)
        }
        None => None,
    };
    if !infra::dispute_repo::resolve_dispute(
        &state.pool,
        id,
        &body.outcome,
        None,
        penalized.as_ref().map(|u| u.id),
        body.note.as_deref(),
    )
    .await?
    {
        return Err(ApiError::conflict(
            "deja_tranche",
            "Ce dossier est déjà tranché.",
        ));
    }

    // L'issue s'applique aux paiements et au troc — via les traits, jamais
    // en SQL : le jour du vrai PSP, c'est ce code qui capture.
    match body.outcome.as_str() {
        "capture" => {
            infra::dispute_repo::resolve_finalize_trade(&state.pool, trade.id).await?;
            for payment in infra::payment_repo::payments_for_trade(&state.pool, trade.id).await? {
                if paiement::peut_capturer(&payment.status) {
                    capture_payment(&state, &payment).await;
                }
            }
        }
        "liberation" => {
            infra::dispute_repo::resolve_cancel_trade(&state.pool, trade.id).await?;
            release_payments_if_any(&state, trade.id).await;
        }
        _ => {
            // Rejet : le parcours normal reprend (dégel si gelé ; la capture
            // différée reprend seule, le dossier n'étant plus ouvert).
            infra::dispute_repo::unfreeze_trade(&state.pool, trade.id).await?;
        }
    }

    // La partie en tort alimente le score ; les seuils s'appliquent seuls.
    let mut penalized_score = None;
    let mut sanction = regles::Sanction::Aucune;
    if let Some(target) = &penalized {
        // non_depot est déjà journalisé par F4.3 — pas de double peine.
        let event_type = match (body.outcome.as_str(), dispute.reason.as_str()) {
            (_, "non_depot") => None,
            ("rejet", _) if dispute.opened_by == Some(target.id) => Some("litige_abusif"),
            (_, "contrefacon") => Some("contrefacon_averee"),
            (_, "jamais_venu") => Some("no_show_confirme"),
            _ => Some("litige_perdu"),
        };
        if let Some(event_type) = event_type {
            sanction = apply_negative_event(
                &state,
                trade.id,
                target.id,
                event_type,
                &format!("dossier {id} tranché : {}", body.outcome),
            )
            .await;
        } else {
            sanction = apply_score_sanctions(&state, target.id).await;
        }
        penalized_score = infra::dispute_repo::reliability_score(&state.pool, target.id)
            .await
            .ok();
    }

    let days = (chrono::Utc::now() - dispute.opened_at).num_days();
    telemetry::track(
        &state,
        "dispute_resolved",
        None,
        json!({"dispute_id": id, "outcome": body.outcome, "days_to_resolve": days}),
    )
    .await;
    let outcome_text = match body.outcome.as_str() {
        "capture" => "le troc est validé, les règlements bloqués sont débités.",
        "liberation" => "les règlements bloqués sont annulés — rien n'est débité.",
        _ => "le dossier est classé sans suite, le troc reprend son cours normal.",
    };
    for user_id in [trade.proposer_id, trade.recipient_id] {
        crate::notification::handlers::notify(
            &state,
            user_id,
            "litige",
            "Ton dossier de litige est tranché".to_string(),
            format!("L'examen est terminé : {outcome_text}"),
            "/trocs".to_string(),
        )
        .await;
        if let Ok(Some(u)) = infra::auth_repo::find_user_by_id(&state.pool, user_id).await {
            if let Err(error) = state
                .mailer
                .send_dispute_resolved(&u.email, &u.pseudo, outcome_text)
                .await
            {
                tracing::error!(%error, "e-mail résolution non parti");
            }
        }
    }
    crate::messaging::ws::broadcast_event(
        &state,
        [trade.proposer_id, trade.recipient_id],
        json!({"type": "trade_updated", "trade_id": trade.id}),
    );
    crate::admin::handlers::record_admin_action(
        &state,
        admin.user_id,
        "dispute_resolved",
        "trade",
        &trade.id.to_string(),
        body.note.as_deref(),
    )
    .await;
    Ok(Json(ResolveDisputeResponse {
        outcome: body.outcome,
        penalized_score,
        sanction: match sanction {
            regles::Sanction::Aucune => "aucune",
            regles::Sanction::Avertissement => "avertissement",
            regles::Sanction::Restriction => "restriction",
            regles::Sanction::Bannissement => "bannissement",
        }
        .to_string(),
    }))
}

/// Lever les sanctions d'un compte (filet des sanctions automatiques).
#[utoipa::path(
    post,
    path = "/admin/users/{pseudo}/lift-sanctions",
    tag = "admin",
    params(("pseudo" = String, Path, description = "Pseudo")),
    responses(
        (status = 204, description = "Sanctions levées"),
        (status = 404, description = "Inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_lift_sanctions(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(pseudo): Path<String>,
) -> Result<StatusCode, ApiError> {
    let cible = infra::admin_repo::find_role_target(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
    // Sanctionner ou dé-sanctionner engage la plateforme : super-admin
    // seulement, et jamais sur le compte maître.
    domain::admin::peut_sanctionner(&admin.role, cible.is_master)
        .map_err(crate::admin::handlers::map_admin_error)?;
    let target = infra::auth_repo::find_user_by_pseudo(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
    infra::dispute_repo::lift_sanctions(&state.pool, target.id).await?;
    crate::admin::handlers::record_admin_action(
        &state,
        admin.user_id,
        "sanctions_lifted",
        "utilisateur",
        &target.id.to_string(),
        Some(&pseudo),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
