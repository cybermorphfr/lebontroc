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

/// Un événement à exporter vers PostHog (F6.2).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExportableEvent {
    pub id: i64,
    pub name: String,
    pub user_id_hash: Option<String>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub properties: serde_json::Value,
}

/// Lot d'événements jamais exportés (ordre d'insertion).
pub async fn unexported_events(
    pool: &sqlx::PgPool,
    limit: i64,
) -> sqlx::Result<Vec<ExportableEvent>> {
    sqlx::query_as::<_, ExportableEvent>(
        "SELECT id, name, user_id_hash, occurred_at, properties \
         FROM analytics_events WHERE exported_at IS NULL ORDER BY id LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn mark_exported(pool: &sqlx::PgPool, ids: &[i64]) -> sqlx::Result<()> {
    sqlx::query("UPDATE analytics_events SET exported_at = now() WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await?;
    Ok(())
}

/// KPI hebdomadaires du cahier des charges §10, calculés en SQL sur les
/// 7 derniers jours (F6.2) — envoyés par e-mail à l'admin.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WeeklyKpis {
    pub signups: i64,
    pub items_published: i64,
    pub proposals_sent: i64,
    pub trades_created: i64,
    pub trades_finalized: i64,
    pub trades_with_cash: i64,
    pub disputes_opened: i64,
}

pub async fn weekly_kpis(pool: &sqlx::PgPool) -> sqlx::Result<WeeklyKpis> {
    sqlx::query_as::<_, WeeklyKpis>(
        "SELECT \
            (SELECT count(*) FROM users WHERE created_at > now() - interval '7 days') AS signups, \
            (SELECT count(*) FROM items WHERE created_at > now() - interval '7 days') AS items_published, \
            (SELECT count(*) FROM proposals WHERE created_at > now() - interval '7 days') AS proposals_sent, \
            (SELECT count(*) FROM trades WHERE created_at > now() - interval '7 days') AS trades_created, \
            (SELECT count(*) FROM trades WHERE finalized_at > now() - interval '7 days') AS trades_finalized, \
            (SELECT count(*) FROM trades WHERE finalized_at > now() - interval '7 days' AND cash_cents > 0) AS trades_with_cash, \
            (SELECT count(*) FROM disputes WHERE opened_at > now() - interval '7 days') AS disputes_opened",
    )
    .fetch_one(pool)
    .await
}

/// Export batch vers PostHog Cloud EU (F6.2) — silencieux si la clé
/// manque. Retourne le nombre d'événements exportés.
pub async fn export_to_posthog(
    pool: &sqlx::PgPool,
    api_key: &str,
    host: &str,
) -> anyhow::Result<usize> {
    let events = unexported_events(pool, 500).await?;
    if events.is_empty() {
        return Ok(0);
    }
    let batch: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            serde_json::json!({
                "event": e.name,
                "distinct_id": e.user_id_hash.clone().unwrap_or_else(|| "anonyme".to_string()),
                "timestamp": e.occurred_at.to_rfc3339(),
                "properties": e.properties,
            })
        })
        .collect();
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/batch/", host.trim_end_matches('/')))
        .json(&serde_json::json!({"api_key": api_key, "batch": batch}))
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("PostHog a répondu {}", response.status());
    }
    let ids: Vec<i64> = events.iter().map(|e| e.id).collect();
    mark_exported(pool, &ids).await?;
    Ok(ids.len())
}
