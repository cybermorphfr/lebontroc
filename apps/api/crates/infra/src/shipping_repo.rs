//! Requêtes SQL des colis (F4.3). Deux lignes par troc en mode envoi, une
//! par direction. Transitions idempotentes (`UPDATE … WHERE status = …`).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Shipment {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub status: String,
    pub format: Option<String>,
    pub relay_code: Option<String>,
    pub relay_name: Option<String>,
    pub relay_address: Option<String>,
    pub provider_ref: Option<String>,
    pub drop_code: Option<String>,
    pub dropped_at: Option<DateTime<Utc>>,
    pub arrived_at: Option<DateTime<Utc>>,
    pub picked_up_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub issue_reason: Option<String>,
}

const SHIPMENT_COLUMNS: &str = "id, trade_id, sender_id, recipient_id, status, format, \
     relay_code, relay_name, relay_address, provider_ref, drop_code, dropped_at, arrived_at, \
     picked_up_at, confirmed_at, issue_reason";

pub async fn shipments_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Vec<Shipment>> {
    sqlx::query_as::<_, Shipment>(&format!(
        "SELECT {SHIPMENT_COLUMNS} FROM shipments WHERE trade_id = $1 ORDER BY id"
    ))
    .bind(trade_id)
    .fetch_all(pool)
    .await
}

pub async fn get_shipment(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Shipment>> {
    sqlx::query_as::<_, Shipment>(&format!(
        "SELECT {SHIPMENT_COLUMNS} FROM shipments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Configure « mon envoi » en une transaction : le format de MON colis
/// (tant que son étiquette n'existe pas et que MON paiement n'est pas
/// séquestré), le relais où JE recevrai l'autre colis, et le montant de mon
/// paiement (transport du format choisi). Retourne `false` si plus modifiable.
#[allow(clippy::too_many_arguments)]
pub async fn configure_my_shipping(
    pool: &PgPool,
    trade_id: Uuid,
    user_id: Uuid,
    format: &str,
    transport_cents: i32,
    relay_code: &str,
    relay_name: &str,
    relay_address: &str,
) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let payment_open: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM payments WHERE trade_id = $1 AND payer_id = $2 FOR UPDATE",
    )
    .bind(trade_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if !matches!(
        payment_open.as_ref().map(|(s,)| s.as_str()),
        Some("en_attente") | Some("echoue")
    ) {
        return Ok(false);
    }
    let my_parcel = sqlx::query(
        "UPDATE shipments SET format = $3, updated_at = now() \
         WHERE trade_id = $1 AND sender_id = $2 AND status = 'preparation'",
    )
    .bind(trade_id)
    .bind(user_id)
    .bind(format)
    .execute(&mut *tx)
    .await?;
    if my_parcel.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE shipments SET relay_code = $3, relay_name = $4, relay_address = $5, \
         updated_at = now() \
         WHERE trade_id = $1 AND recipient_id = $2 AND status = 'preparation'",
    )
    .bind(trade_id)
    .bind(user_id)
    .bind(relay_code)
    .bind(relay_name)
    .bind(relay_address)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE payments SET amount_cents = amount_cents - shipping_cents + $3, \
         shipping_cents = $3, updated_at = now() \
         WHERE trade_id = $1 AND payer_id = $2 AND status IN ('en_attente', 'echoue')",
    )
    .bind(trade_id)
    .bind(user_id)
    .bind(transport_cents)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Un colis prêt pour son étiquette : troc actif, paiement de l'expéditeur
/// séquestré, format et relais connus, pas encore d'étiquette.
pub async fn shipments_ready_for_label(
    pool: &PgPool,
    trade_id: Uuid,
) -> sqlx::Result<Vec<Shipment>> {
    sqlx::query_as::<_, Shipment>(&format!(
        "SELECT {SHIPMENT_COLUMNS} FROM shipments s \
         WHERE s.trade_id = $1 AND s.status = 'preparation' \
           AND s.format IS NOT NULL AND s.relay_code IS NOT NULL \
           AND EXISTS (SELECT 1 FROM trades t WHERE t.id = s.trade_id AND t.status = 'accepte') \
           AND EXISTS (SELECT 1 FROM payments p WHERE p.trade_id = s.trade_id \
                       AND p.payer_id = s.sender_id AND p.status = 'sequestre')"
    ))
    .bind(trade_id)
    .fetch_all(pool)
    .await
}

pub async fn mark_labeled(
    pool: &PgPool,
    shipment_id: Uuid,
    provider: &str,
    provider_ref: &str,
    drop_code: &str,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE shipments SET status = 'etiquette', provider = $2, provider_ref = $3, \
         drop_code = $4, label_generated_at = now(), updated_at = now() \
         WHERE id = $1 AND status = 'preparation'",
    )
    .bind(shipment_id)
    .bind(provider)
    .bind(provider_ref)
    .bind(drop_code)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Dépôt déclaré par l'expéditeur.
pub async fn mark_dropped(pool: &PgPool, shipment_id: Uuid, sender_id: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE shipments SET status = 'depose', dropped_at = now(), updated_at = now() \
         WHERE id = $1 AND sender_id = $2 AND status = 'etiquette'",
    )
    .bind(shipment_id)
    .bind(sender_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Avancée du suivi jusqu'à l'arrivée en relais (le simulateur y saute
/// directement ; le réel passera par `transit` via webhooks).
pub async fn mark_arrived(pool: &PgPool, shipment_id: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE shipments SET status = 'arrive', \
         in_transit_at = COALESCE(in_transit_at, now()), arrived_at = now(), \
         updated_at = now() \
         WHERE id = $1 AND status IN ('depose', 'transit')",
    )
    .bind(shipment_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Retrait déclaré par le destinataire — la fenêtre de 72 h démarre.
pub async fn mark_picked_up(
    pool: &PgPool,
    shipment_id: Uuid,
    recipient_id: Uuid,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE shipments SET status = 'retire', picked_up_at = now(), updated_at = now() \
         WHERE id = $1 AND recipient_id = $2 AND status = 'arrive'",
    )
    .bind(shipment_id)
    .bind(recipient_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Confirmation explicite (« tout est OK ») par le destinataire.
pub async fn confirm_shipment(
    pool: &PgPool,
    shipment_id: Uuid,
    recipient_id: Uuid,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE shipments SET status = 'confirme', confirmed_at = now(), updated_at = now() \
         WHERE id = $1 AND recipient_id = $2 AND status = 'retire'",
    )
    .bind(shipment_id)
    .bind(recipient_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Signalement d'un problème par le destinataire : le colis passe en
/// `incident` et le troc est gelé pour examen manuel (F5.2 fera mieux).
pub async fn report_issue(
    pool: &PgPool,
    shipment_id: Uuid,
    recipient_id: Uuid,
    reason: &str,
) -> sqlx::Result<Option<Uuid>> {
    let mut tx = pool.begin().await?;
    let trade_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE shipments SET status = 'incident', issue_reason = $3, \
         issue_reported_at = now(), updated_at = now() \
         WHERE id = $1 AND recipient_id = $2 AND status IN ('arrive', 'retire') \
         RETURNING trade_id",
    )
    .bind(shipment_id)
    .bind(recipient_id)
    .bind(reason)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(trade_id) = trade_id {
        sqlx::query(
            "UPDATE trades SET status = 'litige_gele' WHERE id = $1 AND status = 'accepte'",
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(trade_id)
}

/// Finalisation d'un troc envoi : les deux colis confirmés → troc
/// `finalise`, objets `troque`. Idempotent ; retourne `true` au passage.
pub async fn finalize_shipping_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let (confirmed,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM shipments WHERE trade_id = $1 AND status = 'confirme'",
    )
    .bind(trade_id)
    .fetch_one(&mut *tx)
    .await?;
    if confirmed < 2 {
        return Ok(false);
    }
    let proposal_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE trades SET status = 'finalise', finalized_at = now() \
         WHERE id = $1 AND status = 'accepte' RETURNING proposal_id",
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
    tx.commit().await?;
    Ok(true)
}

/// Annule les colis non terminaux d'un troc (troc annulé avant aboutissement).
pub async fn cancel_shipments_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE shipments SET status = 'annule', updated_at = now() \
         WHERE trade_id = $1 AND status IN ('preparation', 'etiquette')",
    )
    .bind(trade_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ————— Maintenance —————

/// Un expéditeur à relancer (colis toujours pas déposé).
#[derive(Debug, Clone, FromRow)]
pub struct DropReminder {
    pub shipment_id: Uuid,
    pub email: String,
    pub pseudo: String,
    pub other_pseudo: String,
    pub level: i32,
}

/// Rappels de dépôt J+2 puis J+4 (une seule fois chacun) — claim atomique.
pub async fn claim_drop_reminders(
    pool: &PgPool,
    days: i64,
    level: i32,
) -> sqlx::Result<Vec<DropReminder>> {
    sqlx::query_as::<_, DropReminder>(
        "WITH due AS ( \
            UPDATE shipments SET drop_reminders = $2, updated_at = now() \
            WHERE status IN ('preparation', 'etiquette') AND drop_reminders = $2 - 1 \
              AND created_at < now() - make_interval(days => $1::int) \
              AND trade_id IN (SELECT id FROM trades WHERE status = 'accepte') \
            RETURNING id, sender_id, recipient_id \
         ) \
         SELECT d.id AS shipment_id, u.email::text AS email, u.pseudo::text AS pseudo, \
                o.pseudo::text AS other_pseudo, $2::int AS level \
         FROM due d \
         JOIN users u ON u.id = d.sender_id \
         JOIN users o ON o.id = d.recipient_id",
    )
    .bind(days)
    .bind(level)
    .fetch_all(pool)
    .await
}

/// Les trocs envoi en échec de dépôt à J+5, avec le nombre de colis déposés.
pub async fn overdue_drop_trades(pool: &PgPool, days: i64) -> sqlx::Result<Vec<(Uuid, i64)>> {
    sqlx::query_as(
        "SELECT t.id, count(*) FILTER (WHERE s.status NOT IN ('preparation', 'etiquette')) \
         FROM trades t JOIN shipments s ON s.trade_id = t.id \
         WHERE t.status = 'accepte' AND t.delivery_mode = 'envoi' \
           AND t.created_at < now() - make_interval(days => $1::int) \
         GROUP BY t.id \
         HAVING count(*) FILTER (WHERE s.status IN ('preparation', 'etiquette')) > 0",
    )
    .bind(days)
    .fetch_all(pool)
    .await
}

/// L'expéditeur défaillant d'un troc (colis jamais déposé).
pub async fn undropped_sender(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT sender_id FROM shipments \
         WHERE trade_id = $1 AND status IN ('preparation', 'etiquette') LIMIT 1",
    )
    .bind(trade_id)
    .fetch_optional(pool)
    .await
}

/// Annule un troc envoi (aucun colis déposé à J+5) : troc `annule`, objets
/// libérés, colis annulés — atomique, idempotent.
pub async fn cancel_shipping_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let proposal_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE trades SET status = 'annule', cancelled_at = now() \
         WHERE id = $1 AND status = 'accepte' RETURNING proposal_id",
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
         WHERE trade_id = $1 AND status IN ('preparation', 'etiquette')",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Gèle un troc envoi (litige) et annule ses colis en attente — idempotent.
pub async fn freeze_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE trades SET status = 'litige_gele' WHERE id = $1 AND status = 'accepte'",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE shipments SET status = 'annule', updated_at = now() \
         WHERE trade_id = $1 AND status IN ('preparation', 'etiquette')",
    )
    .bind(trade_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Auto-confirmation : colis retirés il y a plus de `hours` heures sans
/// signalement — claim atomique, retourne les trocs touchés.
pub async fn claim_auto_confirmations(pool: &PgPool, hours: i64) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar(
        "UPDATE shipments SET status = 'confirme', confirmed_at = now(), updated_at = now() \
         WHERE status = 'retire' AND picked_up_at < now() - make_interval(hours => $1::int) \
           AND NOT EXISTS ( \
               SELECT 1 FROM disputes d \
               WHERE d.trade_id = shipments.trade_id AND d.status <> 'tranche') \
         RETURNING trade_id",
    )
    .bind(hours)
    .fetch_all(pool)
    .await
}

/// Filet J+21 : trocs envoi toujours ouverts — à geler pour examen.
pub async fn stale_shipping_trades(pool: &PgPool, days: i64) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar(
        "SELECT id FROM trades WHERE status = 'accepte' AND delivery_mode = 'envoi' \
         AND created_at < now() - make_interval(days => $1::int)",
    )
    .bind(days)
    .fetch_all(pool)
    .await
}

/// Journalise une défaillance pour F5.2 (rien n'est sanctionné aujourd'hui).
pub async fn record_dispute_event(
    pool: &PgPool,
    trade_id: Uuid,
    event_type: &str,
    culprit_id: Option<Uuid>,
    details: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO dispute_events (trade_id, event_type, culprit_id, details) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(trade_id)
    .bind(event_type)
    .bind(culprit_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}
