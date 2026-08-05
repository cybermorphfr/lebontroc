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
    /// `attente_paiement`, `accepte`, `finalise` ou `annule`.
    pub status: String,
    /// `main_propre` ou `envoi` — choisi à l'acceptation.
    pub delivery_mode: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ConfirmTradeRequest {
    /// Le code à 6 chiffres montré par L'AUTRE partie.
    pub code: String,
}

/// Préautorisation de la soulte (F4.2). Bêta fermée : paiement simulé —
/// n'importe quel numéro plausible réussit, sauf les cartes magiques d'échec.
#[derive(Deserialize, ToSchema)]
pub struct PayTradeRequest {
    /// Numéro de carte (espaces acceptés).
    pub card_number: String,
}

/// L'état d'un paiement du troc (soulte et/ou frais d'envoi).
#[derive(Serialize, ToSchema)]
pub struct PaymentInfo {
    /// `en_attente`, `echoue`, `sequestre`, `capture`, `annule` ou `expire`.
    pub status: String,
    /// Total à préautoriser, en centimes (transport + service + soulte).
    pub amount_cents: i32,
    /// Part transport (mode envoi ; 0 tant que le format n'est pas choisi).
    pub shipping_cents: i32,
    /// Frais de service Lebontroc (mode envoi).
    pub service_cents: i32,
    /// Part soulte.
    pub cash_cents: i32,
    /// Commission plateforme (0 en bêta), prélevée sur le montant capturé.
    pub fees_cents: i32,
    /// Ce que le bénéficiaire reçoit.
    pub net_cents: i32,
    /// Le lecteur est-il celui qui paie ?
    pub i_am_payer: bool,
    /// Date limite de préautorisation — passée, le troc est annulé.
    pub deadline: DateTime<Utc>,
    /// Motif du dernier refus (`carte_refusee`, `provision_insuffisante`…).
    pub failure_reason: Option<String>,
    /// Authentification 3DS à ouvrir, le cas échéant (flux PSP réel).
    pub secure_mode_url: Option<String>,
}

/// L'écran de rendez-vous : mon code à montrer, l'état des confirmations.
#[derive(Serialize, ToSchema)]
pub struct TradeDetailResponse {
    pub id: Uuid,
    pub proposal_id: Uuid,
    /// `attente_paiement`, `accepte`, `finalise` ou `annule`.
    pub status: String,
    pub delivery_mode: String,
    /// Mon code de confirmation (à montrer en QR / 6 chiffres) — masqué tant
    /// que la soulte n'est pas séquestrée.
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
    /// MON paiement s'il y en a un à ma charge, sinon celui de l'autre
    /// (main propre avec soulte vue par le bénéficiaire).
    pub payment: Option<PaymentInfo>,
    /// Mode envoi : l'état du paiement de l'autre partie.
    pub other_payment_status: Option<String>,
    /// Mode envoi : les deux colis (le mien à envoyer, celui que je reçois).
    pub shipments: Vec<ShipmentInfo>,
    /// Les évaluations du troc (F5.1), une fois finalisé.
    pub reviews: TradeReviews,
}

/// Un colis du troc, vu par le lecteur.
#[derive(Serialize, ToSchema)]
pub struct ShipmentInfo {
    pub id: Uuid,
    /// C'est MON colis à envoyer (sinon : celui que je reçois).
    pub i_am_sender: bool,
    /// `preparation`, `etiquette`, `depose`, `transit`, `arrive`, `retire`,
    /// `confirme`, `incident` ou `annule`.
    pub status: String,
    /// Format forfaitaire (`s`, `m`, `l`) choisi par l'expéditeur.
    pub format: Option<String>,
    /// Relais de destination (choisi par le destinataire du colis).
    pub relay_code: Option<String>,
    pub relay_name: Option<String>,
    pub relay_address: Option<String>,
    /// Code à présenter au comptoir — visible par l'expéditeur seulement.
    pub drop_code: Option<String>,
    pub dropped_at: Option<DateTime<Utc>>,
    pub arrived_at: Option<DateTime<Utc>>,
    pub picked_up_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    /// Fin de la fenêtre de signalement (72 h après le retrait).
    pub confirmation_deadline: Option<DateTime<Utc>>,
    pub issue_reason: Option<String>,
}

/// Configuration de « mon envoi » : format de mon colis, relais de réception.
#[derive(Deserialize, ToSchema)]
pub struct ConfigureShippingRequest {
    /// `s` (≤ 1 kg), `m` (≤ 3 kg) ou `l` (≤ 10 kg).
    pub format: String,
    /// Code d'un relais renvoyé par GET /trades/{id}/relays.
    pub relay_code: String,
}

/// Un point relais proposé.
#[derive(Serialize, ToSchema)]
pub struct RelayResponse {
    pub code: String,
    pub name: String,
    pub address: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportIssueRequest {
    /// Ce qui ne va pas (500 caractères max).
    pub reason: String,
}

// ————— Évaluations (F5.1) —————

#[derive(Deserialize, ToSchema)]
pub struct SubmitReviewRequest {
    /// Note de 1 à 5.
    pub rating: i16,
    /// Commentaire public (500 caractères max).
    pub comment: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ReviewReplyRequest {
    /// Réponse publique unique du noté (500 caractères max).
    pub reply: String,
}

#[derive(Serialize, ToSchema)]
pub struct ReviewInfo {
    pub id: Uuid,
    pub rating: i16,
    pub comment: Option<String>,
    /// Publiée ? (sinon : sous embargo anti-représailles)
    pub published: bool,
    pub reply: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Les évaluations d'un troc, vues par le lecteur : la sienne (toujours
/// visible pour lui), celle de l'autre seulement une fois publiée.
#[derive(Serialize, ToSchema)]
pub struct TradeReviews {
    pub mine: Option<ReviewInfo>,
    pub received: Option<ReviewInfo>,
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
