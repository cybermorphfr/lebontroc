//! Aides télémétrie : hash des user_id (jamais en clair, §0.4).

use uuid::Uuid;

use crate::auth::tokens::sha256_hex;
use crate::AppState;

pub fn hash_user_id(state: &AppState, user_id: Uuid) -> String {
    sha256_hex(&format!("{}{}", state.config.analytics_salt, user_id))
}

pub async fn track(state: &AppState, name: &str, user_id: Option<Uuid>, properties: serde_json::Value) {
    let hash = user_id.map(|id| hash_user_id(state, id));
    infra::analytics::track(&state.pool, name, hash, properties).await;
}
