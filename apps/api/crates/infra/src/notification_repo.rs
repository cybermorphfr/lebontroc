//! Requêtes SQL du centre de notifications (F5.3).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub r#type: String,
    pub payload: serde_json::Value,
    pub link: String,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

/// Insère et retourne le nouveau compteur de non-lus (pour le badge WS).
pub async fn insert_notification(
    pool: &PgPool,
    user_id: Uuid,
    kind: &str,
    payload: &serde_json::Value,
    link: &str,
) -> sqlx::Result<i64> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO notifications (user_id, type, payload, link) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(kind)
        .bind(payload)
        .bind(link)
        .execute(&mut *tx)
        .await?;
    let (unread,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(unread)
}

/// Les 30 dernières notifications, éventuellement avant un curseur.
pub async fn list_notifications(
    pool: &PgPool,
    user_id: Uuid,
    before: Option<DateTime<Utc>>,
) -> sqlx::Result<Vec<Notification>> {
    sqlx::query_as::<_, Notification>(
        "SELECT id, type, payload, link, created_at, read_at FROM notifications \
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2) \
         ORDER BY created_at DESC LIMIT 30",
    )
    .bind(user_id)
    .bind(before)
    .fetch_all(pool)
    .await
}

pub async fn unread_count(pool: &PgPool, user_id: Uuid) -> sqlx::Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn mark_read(pool: &PgPool, user_id: Uuid, id: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE notifications SET read_at = now() \
         WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> sqlx::Result<u64> {
    let updated = sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected())
}

/// Purge de rétention (90 j) — appelée par la maintenance horaire.
pub async fn purge_old_notifications(pool: &PgPool, days: i64) -> sqlx::Result<u64> {
    let deleted = sqlx::query(
        "DELETE FROM notifications WHERE created_at < now() - make_interval(days => $1::int)",
    )
    .bind(days)
    .execute(pool)
    .await?;
    Ok(deleted.rows_affected())
}

/// Préférences e-mail brutes ({} par défaut).
pub async fn email_prefs(pool: &PgPool, user_id: Uuid) -> sqlx::Result<serde_json::Value> {
    let (prefs,): (serde_json::Value,) =
        sqlx::query_as("SELECT email_prefs FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(prefs)
}

pub async fn set_email_prefs(
    pool: &PgPool,
    user_id: Uuid,
    prefs: &serde_json::Value,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET email_prefs = $2 WHERE id = $1")
        .bind(user_id)
        .bind(prefs)
        .execute(pool)
        .await?;
    Ok(())
}

/// Fan d'un objet du troc : destinataire de « ton favori vient d'être
/// réservé / est de nouveau disponible ».
#[derive(Debug, Clone, FromRow)]
pub struct Fan {
    pub user_id: Uuid,
    pub item_id: Uuid,
    pub title: String,
    pub email: String,
    pub pseudo: String,
}

/// Les utilisateurs (hors parties) ayant en favori un objet du troc.
pub async fn fans_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Vec<Fan>> {
    sqlx::query_as::<_, Fan>(
        "SELECT f.user_id, f.item_id, i.title, u.email::text AS email, \
                u.pseudo::text AS pseudo \
         FROM trades t \
         JOIN proposal_items pi ON pi.proposal_id = t.proposal_id \
         JOIN favorites f ON f.item_id = pi.item_id \
         JOIN items i ON i.id = f.item_id \
         JOIN users u ON u.id = f.user_id \
         WHERE t.id = $1 AND f.user_id NOT IN (t.proposer_id, t.recipient_id)",
    )
    .bind(trade_id)
    .fetch_all(pool)
    .await
}
