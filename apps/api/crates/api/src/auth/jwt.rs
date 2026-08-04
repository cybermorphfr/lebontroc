//! Access token JWT HS256 — 15 minutes, claims minimaux.

use chrono::Utc;
use jsonwebtoken::{decode, encode, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;

pub const ACCESS_TOKEN_MINUTES: i64 = 15;

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    sid: String,
    iat: i64,
    exp: i64,
}

pub fn encode_access(config: &AppConfig, user_id: Uuid, session_id: Uuid) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        sid: session_id.to_string(),
        iat: now,
        exp: now + ACCESS_TOKEN_MINUTES * 60,
    };
    Ok(encode(&Header::default(), &claims, &config.jwt_encoding)?)
}

/// Retourne `(user_id, session_id)` si le token est valide et non expiré.
pub fn decode_access(config: &AppConfig, token: &str) -> Option<(Uuid, Uuid)> {
    let data = decode::<Claims>(token, &config.jwt_decoding, &Validation::default()).ok()?;
    let user_id = Uuid::parse_str(&data.claims.sub).ok()?;
    let session_id = Uuid::parse_str(&data.claims.sid).ok()?;
    Some((user_id, session_id))
}
