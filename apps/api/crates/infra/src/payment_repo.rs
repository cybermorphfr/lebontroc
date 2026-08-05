//! Requêtes SQL des paiements de soulte (F4.2). Les transitions sont
//! idempotentes par construction (`UPDATE … WHERE status IN (…)`) : un
//! webhook rejoué ou un double clic ne change l'état qu'une seule fois.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::trade_repo::{self, Eviction, TradeParty};

#[derive(Debug, Clone, FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub payer_id: Uuid,
    pub beneficiary_id: Uuid,
    pub amount_cents: i32,
    pub fees_cents: i32,
    pub status: String,
    pub provider: String,
    pub provider_ref: Option<String>,
    pub failure_reason: Option<String>,
    pub attempts: i32,
    pub deadline: DateTime<Utc>,
    pub escrowed_at: Option<DateTime<Utc>>,
    pub captured_at: Option<DateTime<Utc>>,
}

const PAYMENT_COLUMNS: &str = "id, trade_id, payer_id, beneficiary_id, amount_cents, fees_cents, \
     status, provider, provider_ref, failure_reason, attempts, deadline, escrowed_at, captured_at";

/// Paiement à créer avec le troc (même transaction, voir `accept_proposal`).
#[derive(Debug, Clone)]
pub struct NewPayment {
    pub payer_id: Uuid,
    pub beneficiary_id: Uuid,
    pub amount_cents: i32,
    pub fees_cents: i32,
    pub provider: String,
    pub deadline: DateTime<Utc>,
}

pub async fn payment_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Option<Payment>> {
    sqlx::query_as::<_, Payment>(&format!(
        "SELECT {PAYMENT_COLUMNS} FROM payments WHERE trade_id = $1"
    ))
    .bind(trade_id)
    .fetch_optional(pool)
    .await
}

/// Trace une tentative de préautorisation.
pub async fn record_attempt(pool: &PgPool, payment_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE payments SET attempts = attempts + 1, updated_at = now() WHERE id = $1")
        .bind(payment_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Issue de l'enregistrement d'un séquestre.
pub enum EscrowOutcome {
    /// Le troc passe à `accepte` ; les propositions concurrentes qui visaient
    /// ses objets deviennent caduques (différé depuis l'acceptation : tant
    /// que la soulte n'était pas réglée, elles gardaient leur chance).
    Escrowed { evictions: Vec<Eviction> },
    /// Rejeu (webhook, double clic) : rien ne change une seconde fois.
    AlreadyEscrowed,
    /// Le paiement n'était plus séquestrable (expiré, annulé).
    NotPending,
}

/// Enregistre le séquestre et active le troc — idempotent.
pub async fn escrow_payment(
    pool: &PgPool,
    trade_id: Uuid,
    provider_ref: Option<&str>,
) -> sqlx::Result<EscrowOutcome> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE payments SET status = 'sequestre', provider_ref = COALESCE($2, provider_ref), \
         failure_reason = NULL, escrowed_at = now(), updated_at = now() \
         WHERE trade_id = $1 AND status IN ('en_attente', 'echoue')",
    )
    .bind(trade_id)
    .bind(provider_ref)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM payments WHERE trade_id = $1")
                .bind(trade_id)
                .fetch_optional(&mut *tx)
                .await?;
        return Ok(match status.as_deref() {
            Some("sequestre") | Some("capture") => EscrowOutcome::AlreadyEscrowed,
            _ => EscrowOutcome::NotPending,
        });
    }
    let proposal_id: Uuid = sqlx::query_scalar(
        "UPDATE trades SET status = 'accepte' \
         WHERE id = $1 AND status = 'attente_paiement' RETURNING proposal_id",
    )
    .bind(trade_id)
    .fetch_one(&mut *tx)
    .await?;
    let item_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT item_id FROM proposal_items WHERE proposal_id = $1")
            .bind(proposal_id)
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;

    // Caducité des concurrentes hors transaction (même leçon que F3.3).
    let evictions = trade_repo::invalidate_competitors(pool, proposal_id, &item_ids).await?;
    Ok(EscrowOutcome::Escrowed { evictions })
}

/// Enregistre un refus de préautorisation — le payeur peut retenter.
pub async fn mark_failed(pool: &PgPool, payment_id: Uuid, reason: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE payments SET status = 'echoue', failure_reason = $2, updated_at = now() \
         WHERE id = $1 AND status IN ('en_attente', 'echoue')",
    )
    .bind(payment_id)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(())
}

/// Enregistre la capture (remise confirmée) — idempotent.
pub async fn mark_captured(pool: &PgPool, payment_id: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE payments SET status = 'capture', captured_at = now(), updated_at = now() \
         WHERE id = $1 AND status = 'sequestre'",
    )
    .bind(payment_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Libère le paiement d'un troc annulé — idempotent. Retourne le paiement
/// s'il restait quelque chose à libérer chez le PSP.
pub async fn cancel_payment_for_trade(
    pool: &PgPool,
    trade_id: Uuid,
) -> sqlx::Result<Option<Payment>> {
    sqlx::query_as::<_, Payment>(&format!(
        "UPDATE payments SET status = 'annule', cancelled_at = now(), updated_at = now() \
         WHERE trade_id = $1 AND status IN ('en_attente', 'echoue', 'sequestre') \
         RETURNING {PAYMENT_COLUMNS}"
    ))
    .bind(trade_id)
    .fetch_optional(pool)
    .await
}

/// Les trocs en attente d'un paiement dont la date limite est dépassée.
pub async fn overdue_unpaid_trades(pool: &PgPool) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar(
        "SELECT t.id FROM trades t \
         JOIN payments p ON p.trade_id = t.id \
         WHERE t.status = 'attente_paiement' \
           AND p.status IN ('en_attente', 'echoue') AND p.deadline < now()",
    )
    .fetch_all(pool)
    .await
}

/// Annule un troc jamais payé : paiement `expire`, troc `annule`, objets
/// libérés — atomique et idempotent. Retourne les parties à notifier
/// (vide si un autre chemin est passé avant).
pub async fn expire_unpaid_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Vec<TradeParty>> {
    let mut tx = pool.begin().await?;
    let Some((proposal_id,)) = sqlx::query_as::<_, (Uuid,)>(
        "SELECT proposal_id FROM trades WHERE id = $1 AND status = 'attente_paiement' FOR UPDATE",
    )
    .bind(trade_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(Vec::new());
    };
    let expired = sqlx::query(
        "UPDATE payments SET status = 'expire', updated_at = now() \
         WHERE trade_id = $1 AND status IN ('en_attente', 'echoue') AND deadline < now()",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    if expired.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    trade_repo::cancel_trade_in_tx(&mut tx, trade_id, proposal_id).await?;
    let parties = sqlx::query_as::<_, TradeParty>(
        "SELECT t.id AS trade_id, u.email::text AS email, u.pseudo::text AS pseudo, \
                o.pseudo::text AS other_pseudo \
         FROM trades t \
         JOIN users u ON u.id IN (t.proposer_id, t.recipient_id) \
         JOIN users o ON o.id = CASE WHEN u.id = t.proposer_id \
                                     THEN t.recipient_id ELSE t.proposer_id END \
         WHERE t.id = $1",
    )
    .bind(trade_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(parties)
}

/// Les paiements séquestrés d'un troc finalisé dont la capture a échoué au
/// moment de la remise — la maintenance horaire retente.
pub async fn payments_to_capture(pool: &PgPool) -> sqlx::Result<Vec<Payment>> {
    sqlx::query_as::<_, Payment>(&format!(
        "SELECT {PAYMENT_COLUMNS} FROM payments p0 WHERE status = 'sequestre' AND EXISTS ( \
            SELECT 1 FROM trades t WHERE t.id = p0.trade_id AND t.status = 'finalise')"
    ))
    .fetch_all(pool)
    .await
}
