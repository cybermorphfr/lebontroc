//! Requêtes SQL des litiges, signalements, blocages et du score de
//! fiabilité (F5.2). Le score n'est jamais matérialisé : à l'échelle de la
//! bêta, la somme pondérée de `dispute_events` est gratuite et toujours juste.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Dispute {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub opened_by: Option<Uuid>,
    pub reason: String,
    pub description: String,
    pub status: String,
    pub response: Option<String>,
    pub responded_at: Option<DateTime<Utc>>,
    pub outcome: Option<String>,
    pub penalty: Option<String>,
    pub penalized_id: Option<Uuid>,
    pub admin_note: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

const DISPUTE_COLUMNS: &str = "id, trade_id, opened_by, reason, description, status, response, \
     responded_at, outcome, penalty, penalized_id, admin_note, opened_at, resolved_at";

/// Ouvre le dossier du troc — un seul par troc (contrainte UNIQUE).
/// Retourne None si un dossier existe déjà.
pub async fn open_dispute(
    pool: &PgPool,
    trade_id: Uuid,
    opened_by: Option<Uuid>,
    reason: &str,
    description: &str,
) -> sqlx::Result<Option<Dispute>> {
    sqlx::query_as::<_, Dispute>(&format!(
        "INSERT INTO disputes (trade_id, opened_by, reason, description) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (trade_id) DO NOTHING RETURNING {DISPUTE_COLUMNS}"
    ))
    .bind(trade_id)
    .bind(opened_by)
    .bind(reason)
    .bind(description)
    .fetch_optional(pool)
    .await
}

pub async fn dispute_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Option<Dispute>> {
    sqlx::query_as::<_, Dispute>(&format!(
        "SELECT {DISPUTE_COLUMNS} FROM disputes WHERE trade_id = $1"
    ))
    .bind(trade_id)
    .fetch_optional(pool)
    .await
}

pub async fn dispute_by_id(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Dispute>> {
    sqlx::query_as::<_, Dispute>(&format!(
        "SELECT {DISPUTE_COLUMNS} FROM disputes WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Réponse contradictoire de l'autre partie (une seule), fait passer le
/// dossier en examen.
pub async fn respond_to_dispute(
    pool: &PgPool,
    dispute_id: Uuid,
    response: &str,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE disputes SET response = $2, responded_at = now(), status = 'en_examen' \
         WHERE id = $1 AND status = 'ouvert' AND response IS NULL",
    )
    .bind(dispute_id)
    .bind(response)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Sans réponse sous 72 h, le dossier passe seul en examen.
pub async fn escalate_unanswered_disputes(pool: &PgPool, hours: i64) -> sqlx::Result<u64> {
    let updated = sqlx::query(
        "UPDATE disputes SET status = 'en_examen' \
         WHERE status = 'ouvert' AND opened_at < now() - make_interval(hours => $1::int)",
    )
    .bind(hours)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected())
}

/// Tranche le dossier (admin). Retourne false s'il était déjà tranché.
pub async fn resolve_dispute(
    pool: &PgPool,
    dispute_id: Uuid,
    outcome: &str,
    penalty: Option<&str>,
    penalized_id: Option<Uuid>,
    admin_note: Option<&str>,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE disputes SET status = 'tranche', outcome = $2, penalty = $3, \
                penalized_id = $4, admin_note = $5, resolved_at = now() \
         WHERE id = $1 AND status <> 'tranche'",
    )
    .bind(dispute_id)
    .bind(outcome)
    .bind(penalty)
    .bind(penalized_id)
    .bind(admin_note)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

pub async fn list_disputes(pool: &PgPool, status: Option<&str>) -> sqlx::Result<Vec<Dispute>> {
    sqlx::query_as::<_, Dispute>(&format!(
        "SELECT {DISPUTE_COLUMNS} FROM disputes \
         WHERE ($1::text IS NULL OR status = $1) ORDER BY opened_at DESC LIMIT 100"
    ))
    .bind(status)
    .fetch_all(pool)
    .await
}

/// Vue minimale d'un troc, sans contrôle de participant (admin, gardes).
#[derive(Debug, Clone, FromRow)]
pub struct TradeSummary {
    pub id: Uuid,
    pub proposer_id: Uuid,
    pub recipient_id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub cash_cents: i32,
    pub created_at: DateTime<Utc>,
    pub finalized_at: Option<DateTime<Utc>>,
}

pub async fn trade_summary(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Option<TradeSummary>> {
    sqlx::query_as::<_, TradeSummary>(
        "SELECT id, proposer_id, recipient_id, status, delivery_mode, cash_cents, \
                created_at, finalized_at \
         FROM trades WHERE id = $1",
    )
    .bind(trade_id)
    .fetch_optional(pool)
    .await
}

// ————— Transitions de résolution (admin) —————

/// Issue `capture` sur un troc non finalisé : le troc va au bout — objets
/// `troque`, colis restants clos. Idempotent.
pub async fn resolve_finalize_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let proposal_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE trades SET status = 'finalise', finalized_at = now() \
         WHERE id = $1 AND status IN ('accepte', 'litige_gele') RETURNING proposal_id",
    )
    .bind(trade_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(proposal_id) = proposal_id else {
        return Ok(false);
    };
    sqlx::query(
        "UPDATE items SET status = 'troque', updated_at = now() \
         WHERE id IN (SELECT item_id FROM proposal_items WHERE proposal_id = $1)",
    )
    .bind(proposal_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE shipments SET status = 'annule', updated_at = now() \
         WHERE trade_id = $1 AND status NOT IN ('confirme', 'annule')",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Issue `liberation` sur un troc non finalisé : troc annulé, objets
/// libérés, colis clos (les retours s'organisent hors plateforme en bêta).
pub async fn resolve_cancel_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let proposal_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE trades SET status = 'annule', cancelled_at = now() \
         WHERE id = $1 AND status IN ('accepte', 'litige_gele') RETURNING proposal_id",
    )
    .bind(trade_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(proposal_id) = proposal_id else {
        return Ok(false);
    };
    sqlx::query(
        "UPDATE items SET status = 'disponible', updated_at = now() \
         WHERE status = 'reserve' \
           AND id IN (SELECT item_id FROM proposal_items WHERE proposal_id = $1)",
    )
    .bind(proposal_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE shipments SET status = 'annule', updated_at = now() \
         WHERE trade_id = $1 AND status NOT IN ('confirme', 'annule')",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Issue `rejet` sur un troc gelé : dégel, le parcours normal reprend
/// (l'auto-confirmation redevient possible dès le dossier tranché).
pub async fn unfreeze_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE trades SET status = 'accepte' WHERE id = $1 AND status = 'litige_gele'",
    )
    .bind(trade_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

// ————— Pièces (bucket privé) —————

#[derive(Debug, Clone, FromRow)]
pub struct DisputePhoto {
    pub id: Uuid,
    pub uploader_id: Uuid,
    pub s3_key: String,
}

pub async fn attach_photos(
    pool: &PgPool,
    dispute_id: Uuid,
    uploader_id: Uuid,
    keys: &[String],
) -> sqlx::Result<()> {
    for key in keys {
        sqlx::query(
            "INSERT INTO dispute_photos (dispute_id, uploader_id, s3_key) VALUES ($1, $2, $3)",
        )
        .bind(dispute_id)
        .bind(uploader_id)
        .bind(key)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn photos_for_dispute(
    pool: &PgPool,
    dispute_id: Uuid,
) -> sqlx::Result<Vec<DisputePhoto>> {
    sqlx::query_as::<_, DisputePhoto>(
        "SELECT id, uploader_id, s3_key FROM dispute_photos \
         WHERE dispute_id = $1 ORDER BY created_at",
    )
    .bind(dispute_id)
    .fetch_all(pool)
    .await
}

// ————— Signalements —————

pub async fn create_report(
    pool: &PgPool,
    reporter_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    reason: &str,
    comment: Option<&str>,
) -> sqlx::Result<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO reports (reporter_id, target_type, target_id, reason, comment) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(reporter_id)
    .bind(target_type)
    .bind(target_id)
    .bind(reason)
    .bind(comment)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

// ————— Blocages —————

/// Bloque et caduque les propositions en attente dans les deux sens.
/// Retourne false si déjà bloqué.
pub async fn block_user(pool: &PgPool, blocker_id: Uuid, blocked_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO user_blocks (blocker_id, blocked_id) VALUES ($1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(blocker_id)
    .bind(blocked_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE proposals SET status = 'caduque' \
         WHERE status IN ('envoyee', 'vue', 'contre_proposee') \
           AND ((proposer_id = $1 AND recipient_id = $2) \
             OR (proposer_id = $2 AND recipient_id = $1))",
    )
    .bind(blocker_id)
    .bind(blocked_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(inserted.rows_affected() > 0)
}

pub async fn unblock_user(pool: &PgPool, blocker_id: Uuid, blocked_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2")
        .bind(blocker_id)
        .bind(blocked_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Un blocage existe-t-il dans un sens ou l'autre ?
pub async fn is_blocked_either_way(pool: &PgPool, a: Uuid, b: Uuid) -> sqlx::Result<bool> {
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM user_blocks \
         WHERE (blocker_id = $1 AND blocked_id = $2) \
            OR (blocker_id = $2 AND blocked_id = $1))",
    )
    .bind(a)
    .bind(b)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Pseudos bloqués par l'utilisateur (pour l'écran de réglages).
pub async fn blocked_pseudos(pool: &PgPool, blocker_id: Uuid) -> sqlx::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT u.pseudo::text FROM user_blocks b JOIN users u ON u.id = b.blocked_id \
         WHERE b.blocker_id = $1 ORDER BY b.created_at DESC",
    )
    .bind(blocker_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(p,)| p).collect())
}

// ————— Score de fiabilité et sanctions —————

/// Score = somme pondérée des événements négatifs du journal.
/// La pondération SQL doit rester alignée sur `domain::dispute::points`.
pub async fn reliability_score(pool: &PgPool, user_id: Uuid) -> sqlx::Result<i32> {
    let (score,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(sum(CASE event_type \
             WHEN 'contrefacon_averee' THEN 15 \
             WHEN 'litige_perdu' THEN 6 \
             WHEN 'non_depot' THEN 5 \
             WHEN 'no_show_confirme' THEN 4 \
             WHEN 'litige_abusif' THEN 2 \
             WHEN 'signalement_fonde' THEN 2 \
             ELSE 0 END), 0) \
         FROM dispute_events WHERE culprit_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(score as i32)
}

/// Restreint l'utilisateur (plus de nouvelles propositions) pour `days` jours.
pub async fn restrict_user(pool: &PgPool, user_id: Uuid, days: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE users SET restricted_until = now() + make_interval(days => $2::int) WHERE id = $1",
    )
    .bind(user_id)
    .bind(days)
    .execute(pool)
    .await?;
    Ok(())
}

/// Bannit : compte marqué + toutes les sessions révoquées.
pub async fn ban_user(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET banned_at = now() WHERE id = $1 AND banned_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Lève bannissement et restriction (filet admin des sanctions automatiques).
pub async fn lift_sanctions(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET banned_at = NULL, restricted_until = NULL WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
pub struct UserSanctionState {
    pub restricted_until: Option<DateTime<Utc>>,
    pub banned_at: Option<DateTime<Utc>>,
}

pub async fn sanction_state(pool: &PgPool, user_id: Uuid) -> sqlx::Result<UserSanctionState> {
    sqlx::query_as::<_, UserSanctionState>(
        "SELECT restricted_until, banned_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}
