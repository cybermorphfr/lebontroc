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
pub struct TradeResponse {
    pub id: Uuid,
    /// `accepte`, `finalise` ou `annule`.
    pub status: String,
    /// `main_propre` ou `envoi` — choisi à l'acceptation.
    pub delivery_mode: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ConfirmTradeRequest {
    /// Le code à 6 chiffres montré par L'AUTRE partie.
    pub code: String,
}

/// L'écran de rendez-vous : mon code à montrer, l'état des confirmations.
#[derive(Serialize, ToSchema)]
pub struct TradeDetailResponse {
    pub id: Uuid,
    pub proposal_id: Uuid,
    /// `accepte`, `finalise` ou `annule`.
    pub status: String,
    pub delivery_mode: String,
    /// Mon code de confirmation (à montrer en QR / 6 chiffres).
    pub my_code: Option<String>,
    /// J'ai saisi le code de l'autre.
    pub i_confirmed: bool,
    /// L'autre a saisi mon code.
    pub other_confirmed: bool,
    pub finalized_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    /// J'ai demandé l'annulation (en attente de l'accord de l'autre).
    pub cancel_requested_by_me: bool,
    /// L'autre a demandé l'annulation (à moi de confirmer).
    pub cancel_requested_by_other: bool,
    pub accepted_at: DateTime<Utc>,
}

#[derive(Deserialize, ToSchema)]
pub struct AcceptProposalRequest {
    /// `main_propre` ou `envoi`.
    pub delivery_mode: String,
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
    /// Proposition que celle-ci remplace (chaîne de contre-propositions).
    pub counter_of: Option<Uuid>,
    /// Contre-proposition qui a remplacé celle-ci, le cas échéant.
    pub superseded_by: Option<Uuid>,
    /// Le troc créé si la proposition a été acceptée.
    pub trade: Option<TradeResponse>,
}
