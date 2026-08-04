//! Requêtes SQL de la messagerie (F3.2).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub photo_key: Option<String>,
    pub redacted: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

pub async fn insert_message(
    pool: &PgPool,
    proposal_id: Uuid,
    sender_id: Uuid,
    body: &str,
    photo_key: Option<&str>,
    redacted: bool,
) -> sqlx::Result<Message> {
    sqlx::query_as::<_, Message>(
        "INSERT INTO messages (proposal_id, sender_id, body, photo_key, redacted) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, proposal_id, sender_id, body, photo_key, redacted, created_at, read_at",
    )
    .bind(proposal_id)
    .bind(sender_id)
    .bind(body)
    .bind(photo_key)
    .bind(redacted)
    .fetch_one(pool)
    .await
}

pub async fn list_messages(pool: &PgPool, proposal_id: Uuid) -> sqlx::Result<Vec<Message>> {
    sqlx::query_as::<_, Message>(
        "SELECT id, proposal_id, sender_id, body, photo_key, redacted, created_at, read_at \
         FROM messages WHERE proposal_id = $1 ORDER BY created_at LIMIT 500",
    )
    .bind(proposal_id)
    .fetch_all(pool)
    .await
}

/// Marque comme lus les messages reçus par `reader_id`. Retourne le nombre
/// de messages concernés (accusé de lecture à diffuser si > 0).
pub async fn mark_read(pool: &PgPool, proposal_id: Uuid, reader_id: Uuid) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE messages SET read_at = now() \
         WHERE proposal_id = $1 AND sender_id <> $2 AND read_at IS NULL",
    )
    .bind(proposal_id)
    .bind(reader_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Résumé de conversation pour la liste : dernier message + non-lus.
#[derive(Debug, Clone, FromRow)]
pub struct ConversationSummary {
    pub proposal_id: Uuid,
    pub last_body: Option<String>,
    pub last_sender_id: Option<Uuid>,
    pub last_at: Option<DateTime<Utc>>,
    pub unread_count: i64,
}

pub async fn conversation_summaries(
    pool: &PgPool,
    user_id: Uuid,
    proposal_ids: &[Uuid],
) -> sqlx::Result<Vec<ConversationSummary>> {
    sqlx::query_as::<_, ConversationSummary>(
        "SELECT p.id AS proposal_id, \
                d.body AS last_body, d.sender_id AS last_sender_id, d.created_at AS last_at, \
                (SELECT count(*) FROM messages m \
                 WHERE m.proposal_id = p.id AND m.sender_id <> $1 AND m.read_at IS NULL) \
                AS unread_count \
         FROM proposals p \
         LEFT JOIN LATERAL ( \
            SELECT body, sender_id, created_at FROM messages m \
            WHERE m.proposal_id = p.id ORDER BY m.created_at DESC LIMIT 1 \
         ) d ON TRUE \
         WHERE p.id = ANY($2)",
    )
    .bind(user_id)
    .bind(proposal_ids)
    .fetch_all(pool)
    .await
}

/// Un destinataire à relancer : messages non lus depuis plus de 24 h.
#[derive(Debug, Clone, FromRow)]
pub struct UnreadReminder {
    pub proposal_id: Uuid,
    pub recipient_email: String,
    pub recipient_pseudo: String,
    pub sender_pseudo: String,
}

/// Sélectionne et marque (reminded_at) les conversations à relancer :
/// au moins un message non lu, non relancé, envoyé il y a plus de 24 h.
pub async fn claim_unread_reminders(pool: &PgPool) -> sqlx::Result<Vec<UnreadReminder>> {
    sqlx::query_as::<_, UnreadReminder>(
        "WITH stale AS ( \
            UPDATE messages SET reminded_at = now() \
            WHERE read_at IS NULL AND reminded_at IS NULL \
              AND created_at < now() - interval '24 hours' \
            RETURNING proposal_id, sender_id \
         ), grouped AS ( \
            SELECT DISTINCT proposal_id, sender_id FROM stale \
         ) \
         SELECT g.proposal_id, \
                ur.email::text AS recipient_email, ur.pseudo::text AS recipient_pseudo, \
                us.pseudo::text AS sender_pseudo \
         FROM grouped g \
         JOIN proposals p ON p.id = g.proposal_id \
         JOIN users us ON us.id = g.sender_id \
         JOIN users ur ON ur.id = CASE WHEN g.sender_id = p.proposer_id \
                                       THEN p.recipient_id ELSE p.proposer_id END",
    )
    .fetch_all(pool)
    .await
}
