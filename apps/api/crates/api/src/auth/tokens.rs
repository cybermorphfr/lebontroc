//! Tokens opaques (refresh, vérification e-mail) : 32 octets aléatoires,
//! transmis en base64url, stockés hashés (SHA-256 hex).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub struct OpaqueToken {
    /// Valeur brute envoyée au client — jamais stockée.
    pub raw: String,
    /// SHA-256 hex, seul stocké en base.
    pub hash: String,
}

pub fn generate() -> OpaqueToken {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let raw = URL_SAFE_NO_PAD.encode(bytes);
    let hash = sha256_hex(&raw);
    OpaqueToken { raw, hash }
}

pub fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
