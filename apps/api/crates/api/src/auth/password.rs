//! Hashing Argon2id — paramètres OWASP (m=19 MiB, t=2, p=1), format PHC.
//! Hash et vérification passent par `spawn_blocking` pour ne pas bloquer Tokio.

use std::sync::OnceLock;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

fn hasher() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, None).expect("paramètres argon2 valides");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub async fn hash_password(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        hasher()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| anyhow::anyhow!("hash argon2 : {e}"))
    })
    .await?
}

pub async fn verify_password(hash: String, password: String) -> anyhow::Result<bool> {
    tokio::task::spawn_blocking(move || {
        let parsed =
            PasswordHash::new(&hash).map_err(|e| anyhow::anyhow!("hash PHC invalide : {e}"))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await?
}

/// Vérification factice à durée équivalente, pour ne pas révéler par timing
/// qu'un e-mail est inconnu.
pub async fn verify_dummy(password: String) -> anyhow::Result<()> {
    static DUMMY_HASH: OnceLock<String> = OnceLock::new();
    let hash = match DUMMY_HASH.get() {
        Some(hash) => hash.clone(),
        None => {
            let hash = hash_password("mot-de-passe-factice".to_string()).await?;
            DUMMY_HASH.get_or_init(|| hash).clone()
        }
    };
    let _ = verify_password(hash, password).await?;
    Ok(())
}
