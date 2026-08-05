//! Contrats des signalements, blocages et litiges (F5.2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct OpenDisputeRequest {
    /// Motif : non_conforme, abime, manquant, contrefacon, jamais_venu.
    pub reason: String,
    /// Ce qui ne va pas (obligatoire, 1000 caractères max).
    pub description: String,
    /// Clés S3 des pièces déjà uploadées (5 max, bucket privé).
    #[serde(default)]
    pub photo_keys: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct RespondDisputeRequest {
    /// Version de l'autre partie (1000 caractères max).
    pub response: String,
    #[serde(default)]
    pub photo_keys: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ReportRequest {
    /// objet, utilisateur ou message.
    pub target_type: String,
    pub target_id: Uuid,
    /// Motif typé selon la cible (voir référentiel F5.2).
    pub reason: String,
    /// Précisions libres (1000 caractères max, obligatoire pour « autre »).
    pub comment: Option<String>,
}

/// Le dossier de litige d'un troc, vu par une partie.
#[derive(Serialize, ToSchema)]
pub struct DisputeInfo {
    pub id: Uuid,
    pub reason: String,
    pub description: String,
    /// ouvert · en_examen · tranche
    pub status: String,
    /// Le dossier a été ouvert par moi (false : autre partie ou système).
    pub opened_by_me: bool,
    pub response: Option<String>,
    /// Je peux encore verser ma version au dossier.
    pub can_respond: bool,
    pub outcome: Option<String>,
    pub opened_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct BlockedListResponse {
    pub pseudos: Vec<String>,
}

// ————— Administration (token) —————

#[derive(Serialize, ToSchema)]
pub struct AdminDisputeSummary {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub reason: String,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub opened_by_pseudo: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminDisputeDetail {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub trade_status: String,
    pub delivery_mode: String,
    pub cash_cents: i32,
    pub reason: String,
    pub description: String,
    pub status: String,
    pub response: Option<String>,
    pub opened_by_pseudo: Option<String>,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    /// Score de fiabilité interne des deux parties (jamais public).
    pub proposer_score: i32,
    pub recipient_score: i32,
    /// Pièces en URL GET présignées (15 min), uploader identifié.
    pub photos: Vec<AdminDisputePhoto>,
    pub payments: Vec<AdminDisputePayment>,
    pub outcome: Option<String>,
    pub admin_note: Option<String>,
    pub opened_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminDisputePhoto {
    pub uploader_pseudo: String,
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminDisputePayment {
    pub payer_pseudo: String,
    pub amount_cents: i32,
    pub status: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ResolveDisputeRequest {
    /// capture (le troc va au bout, règlements débités) ·
    /// liberation (préautorisations annulées ; troc annulé s'il ne l'était
    /// pas) · rejet (dossier classé, le parcours reprend).
    pub outcome: String,
    /// Partie en tort : alimente le score de fiabilité, les sanctions
    /// automatiques s'appliquent aux seuils (5/10/15).
    pub penalized_pseudo: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ResolveDisputeResponse {
    pub outcome: String,
    /// Score du pénalisé après journalisation (le cas échéant).
    pub penalized_score: Option<i32>,
    /// Sanction automatique déclenchée : aucune, avertissement,
    /// restriction, bannissement.
    pub sanction: String,
}
