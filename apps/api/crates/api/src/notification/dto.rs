//! Contrats du centre de notifications (F5.3).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct NotificationResponse {
    pub id: Uuid,
    /// Type de la taxonomie (proposition_recue, paiement, favori…).
    pub r#type: String,
    pub title: String,
    pub body: String,
    /// Lien profond vers la page concernée.
    pub link: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationResponse>,
    pub unread_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct UnreadCountResponse {
    pub unread_count: i64,
}

/// Préférences e-mail : uniquement les types désactivables ; true = e-mail
/// envoyé (défaut), false = in-app seulement.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct EmailPrefsResponse {
    pub proposition_recue: bool,
    pub proposition_cloturee: bool,
    pub message_recu: bool,
    pub evaluation: bool,
    pub favori: bool,
}
