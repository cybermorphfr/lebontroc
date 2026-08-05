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
    pub counter_of: Option<Uuid>,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
}

const PROPOSAL_COLUMNS: &str = "p.id, p.proposer_id, p.recipient_id, p.status, p.cash_cents, \
     p.cash_direction, p.message, p.created_at, p.viewed_at, p.expires_at, p.counter_of, \
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

/// Toutes les propositions où l'utilisateur est partie prenante.
pub async fn list_user_proposals(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<Proposal>> {
    sqlx::query_as::<_, Proposal>(&format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals p \
         JOIN users up ON up.id = p.proposer_id \
         JOIN users ur ON ur.id = p.recipient_id \
         WHERE p.proposer_id = $1 OR p.recipient_id = $1 \
         ORDER BY p.created_at DESC LIMIT 100"
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

// ————— Acceptation atomique (F3.3) —————

#[derive(Debug, Clone, FromRow)]
pub struct Trade {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub cash_cents: i32,
    pub cash_direction: String,
    pub created_at: DateTime<Utc>,
}

/// Un proposant évincé par la caducité de sa proposition.
#[derive(Debug, Clone, FromRow)]
pub struct Eviction {
    pub proposal_id: Uuid,
    pub proposer_email: String,
    pub proposer_pseudo: String,
}

/// Issue métier d'une tentative d'acceptation.
pub enum AcceptOutcome {
    /// Le troc est créé : objets réservés, concurrentes caduques.
    Accepted(Trade, Vec<Eviction>),
    /// Déjà acceptée (double clic, retry) : le troc existant est renvoyé.
    AlreadyAccepted(Trade),
    /// La proposition n'existe pas ou n'est pas adressée à ce destinataire.
    NotFound,
    /// La proposition n'est plus ouverte (refusée, expirée, caduque…).
    NotOpen(String),
    /// Course perdue : un objet n'est plus disponible.
    ItemsUnavailable,
}

/// LA transaction critique du système (Gherkin F3.3) : verrouillage de la
/// proposition puis des objets (ordre stable anti-deadlock), vérification de
/// disponibilité, passage en `reserve`, création du Trade, caducité des
/// propositions concurrentes. Deux acceptations simultanées visant le même
/// objet : la seconde attend le verrou, voit `reserve`, échoue proprement.
///
/// Avec paiement(s) — soulte (F4.2) ou frais d'envoi (F4.3) — le troc naît
/// en `attente_paiement`, les paiements sont créés dans la même transaction,
/// et la caducité des concurrentes est DIFFÉRÉE au dernier séquestre — tant
/// que tout n'est pas réglé, les autres propositions gardent leur chance.
/// En mode `envoi`, les deux colis (un par direction) naissent aussi ici.
pub async fn accept_proposal(
    pool: &PgPool,
    proposal_id: Uuid,
    recipient_id: Uuid,
    delivery_mode: &str,
    codes: (&str, &str), // (code du proposant, code du destinataire)
    payments: &[crate::payment_repo::NewPayment],
) -> sqlx::Result<AcceptOutcome> {
    let mut tx = pool.begin().await?;

    #[derive(FromRow)]
    struct Locked {
        proposer_id: Uuid,
        recipient_id: Uuid,
        status: String,
        cash_cents: i32,
        cash_direction: String,
    }
    let Some(proposal) = sqlx::query_as::<_, Locked>(
        "SELECT proposer_id, recipient_id, status, cash_cents, cash_direction \
         FROM proposals WHERE id = $1 FOR UPDATE",
    )
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(AcceptOutcome::NotFound);
    };
    if proposal.recipient_id != recipient_id {
        return Ok(AcceptOutcome::NotFound);
    }
    if proposal.status == "acceptee" {
        let trade = sqlx::query_as::<_, Trade>(
            "SELECT id, proposal_id, status, delivery_mode, cash_cents, cash_direction, \
             created_at FROM trades WHERE proposal_id = $1",
        )
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await?;
        return Ok(AcceptOutcome::AlreadyAccepted(trade));
    }
    if !matches!(proposal.status.as_str(), "envoyee" | "vue") {
        return Ok(AcceptOutcome::NotOpen(proposal.status));
    }

    let item_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT item_id FROM proposal_items WHERE proposal_id = $1")
            .bind(proposal_id)
            .fetch_all(&mut *tx)
            .await?;

    // Verrouillage des objets dans un ordre stable (anti-deadlock), puis
    // vérification : tous encore disponibles et non supprimés.
    let locked: Vec<(Uuid, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, status, deleted_at FROM items WHERE id = ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&item_ids)
    .fetch_all(&mut *tx)
    .await?;
    if locked.len() != item_ids.len()
        || locked
            .iter()
            .any(|(_, status, deleted)| status != "disponible" || deleted.is_some())
    {
        return Ok(AcceptOutcome::ItemsUnavailable);
    }

    sqlx::query("UPDATE items SET status = 'reserve', updated_at = now() WHERE id = ANY($1)")
        .bind(&item_ids)
        .execute(&mut *tx)
        .await?;

    let status = if payments.is_empty() {
        "accepte"
    } else {
        "attente_paiement"
    };
    let trade = sqlx::query_as::<_, Trade>(
        "INSERT INTO trades (proposal_id, proposer_id, recipient_id, status, delivery_mode, \
         cash_cents, cash_direction, proposer_code, recipient_code) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         RETURNING id, proposal_id, status, delivery_mode, cash_cents, cash_direction, created_at",
    )
    .bind(proposal_id)
    .bind(proposal.proposer_id)
    .bind(proposal.recipient_id)
    .bind(status)
    .bind(delivery_mode)
    .bind(proposal.cash_cents)
    .bind(&proposal.cash_direction)
    .bind(codes.0)
    .bind(codes.1)
    .fetch_one(&mut *tx)
    .await?;

    for payment in payments {
        sqlx::query(
            "INSERT INTO payments (trade_id, payer_id, beneficiary_id, amount_cents, \
             fees_cents, service_cents, provider, deadline) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(trade.id)
        .bind(payment.payer_id)
        .bind(payment.beneficiary_id)
        .bind(payment.amount_cents)
        .bind(payment.fees_cents)
        .bind(payment.service_cents)
        .bind(&payment.provider)
        .bind(payment.deadline)
        .execute(&mut *tx)
        .await?;
    }

    // Mode envoi : deux colis, un par direction (F4.3).
    if delivery_mode == "envoi" {
        for (sender, recipient) in [
            (proposal.proposer_id, proposal.recipient_id),
            (proposal.recipient_id, proposal.proposer_id),
        ] {
            sqlx::query(
                "INSERT INTO shipments (trade_id, sender_id, recipient_id) VALUES ($1, $2, $3)",
            )
            .bind(trade.id)
            .bind(sender)
            .bind(recipient)
            .execute(&mut *tx)
            .await?;
        }
    }

    sqlx::query("UPDATE proposals SET status = 'acceptee', updated_at = now() WHERE id = $1")
        .bind(proposal_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Sans paiement, les concurrentes deviennent caduques tout de suite.
    // Avec paiement(s), ce serait injuste : si tout n'est jamais réglé, les
    // objets seront libérés — les autres propositions doivent rester
    // vivantes. La caducité se joue alors au dernier séquestre
    // (`payment_repo::escrow_payment`).
    let evictions = if payments.is_empty() {
        invalidate_competitors(pool, proposal_id, &item_ids).await?
    } else {
        Vec::new()
    };

    Ok(AcceptOutcome::Accepted(trade, evictions))
}

/// Caducité des propositions concurrentes visant un des objets réservés —
/// APRÈS le commit, hors transaction : une acceptation concurrente tient
/// sa proposition en FOR UPDATE pendant qu'elle attend un objet, la rendre
/// caduque depuis la transaction critique créerait un deadlock. Ici la
/// gagnante ne tient plus aucun verrou ; l'UPDATE attend simplement le
/// rollback des perdantes. Idempotent et sans danger en cas de crash :
/// une proposition zombie échouerait proprement à l'acceptation.
pub(crate) async fn invalidate_competitors(
    pool: &PgPool,
    winner_proposal_id: Uuid,
    item_ids: &[Uuid],
) -> sqlx::Result<Vec<Eviction>> {
    sqlx::query_as::<_, Eviction>(
        "WITH victimes AS ( \
            UPDATE proposals SET status = 'caduque', updated_at = now() \
            WHERE id IN ( \
                SELECT DISTINCT p.id FROM proposals p \
                JOIN proposal_items pi ON pi.proposal_id = p.id \
                WHERE pi.item_id = ANY($1) AND p.id <> $2 \
                  AND p.status IN ('envoyee', 'vue') \
            ) \
            RETURNING id, proposer_id \
         ) \
         SELECT v.id AS proposal_id, u.email::text AS proposer_email, \
                u.pseudo::text AS proposer_pseudo \
         FROM victimes v JOIN users u ON u.id = v.proposer_id",
    )
    .bind(item_ids)
    .bind(winner_proposal_id)
    .fetch_all(pool)
    .await
}

/// Contre-proposition : bascule l'ancienne en `contre_proposee`, crée la
/// nouvelle (rôles inversés, chaînée par `counter_of`) et déplace le fil de
/// conversation — le tout atomiquement. `None` si l'ancienne n'était plus ouverte.
#[allow(clippy::too_many_arguments)]
pub async fn counter_proposal(
    pool: &PgPool,
    old_id: Uuid,
    proposer_id: Uuid,
    recipient_id: Uuid,
    cash_cents: i32,
    cash_direction: &str,
    message: Option<&str>,
    expires_at: DateTime<Utc>,
    items: &[(Uuid, &str, i32)],
) -> sqlx::Result<Option<Uuid>> {
    let mut tx = pool.begin().await?;
    let closed = sqlx::query(
        "UPDATE proposals SET status = 'contre_proposee', updated_at = now() \
         WHERE id = $1 AND status IN ('envoyee', 'vue')",
    )
    .bind(old_id)
    .execute(&mut *tx)
    .await?;
    if closed.rows_affected() == 0 {
        return Ok(None);
    }

    let new_id: Uuid = sqlx::query_scalar(
        "INSERT INTO proposals (proposer_id, recipient_id, cash_cents, cash_direction, \
         message, expires_at, counter_of) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(proposer_id)
    .bind(recipient_id)
    .bind(cash_cents)
    .bind(cash_direction)
    .bind(message)
    .bind(expires_at)
    .bind(old_id)
    .fetch_one(&mut *tx)
    .await?;
    for (item_id, side, value) in items {
        sqlx::query(
            "INSERT INTO proposal_items (proposal_id, item_id, side, value_cents_snapshot) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(new_id)
        .bind(item_id)
        .bind(side)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }
    // Le fil suit la négociation : l'historique reste dans la conversation.
    sqlx::query("UPDATE messages SET proposal_id = $1 WHERE proposal_id = $2")
        .bind(new_id)
        .bind(old_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(new_id))
}

/// Les trocs créés pour un lot de propositions.
pub async fn trades_for_proposals(
    pool: &PgPool,
    proposal_ids: &[Uuid],
) -> sqlx::Result<Vec<Trade>> {
    sqlx::query_as::<_, Trade>(
        "SELECT id, proposal_id, status, delivery_mode, cash_cents, cash_direction, created_at \
         FROM trades WHERE proposal_id = ANY($1)",
    )
    .bind(proposal_ids)
    .fetch_all(pool)
    .await
}

/// Qui a remplacé qui : (ancienne, nouvelle) pour un lot de propositions.
pub async fn superseded_by(
    pool: &PgPool,
    proposal_ids: &[Uuid],
) -> sqlx::Result<Vec<(Uuid, Uuid)>> {
    sqlx::query_as("SELECT counter_of, id FROM proposals WHERE counter_of = ANY($1)")
        .bind(proposal_ids)
        .fetch_all(pool)
        .await
}

// ————— Remise en main propre (F4.1) —————

/// Vue complète d'un troc pour l'écran de rendez-vous.
#[derive(Debug, Clone, FromRow)]
pub struct TradeDetail {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub proposer_id: Uuid,
    pub recipient_id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub cash_cents: i32,
    pub created_at: DateTime<Utc>,
    pub proposer_code: Option<String>,
    pub recipient_code: Option<String>,
    pub proposer_confirmed_at: Option<DateTime<Utc>>,
    pub recipient_confirmed_at: Option<DateTime<Utc>>,
    pub finalized_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancel_requested_by: Option<Uuid>,
}

const TRADE_DETAIL_COLUMNS: &str = "id, proposal_id, proposer_id, recipient_id, status, \
     delivery_mode, cash_cents, created_at, proposer_code, recipient_code, \
     proposer_confirmed_at, recipient_confirmed_at, finalized_at, cancelled_at, \
     cancel_requested_by";

pub async fn get_trade(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<TradeDetail>> {
    sqlx::query_as::<_, TradeDetail>(&format!(
        "SELECT {TRADE_DETAIL_COLUMNS} FROM trades WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Issue d'une saisie de code au rendez-vous.
pub enum ConfirmOutcome {
    /// Confirmation enregistrée ; `true` si le troc vient d'être finalisé.
    Confirmed {
        finalized: bool,
    },
    WrongCode,
    NotFound,
    /// Le troc n'est plus en attente (finalisé ou annulé).
    NotActive(String),
}

/// Chaque partie saisit le code de L'AUTRE. Quand les deux confirmations
/// sont là : `finalise`, objets en `troque` (Gherkin F4.1) — atomique.
pub async fn confirm_trade(
    pool: &PgPool,
    trade_id: Uuid,
    user_id: Uuid,
    code: &str,
) -> sqlx::Result<ConfirmOutcome> {
    let mut tx = pool.begin().await?;
    let Some(trade) = sqlx::query_as::<_, TradeDetail>(&format!(
        "SELECT {TRADE_DETAIL_COLUMNS} FROM trades WHERE id = $1 FOR UPDATE"
    ))
    .bind(trade_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(ConfirmOutcome::NotFound);
    };
    let is_proposer = trade.proposer_id == user_id;
    if !is_proposer && trade.recipient_id != user_id {
        return Ok(ConfirmOutcome::NotFound);
    }
    if trade.status != "accepte" {
        return Ok(ConfirmOutcome::NotActive(trade.status));
    }

    // Le code attendu est celui de l'autre partie.
    let expected = if is_proposer {
        trade.recipient_code.as_deref()
    } else {
        trade.proposer_code.as_deref()
    };
    if expected != Some(code) {
        return Ok(ConfirmOutcome::WrongCode);
    }

    let column = if is_proposer {
        "proposer_confirmed_at"
    } else {
        "recipient_confirmed_at"
    };
    sqlx::query(&format!(
        "UPDATE trades SET {column} = COALESCE({column}, now()) WHERE id = $1"
    ))
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;

    let (both,): (bool,) = sqlx::query_as(
        "SELECT proposer_confirmed_at IS NOT NULL AND recipient_confirmed_at IS NOT NULL \
         FROM trades WHERE id = $1",
    )
    .bind(trade_id)
    .fetch_one(&mut *tx)
    .await?;
    if both {
        sqlx::query("UPDATE trades SET status = 'finalise', finalized_at = now() WHERE id = $1")
            .bind(trade_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE items SET status = 'troque', updated_at = now() \
             WHERE id IN (SELECT item_id FROM proposal_items WHERE proposal_id = $1)",
        )
        .bind(trade.proposal_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(ConfirmOutcome::Confirmed { finalized: both })
}

/// Issue d'une demande d'annulation d'un commun accord.
pub enum CancelOutcome {
    /// Première demande enregistrée : on attend l'accord de l'autre.
    Pending,
    /// Les deux sont d'accord : troc annulé, objets libérés.
    Cancelled,
    NotFound,
    NotActive(String),
    /// Mode envoi : au moins un colis est déjà en route — trop tard pour
    /// une annulation amiable (le signalement prendra le relais).
    ParcelsMoving,
}

/// Annulation d'un commun accord : la première partie demande, la seconde
/// confirme — le troc passe en `annule` et les objets redeviennent disponibles.
pub async fn request_cancel(
    pool: &PgPool,
    trade_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<CancelOutcome> {
    let mut tx = pool.begin().await?;
    let Some(trade) = sqlx::query_as::<_, TradeDetail>(&format!(
        "SELECT {TRADE_DETAIL_COLUMNS} FROM trades WHERE id = $1 FOR UPDATE"
    ))
    .bind(trade_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(CancelOutcome::NotFound);
    };
    if trade.proposer_id != user_id && trade.recipient_id != user_id {
        return Ok(CancelOutcome::NotFound);
    }
    if trade.status != "accepte" {
        return Ok(CancelOutcome::NotActive(trade.status));
    }
    // Mode envoi : plus d'annulation amiable dès qu'un colis est en route.
    let (moving,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM shipments WHERE trade_id = $1 \
         AND status NOT IN ('preparation', 'etiquette', 'annule')",
    )
    .bind(trade_id)
    .fetch_one(&mut *tx)
    .await?;
    if moving > 0 {
        return Ok(CancelOutcome::ParcelsMoving);
    }

    match trade.cancel_requested_by {
        None => {
            sqlx::query("UPDATE trades SET cancel_requested_by = $2 WHERE id = $1")
                .bind(trade_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(CancelOutcome::Pending)
        }
        Some(by) if by == user_id => Ok(CancelOutcome::Pending),
        Some(_) => {
            cancel_trade_in_tx(&mut tx, trade_id, trade.proposal_id).await?;
            tx.commit().await?;
            Ok(CancelOutcome::Cancelled)
        }
    }
}

pub(crate) async fn cancel_trade_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    trade_id: Uuid,
    proposal_id: Uuid,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE trades SET status = 'annule', cancelled_at = now() WHERE id = $1")
        .bind(trade_id)
        .execute(&mut **tx)
        .await?;
    // Les objets encore réservés redeviennent disponibles.
    sqlx::query(
        "UPDATE items SET status = 'disponible', updated_at = now() \
         WHERE status = 'reserve' \
           AND id IN (SELECT item_id FROM proposal_items WHERE proposal_id = $1)",
    )
    .bind(proposal_id)
    .execute(&mut **tx)
    .await?;
    // Les colis jamais partis n'ont plus lieu d'être (mode envoi).
    sqlx::query(
        "UPDATE shipments SET status = 'annule', updated_at = now() \
         WHERE trade_id = $1 AND status IN ('preparation', 'etiquette')",
    )
    .bind(trade_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Une partie prenante d'un troc à notifier (relance ou annulation).
#[derive(Debug, Clone, FromRow)]
pub struct TradeParty {
    pub trade_id: Uuid,
    pub email: String,
    pub pseudo: String,
    pub other_pseudo: String,
}

/// Relance J+7 : trocs acceptés sans finalisation depuis plus de 7 jours,
/// jamais relancés. Marque et retourne les deux parties de chaque troc.
pub async fn claim_meetup_reminders(pool: &PgPool) -> sqlx::Result<Vec<TradeParty>> {
    sqlx::query_as::<_, TradeParty>(
        "WITH stale AS ( \
            UPDATE trades SET reminded_at = now() \
            WHERE status = 'accepte' AND reminded_at IS NULL \
              AND created_at < now() - interval '7 days' \
            RETURNING id, proposer_id, recipient_id \
         ) \
         SELECT s.id AS trade_id, u.email::text AS email, u.pseudo::text AS pseudo, \
                o.pseudo::text AS other_pseudo \
         FROM stale s \
         JOIN users u ON u.id IN (s.proposer_id, s.recipient_id) \
         JOIN users o ON o.id = CASE WHEN u.id = s.proposer_id \
                                     THEN s.recipient_id ELSE s.proposer_id END",
    )
    .fetch_all(pool)
    .await
}

/// Annulation automatique J+14 (Gherkin « rendez-vous fantôme ») : trocs
/// MAIN PROPRE acceptés jamais finalisés — annulés, objets libérés, parties
/// notifiées. Les trocs envoi ont leurs propres échéances (J+5 dépôt,
/// gel J+21) : un colis en transit ne doit jamais être annulé d'office.
pub async fn auto_cancel_stale_trades(pool: &PgPool) -> sqlx::Result<Vec<TradeParty>> {
    let mut tx = pool.begin().await?;
    let stale: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, proposal_id FROM trades \
         WHERE status = 'accepte' AND delivery_mode = 'main_propre' \
           AND created_at < now() - interval '14 days' \
         FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (trade_id, proposal_id) in &stale {
        cancel_trade_in_tx(&mut tx, *trade_id, *proposal_id).await?;
    }
    let ids: Vec<Uuid> = stale.iter().map(|(id, _)| *id).collect();
    let parties = sqlx::query_as::<_, TradeParty>(
        "SELECT t.id AS trade_id, u.email::text AS email, u.pseudo::text AS pseudo, \
                o.pseudo::text AS other_pseudo \
         FROM trades t \
         JOIN users u ON u.id IN (t.proposer_id, t.recipient_id) \
         JOIN users o ON o.id = CASE WHEN u.id = t.proposer_id \
                                     THEN t.recipient_id ELSE t.proposer_id END \
         WHERE t.id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(parties)
}
