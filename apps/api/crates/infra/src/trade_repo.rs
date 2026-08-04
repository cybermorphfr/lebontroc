//! Requêtes SQL des propositions de troc (F3.1).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Proposal {
    pub id: Uuid,
    pub proposer_id: Uuid,
    pub recipient_id: Uuid,
    pub status: String,
    pub cash_cents: i32,
    pub cash_direction: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub viewed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
}

const PROPOSAL_COLUMNS: &str = "p.id, p.proposer_id, p.recipient_id, p.status, p.cash_cents, \
     p.cash_direction, p.message, p.created_at, p.viewed_at, p.expires_at, \
     up.pseudo::text AS proposer_pseudo, ur.pseudo::text AS recipient_pseudo";

#[derive(Debug, Clone, FromRow)]
pub struct ProposalItem {
    pub proposal_id: Uuid,
    pub item_id: Uuid,
    pub side: String,
    pub value_cents_snapshot: i32,
    pub title: String,
    pub s3_key: Option<String>,
}

/// Crée la proposition et fige la valeur des objets, en une transaction.
#[allow(clippy::too_many_arguments)]
pub async fn create_proposal(
    pool: &PgPool,
    proposer_id: Uuid,
    recipient_id: Uuid,
    cash_cents: i32,
    cash_direction: &str,
    message: Option<&str>,
    expires_at: DateTime<Utc>,
    items: &[(Uuid, &str, i32)], // (item_id, side, valeur figée)
) -> sqlx::Result<Uuid> {
    let mut tx = pool.begin().await?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO proposals (proposer_id, recipient_id, cash_cents, cash_direction, message, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(proposer_id)
    .bind(recipient_id)
    .bind(cash_cents)
    .bind(cash_direction)
    .bind(message)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await?;
    for (item_id, side, value) in items {
        sqlx::query(
            "INSERT INTO proposal_items (proposal_id, item_id, side, value_cents_snapshot) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(row.0)
        .bind(item_id)
        .bind(side)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(row.0)
}

pub async fn get_proposal(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Proposal>> {
    sqlx::query_as::<_, Proposal>(&format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals p \
         JOIN users up ON up.id = p.proposer_id \
         JOIN users ur ON ur.id = p.recipient_id \
         WHERE p.id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Boîte de réception (`recues`) ou d'envoi (`envoyees`), la plus récente d'abord.
pub async fn list_proposals(
    pool: &PgPool,
    user_id: Uuid,
    received: bool,
) -> sqlx::Result<Vec<Proposal>> {
    let filter = if received {
        "p.recipient_id = $1"
    } else {
        "p.proposer_id = $1"
    };
    sqlx::query_as::<_, Proposal>(&format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals p \
         JOIN users up ON up.id = p.proposer_id \
         JOIN users ur ON ur.id = p.recipient_id \
         WHERE {filter} ORDER BY p.created_at DESC LIMIT 100"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Les objets (avec titre et photo de couverture) d'un lot de propositions.
pub async fn proposal_items(
    pool: &PgPool,
    proposal_ids: &[Uuid],
) -> sqlx::Result<Vec<ProposalItem>> {
    sqlx::query_as::<_, ProposalItem>(
        "SELECT pi.proposal_id, pi.item_id, pi.side, pi.value_cents_snapshot, i.title, \
                (SELECT ip.s3_key FROM item_photos ip WHERE ip.item_id = i.id \
                 ORDER BY ip.position LIMIT 1) AS s3_key \
         FROM proposal_items pi \
         JOIN items i ON i.id = pi.item_id \
         WHERE pi.proposal_id = ANY($1) \
         ORDER BY pi.side, i.title",
    )
    .bind(proposal_ids)
    .fetch_all(pool)
    .await
}

/// Passe une proposition `envoyee` à `vue` (première ouverture par le
/// destinataire). Retourne `true` si la transition a eu lieu.
pub async fn mark_viewed(pool: &PgPool, id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE proposals SET status = 'vue', viewed_at = now(), updated_at = now() \
         WHERE id = $1 AND status = 'envoyee'",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Refus par le destinataire (garde SQL en plus de la règle domaine).
pub async fn refuse_proposal(pool: &PgPool, id: Uuid, recipient_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE proposals SET status = 'refusee', updated_at = now() \
         WHERE id = $1 AND recipient_id = $2 AND status IN ('envoyee', 'vue')",
    )
    .bind(id)
    .bind(recipient_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Une proposition expirée à notifier au proposant.
#[derive(Debug, Clone, FromRow)]
pub struct ExpiredProposal {
    pub id: Uuid,
    pub proposer_email: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
}

/// Expire les propositions sans réponse (Gherkin F3.1 : 7 jours) et
/// retourne de quoi notifier les proposants.
pub async fn expire_overdue(pool: &PgPool) -> sqlx::Result<Vec<ExpiredProposal>> {
    sqlx::query_as::<_, ExpiredProposal>(
        "WITH expired AS ( \
            UPDATE proposals SET status = 'expiree', updated_at = now() \
            WHERE status IN ('envoyee', 'vue') AND expires_at < now() \
            RETURNING id, proposer_id, recipient_id \
         ) \
         SELECT e.id, u.email::text AS proposer_email, u.pseudo::text AS proposer_pseudo, \
                r.pseudo::text AS recipient_pseudo \
         FROM expired e \
         JOIN users u ON u.id = e.proposer_id \
         JOIN users r ON r.id = e.recipient_id",
    )
    .fetch_all(pool)
    .await
}
