//! Types du contrat API pour la messagerie.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::trade::dto::ProposalResponse;

#[derive(Deserialize, ToSchema)]
pub struct SendMessageRequest {
    /// Texte du message (2 000 caractères max) — optionnel si photo.
    pub body: Option<String>,
    /// Photo présignée (via /items/photos/presign) — optionnelle.
    pub photo_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct MessageResponse {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub sender_pseudo: String,
    pub body: String,
    pub photo_url: Option<String>,
    /// Des coordonnées ont été masquées (message pédagogique côté client).
    pub redacted: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

/// Une conversation = une proposition + son fil.
#[derive(Serialize, ToSchema)]
pub struct ConversationResponse {
    pub proposal: ProposalResponse,
    pub last_message: Option<String>,
    pub last_at: Option<DateTime<Utc>>,
    pub last_is_mine: bool,
    pub unread_count: i64,
}
