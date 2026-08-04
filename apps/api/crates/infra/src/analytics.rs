//! Télémétrie produit (§0.4) : événements snake_case dans `analytics_events`,
//! `user_id` toujours hashé, jamais en clair.

use sqlx::PgPool;

/// Insère un événement. Les erreurs sont loguées, jamais propagées :
/// la télémétrie ne doit pas faire échouer une requête métier.
pub async fn track(
    pool: &PgPool,
    name: &str,
    user_id_hash: Option<String>,
    properties: serde_json::Value,
) {
    let result = sqlx::query(
        "INSERT INTO analytics_events (name, user_id_hash, properties) VALUES ($1, $2, $3)",
    )
    .bind(name)
    .bind(user_id_hash)
    .bind(properties)
    .execute(pool)
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, event = name, "échec d'insertion télémétrie");
    }
}
