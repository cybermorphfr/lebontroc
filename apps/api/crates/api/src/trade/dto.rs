//! Types du contrat API pour les propositions de troc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct CreateProposalRequest {
    /// Mes objets (1 à 10), tous disponibles.
    pub offered_item_ids: Vec<Uuid>,
    /// Ses objets (1 à 10), tous au même troqueur.
    pub requested_item_ids: Vec<Uuid>,
    /// Soulte en centimes (0 = pas de soulte).
    #[serde(default)]
    pub cash_cents: i32,
    /// `du_proposant` ou `du_destinataire` (obligatoire si soulte > 0).
    pub cash_direction: Option<String>,
    /// Message d'accompagnement (500 caractères max).
    pub message: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProposalItemResponse {
    pub item_id: Uuid,
    pub title: String,
    /// Valeur figée au moment de la proposition.
    pub value_cents: i32,
    pub photo_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProposalResponse {
    pub id: Uuid,
    /// `envoyee`, `vue`, `acceptee`, `refusee`, `contre_proposee` ou `expiree`.
    pub status: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    /// Le lecteur est-il le proposant (sinon : le destinataire) ?
    pub is_proposer: bool,
    pub cash_cents: i32,
    pub cash_direction: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Ce que le proposant donne.
    pub offered: Vec<ProposalItemResponse>,
    /// Ce que le proposant demande.
    pub requested: Vec<ProposalItemResponse>,
}
