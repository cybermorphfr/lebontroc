//! Types du contrat API pour l'authentification et le profil.

use chrono::{DateTime, Utc};
use infra::auth_repo::{Session, User};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct SignupRequest {
    #[schema(example = "camille@exemple.fr")]
    pub email: String,
    #[schema(example = "un-bon-mot-de-passe")]
    pub password: String,
    #[schema(example = "camille_troc")]
    pub pseudo: String,
    #[schema(example = "44000")]
    pub postal_code: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TrackRequest {
    /// Nom d'événement d'interface autorisé (ex. `signup_started`).
    #[schema(example = "signup_started")]
    pub name: String,
    /// Objet concerné, le cas échéant (ex. `item_gallery_opened`).
    pub item_id: Option<uuid::Uuid>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    pub pseudo: String,
    pub postal_code: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub pseudo: String,
    pub postal_code: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            pseudo: user.pseudo,
            postal_code: user.postal_code,
            email_verified: user.email_verified_at.is_some(),
            created_at: user.created_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    pub id: Uuid,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    /// `true` pour la session qui fait la requête.
    pub current: bool,
}

impl SessionResponse {
    pub fn from_session(session: Session, current_session_id: Uuid) -> Self {
        Self {
            id: session.id,
            user_agent: session.user_agent,
            created_at: session.created_at,
            last_used_at: session.last_used_at,
            current: session.id == current_session_id,
        }
    }
}
