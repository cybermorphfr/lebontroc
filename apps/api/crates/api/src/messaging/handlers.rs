//! Handlers de la messagerie : fil, envoi (avec modération), lecture, liste.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use domain::moderation;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::messaging::dto::{ConversationResponse, MessageResponse, SendMessageRequest};
use crate::messaging::ws::broadcast_event;
use crate::telemetry;
use crate::AppState;

/// La proposition, seulement si le lecteur y participe.
async fn participant_proposal(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
) -> Result<infra::trade_repo::Proposal, ApiError> {
    infra::trade_repo::get_proposal(&state.pool, id)
        .await?
        .filter(|p| p.proposer_id == user_id || p.recipient_id == user_id)
        .ok_or_else(|| ApiError::not_found("Cette proposition n'existe pas."))
}

fn message_response(
    state: &AppState,
    message: infra::message_repo::Message,
    pseudo_of: &dyn Fn(Uuid) -> String,
) -> MessageResponse {
    MessageResponse {
        id: message.id,
        proposal_id: message.proposal_id,
        sender_pseudo: pseudo_of(message.sender_id),
        body: message.body,
        photo_url: message
            .photo_key
            .as_deref()
            .map(|k| state.photos.public_url(k)),
        redacted: message.redacted,
        created_at: message.created_at,
        read_at: message.read_at,
    }
}

/// Le fil d'une conversation (participants uniquement).
#[utoipa::path(
    get,
    path = "/proposals/{id}/messages",
    tag = "messaging",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    responses(
        (status = 200, description = "Fil de la conversation", body = [MessageResponse]),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_messages(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MessageResponse>>, ApiError> {
    let proposal = participant_proposal(&state, id, user.user_id).await?;
    let messages = infra::message_repo::list_messages(&state.pool, id).await?;

    telemetry::track(
        &state,
        "conversation_opened",
        Some(user.user_id),
        json!({"proposal_id": id, "messages_count": messages.len()}),
    )
    .await;

    let pseudo_of = |sender: Uuid| {
        if sender == proposal.proposer_id {
            proposal.proposer_pseudo.clone()
        } else {
            proposal.recipient_pseudo.clone()
        }
    };
    Ok(Json(
        messages
            .into_iter()
            .map(|m| message_response(&state, m, &pseudo_of))
            .collect(),
    ))
}

/// Envoyer un message (texte et/ou photo). Avant acceptation, les
/// coordonnées (téléphone, e-mail, IBAN) sont masquées — anti-contournement.
#[utoipa::path(
    post,
    path = "/proposals/{id}/messages",
    tag = "messaging",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    request_body = SendMessageRequest,
    responses(
        (status = 201, description = "Message envoyé", body = MessageResponse),
        (status = 400, description = "Message vide ou conversation fermée", body = crate::error::ErrorResponse),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn send_message(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), ApiError> {
    let proposal = participant_proposal(&state, id, user.user_id).await?;
    if matches!(proposal.status.as_str(), "refusee" | "expiree") {
        return Err(ApiError::bad_request(
            "conversation_fermee",
            "Cette proposition est close — retente ta chance avec une nouvelle proposition.",
        ));
    }

    let texte = body
        .body
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(2000).collect::<String>());

    // Photo optionnelle : un upload présigné à moi, réellement téléversé.
    let photo_key = match body.photo_id {
        Some(photo_id) => {
            let uploads =
                infra::catalog_repo::find_photo_uploads(&state.pool, user.user_id, &[photo_id])
                    .await?;
            let upload = uploads.into_iter().next().ok_or_else(|| {
                ApiError::bad_request(
                    "photo_inconnue",
                    "La photo n'a pas été trouvée. Réessaie l'envoi.",
                )
            })?;
            if !state.photos.object_exists(&upload.s3_key).await {
                return Err(ApiError::bad_request(
                    "photo_manquante",
                    "L'envoi de la photo n'est pas terminé. Réessaie dans un instant.",
                ));
            }
            infra::catalog_repo::delete_photo_upload(&state.pool, photo_id).await?;
            Some(upload.s3_key)
        }
        None => None,
    };

    if texte.is_none() && photo_key.is_none() {
        return Err(ApiError::bad_request(
            "message_vide",
            "Écris un message ou joins une photo.",
        ));
    }

    // Modération : masquage des coordonnées tant que le troc n'est pas accepté.
    let (final_body, redacted) = match texte {
        Some(texte) if proposal.status != "acceptee" => moderation::masquer_coordonnees(&texte),
        Some(texte) => (texte, false),
        None => (String::new(), false),
    };

    let message = infra::message_repo::insert_message(
        &state.pool,
        id,
        user.user_id,
        &final_body,
        photo_key.as_deref(),
        redacted,
    )
    .await?;

    telemetry::track(
        &state,
        "message_sent",
        Some(user.user_id),
        json!({"proposal_id": id, "has_photo": photo_key.is_some(), "redacted": redacted}),
    )
    .await;
    if redacted {
        telemetry::track(
            &state,
            "contact_info_blocked",
            Some(user.user_id),
            json!({"proposal_id": id}),
        )
        .await;
    }

    let pseudo_of = |sender: Uuid| {
        if sender == proposal.proposer_id {
            proposal.proposer_pseudo.clone()
        } else {
            proposal.recipient_pseudo.clone()
        }
    };
    let response = message_response(&state, message, &pseudo_of);
    broadcast_event(
        &state,
        [proposal.proposer_id, proposal.recipient_id],
        json!({
            "type": "message",
            "proposal_id": id,
            "message": {
                "id": response.id,
                "proposal_id": response.proposal_id,
                "sender_pseudo": response.sender_pseudo,
                "body": response.body,
                "photo_url": response.photo_url,
                "redacted": response.redacted,
                "created_at": response.created_at,
                "read_at": response.read_at,
            }
        }),
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Accusé de lecture : marque comme lus les messages reçus.
#[utoipa::path(
    post,
    path = "/proposals/{id}/read",
    tag = "messaging",
    params(("id" = Uuid, Path, description = "Identifiant de la proposition")),
    responses(
        (status = 204, description = "Messages marqués lus"),
        (status = 404, description = "Introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn mark_read(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let proposal = participant_proposal(&state, id, user.user_id).await?;
    let marked = infra::message_repo::mark_read(&state.pool, id, user.user_id).await?;
    if marked > 0 {
        let reader_pseudo = if user.user_id == proposal.proposer_id {
            &proposal.proposer_pseudo
        } else {
            &proposal.recipient_pseudo
        };
        broadcast_event(
            &state,
            [proposal.proposer_id, proposal.recipient_id],
            json!({"type": "read", "proposal_id": id, "reader_pseudo": reader_pseudo}),
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Mes conversations, la plus active d'abord (indicateurs non-lu inclus).
#[utoipa::path(
    get,
    path = "/me/conversations",
    tag = "messaging",
    responses((status = 200, description = "Mes conversations", body = [ConversationResponse]))
)]
pub async fn my_conversations(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<ConversationResponse>>, ApiError> {
    let proposals = infra::trade_repo::list_user_proposals(&state.pool, user.user_id).await?;
    let ids: Vec<Uuid> = proposals.iter().map(|p| p.id).collect();
    let summaries: HashMap<Uuid, infra::message_repo::ConversationSummary> =
        infra::message_repo::conversation_summaries(&state.pool, user.user_id, &ids)
            .await?
            .into_iter()
            .map(|s| (s.proposal_id, s))
            .collect();

    let responses =
        crate::trade::handlers::proposal_responses(&state, proposals, user.user_id).await?;
    let mut conversations: Vec<ConversationResponse> = responses
        .into_iter()
        .map(|proposal| {
            let summary = summaries.get(&proposal.id);
            ConversationResponse {
                last_message: summary.and_then(|s| s.last_body.clone()),
                last_at: summary.and_then(|s| s.last_at),
                last_is_mine: summary
                    .and_then(|s| s.last_sender_id)
                    .map(|sender| sender == user.user_id)
                    .unwrap_or(false),
                unread_count: summary.map(|s| s.unread_count).unwrap_or(0),
                proposal,
            }
        })
        .collect();
    conversations.sort_by_key(|c| std::cmp::Reverse(c.last_at.unwrap_or(c.proposal.created_at)));
    Ok(Json(conversations))
}

/// Relance les destinataires de messages non lus depuis plus de 24 h.
/// Utilisée par la tâche de fond et testable directement.
pub async fn remind_unread(state: &AppState) -> usize {
    let reminders = match infra::message_repo::claim_unread_reminders(&state.pool).await {
        Ok(reminders) => reminders,
        Err(error) => {
            tracing::error!(%error, "relance des non-lus en échec");
            return 0;
        }
    };
    let count = reminders.len();
    for reminder in reminders {
        if let Err(error) = state
            .mailer
            .send_unread_reminder(
                &reminder.recipient_email,
                &reminder.recipient_pseudo,
                &reminder.sender_pseudo,
            )
            .await
        {
            tracing::error!(%error, proposal_id = %reminder.proposal_id, "e-mail de relance en échec");
        }
    }
    count
}
