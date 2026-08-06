//! Le moteur de métriques du tableau de bord d'administration : tout ce
//! qui se calcule honnêtement depuis les données réelles. Les indicateurs
//! qui exigent le PSP réel (trésorerie, portefeuilles) ou des sources
//! absentes (marketing, apps mobiles) n'ont pas leur place ici.

use chrono::NaiveDate;
use sqlx::{FromRow, PgPool};

/// Un point de série quotidienne (30 derniers jours).
#[derive(Debug, Clone, FromRow)]
pub struct DailyPoint {
    pub jour: NaiveDate,
    pub inscriptions: i64,
    pub annonces: i64,
    pub propositions: i64,
    pub trocs_finalises: i64,
    pub volume_soulte_cents: i64,
}

pub async fn daily_series(pool: &PgPool) -> sqlx::Result<Vec<DailyPoint>> {
    sqlx::query_as::<_, DailyPoint>(
        "WITH jours AS ( \
            SELECT generate_series(current_date - 29, current_date, '1 day')::date AS jour \
         ) \
         SELECT j.jour, \
            (SELECT count(*) FROM users u WHERE u.created_at::date = j.jour) AS inscriptions, \
            (SELECT count(*) FROM items i WHERE i.created_at::date = j.jour) AS annonces, \
            (SELECT count(*) FROM proposals p WHERE p.created_at::date = j.jour) AS propositions, \
            (SELECT count(*) FROM trades t WHERE t.finalized_at::date = j.jour) AS trocs_finalises, \
            (SELECT COALESCE(sum(t.cash_cents), 0) FROM trades t \
             WHERE t.finalized_at::date = j.jour)::bigint AS volume_soulte_cents \
         FROM jours j ORDER BY j.jour",
    )
    .fetch_all(pool)
    .await
}

/// Activité et engagement — depuis la télémétrie pseudonymisée.
#[derive(Debug, Clone, FromRow)]
pub struct Activite {
    pub inscrits_total: i64,
    pub comptes_supprimes: i64,
    pub comptes_bannis: i64,
    pub comptes_restreints: i64,
    pub dau: i64,
    pub wau: i64,
    pub mau: i64,
    pub recherches_7j: i64,
    pub messages_7j: i64,
    pub favoris_total: i64,
    pub notifications_ouvertes_7j: i64,
}

pub async fn activite(pool: &PgPool) -> sqlx::Result<Activite> {
    sqlx::query_as::<_, Activite>(
        "SELECT \
            (SELECT count(*) FROM users WHERE deleted_at IS NULL) AS inscrits_total, \
            (SELECT count(*) FROM users WHERE deleted_at IS NOT NULL) AS comptes_supprimes, \
            (SELECT count(*) FROM users WHERE banned_at IS NOT NULL AND deleted_at IS NULL) AS comptes_bannis, \
            (SELECT count(*) FROM users WHERE restricted_until > now()) AS comptes_restreints, \
            (SELECT count(DISTINCT user_id_hash) FROM analytics_events \
             WHERE user_id_hash IS NOT NULL AND occurred_at > now() - interval '1 day') AS dau, \
            (SELECT count(DISTINCT user_id_hash) FROM analytics_events \
             WHERE user_id_hash IS NOT NULL AND occurred_at > now() - interval '7 days') AS wau, \
            (SELECT count(DISTINCT user_id_hash) FROM analytics_events \
             WHERE user_id_hash IS NOT NULL AND occurred_at > now() - interval '30 days') AS mau, \
            (SELECT count(*) FROM analytics_events WHERE name = 'search_performed' \
             AND occurred_at > now() - interval '7 days') AS recherches_7j, \
            (SELECT count(*) FROM analytics_events WHERE name = 'message_sent' \
             AND occurred_at > now() - interval '7 days') AS messages_7j, \
            (SELECT count(*) FROM favorites) AS favoris_total, \
            (SELECT count(*) FROM analytics_events WHERE name = 'notification_opened' \
             AND occurred_at > now() - interval '7 days') AS notifications_ouvertes_7j",
    )
    .fetch_one(pool)
    .await
}

/// Marketplace : catalogue, négociation, performance.
#[derive(Debug, Clone, FromRow)]
pub struct Marketplace {
    pub annonces_actives: i64,
    pub annonces_reservees: i64,
    pub annonces_troquees: i64,
    pub propositions_total: i64,
    pub contre_propositions: i64,
    pub taux_acceptation_pct: f64,
    pub heures_moyennes_avant_accord: Option<f64>,
    pub contre_avant_accord: f64,
    pub valeur_echangee_cents: i64,
    pub heures_avant_premier_message: Option<f64>,
}

pub async fn marketplace(pool: &PgPool) -> sqlx::Result<Marketplace> {
    sqlx::query_as::<_, Marketplace>(
        "SELECT \
            (SELECT count(*) FROM items WHERE status = 'disponible' AND deleted_at IS NULL) AS annonces_actives, \
            (SELECT count(*) FROM items WHERE status = 'reserve') AS annonces_reservees, \
            (SELECT count(*) FROM items WHERE status = 'troque') AS annonces_troquees, \
            (SELECT count(*) FROM proposals) AS propositions_total, \
            (SELECT count(*) FROM proposals WHERE counter_of IS NOT NULL) AS contre_propositions, \
            (SELECT COALESCE( \
                100.0 * count(*) FILTER (WHERE status = 'acceptee') \
                / NULLIF(count(*) FILTER (WHERE status IN ('acceptee','refusee','expiree')), 0), 0 \
             )::float8 FROM proposals) AS taux_acceptation_pct, \
            (SELECT avg(EXTRACT(EPOCH FROM t.created_at - racine.created_at) / 3600)::float8 \
             FROM trades t \
             JOIN proposals p ON p.id = t.proposal_id \
             LEFT JOIN LATERAL ( \
                WITH RECURSIVE remontee AS ( \
                    SELECT id, counter_of, created_at FROM proposals WHERE id = p.id \
                    UNION ALL \
                    SELECT pa.id, pa.counter_of, pa.created_at \
                    FROM proposals pa JOIN remontee r ON r.counter_of = pa.id \
                ) SELECT min(created_at) AS created_at FROM remontee \
             ) racine ON true) AS heures_moyennes_avant_accord, \
            (SELECT COALESCE(avg(nb), 0)::float8 FROM ( \
                SELECT count(*) FILTER (WHERE counter_of IS NOT NULL) AS nb \
                FROM proposals GROUP BY COALESCE(counter_of, id)) chaines \
             ) AS contre_avant_accord, \
            (SELECT COALESCE(sum(pi.value_cents_snapshot), 0) FROM proposal_items pi \
             JOIN trades t ON t.proposal_id = pi.proposal_id \
             WHERE t.status = 'finalise')::bigint AS valeur_echangee_cents, \
            (SELECT avg(EXTRACT(EPOCH FROM m.premier - p.created_at) / 3600)::float8 \
             FROM proposals p JOIN LATERAL ( \
                SELECT min(created_at) AS premier FROM messages WHERE proposal_id = p.id \
             ) m ON m.premier IS NOT NULL) AS heures_avant_premier_message",
    )
    .fetch_one(pool)
    .await
}

/// Un top (catégorie, objet…) : libellé + compteur.
#[derive(Debug, Clone, FromRow)]
pub struct TopEntry {
    pub libelle: String,
    pub total: i64,
}

pub async fn top_categories(pool: &PgPool) -> sqlx::Result<Vec<TopEntry>> {
    sqlx::query_as::<_, TopEntry>(
        "SELECT c.label AS libelle, count(*) AS total \
         FROM items i \
         JOIN categories c0 ON c0.id = i.category_id \
         JOIN categories c ON c.id = COALESCE( \
            (SELECT parent_id FROM categories WHERE id = c0.parent_id), \
            c0.parent_id, c0.id) \
         WHERE i.deleted_at IS NULL \
         GROUP BY c.label ORDER BY total DESC LIMIT 5",
    )
    .fetch_all(pool)
    .await
}

pub async fn top_communes(pool: &PgPool) -> sqlx::Result<Vec<TopEntry>> {
    sqlx::query_as::<_, TopEntry>(
        "SELECT COALESCE(c.nom, 'CP ' || left(u.postal_code, 2) || 'xxx') AS libelle, \
                count(*) AS total \
         FROM users u LEFT JOIN communes c ON c.code_postal = u.postal_code \
         WHERE u.deleted_at IS NULL \
         GROUP BY 1 ORDER BY total DESC LIMIT 5",
    )
    .fetch_all(pool)
    .await
}

/// Qualité : litiges, signalements, sanctions.
#[derive(Debug, Clone, FromRow)]
pub struct Qualite {
    pub litiges_ouverts: i64,
    pub litiges_en_examen: i64,
    pub litiges_tranches: i64,
    pub heures_moyennes_resolution: Option<f64>,
    pub signalements_en_attente: i64,
    pub note_moyenne: Option<f64>,
}

pub async fn qualite(pool: &PgPool) -> sqlx::Result<Qualite> {
    sqlx::query_as::<_, Qualite>(
        "SELECT \
            (SELECT count(*) FROM disputes WHERE status = 'ouvert') AS litiges_ouverts, \
            (SELECT count(*) FROM disputes WHERE status = 'en_examen') AS litiges_en_examen, \
            (SELECT count(*) FROM disputes WHERE status = 'tranche') AS litiges_tranches, \
            (SELECT avg(EXTRACT(EPOCH FROM resolved_at - opened_at) / 3600)::float8 \
             FROM disputes WHERE resolved_at IS NOT NULL) AS heures_moyennes_resolution, \
            (SELECT count(*) FROM reports WHERE status = 'nouveau') AS signalements_en_attente, \
            (SELECT avg(rating)::float8 FROM reviews WHERE published_at IS NOT NULL) AS note_moyenne",
    )
    .fetch_one(pool)
    .await
}

/// Finances de la bêta — paiements SIMULÉS : des ordres de grandeur de
/// parcours, pas de la trésorerie.
#[derive(Debug, Clone, FromRow)]
pub struct FinancesBeta {
    pub soultes_capturees_cents: i64,
    pub soultes_sequestrees_cents: i64,
    pub frais_service_percus_cents: i64,
    pub transport_encaisse_cents: i64,
    pub commissions_cents: i64,
    pub paiements_echoues: i64,
    pub jours_moyens_finalisation: Option<f64>,
    pub colis_expedies: i64,
    pub trocs_envoi_litigieux: i64,
}

pub async fn finances_beta(pool: &PgPool) -> sqlx::Result<FinancesBeta> {
    sqlx::query_as::<_, FinancesBeta>(
        "SELECT \
            (SELECT COALESCE(sum(amount_cents - service_cents - shipping_cents), 0) \
             FROM payments WHERE status = 'capture')::bigint AS soultes_capturees_cents, \
            (SELECT COALESCE(sum(amount_cents - service_cents - shipping_cents), 0) \
             FROM payments WHERE status = 'sequestre')::bigint AS soultes_sequestrees_cents, \
            (SELECT COALESCE(sum(service_cents), 0) FROM payments WHERE status = 'capture')::bigint \
                AS frais_service_percus_cents, \
            (SELECT COALESCE(sum(shipping_cents), 0) FROM payments WHERE status = 'capture')::bigint \
                AS transport_encaisse_cents, \
            (SELECT COALESCE(sum(fees_cents), 0) FROM payments WHERE status = 'capture')::bigint \
                AS commissions_cents, \
            (SELECT count(*) FROM payments WHERE status IN ('echoue', 'expire')) AS paiements_echoues, \
            (SELECT avg(EXTRACT(EPOCH FROM finalized_at - created_at) / 86400)::float8 \
             FROM trades WHERE finalized_at IS NOT NULL) AS jours_moyens_finalisation, \
            (SELECT count(*) FROM shipments WHERE dropped_at IS NOT NULL) AS colis_expedies, \
            (SELECT count(DISTINCT trade_id) FROM disputes d \
             JOIN trades t ON t.id = d.trade_id WHERE t.delivery_mode = 'envoi') \
                AS trocs_envoi_litigieux",
    )
    .fetch_one(pool)
    .await
}

/// Système : ce que la base sait dire d'elle-même.
#[derive(Debug, Clone, FromRow)]
pub struct Systeme {
    pub taille_base: String,
    pub evenements_telemetrie: i64,
    pub evenements_non_exportes: i64,
    pub notifications_stockees: i64,
    pub sessions_actives: i64,
}

pub async fn systeme(pool: &PgPool) -> sqlx::Result<Systeme> {
    sqlx::query_as::<_, Systeme>(
        "SELECT \
            pg_size_pretty(pg_database_size(current_database())) AS taille_base, \
            (SELECT count(*) FROM analytics_events) AS evenements_telemetrie, \
            (SELECT count(*) FROM analytics_events WHERE exported_at IS NULL) \
                AS evenements_non_exportes, \
            (SELECT count(*) FROM notifications) AS notifications_stockees, \
            (SELECT count(*) FROM sessions) AS sessions_actives",
    )
    .fetch_one(pool)
    .await
}

/// Comparaison 7 derniers jours vs 7 précédents — la matière des alertes.
#[derive(Debug, Clone, FromRow)]
pub struct Tendance {
    pub litiges_7j: i64,
    pub litiges_7j_precedents: i64,
    pub trocs_7j: i64,
    pub trocs_7j_precedents: i64,
    pub echecs_paiement_7j: i64,
}

pub async fn tendance(pool: &PgPool) -> sqlx::Result<Tendance> {
    sqlx::query_as::<_, Tendance>(
        "SELECT \
            (SELECT count(*) FROM disputes WHERE opened_at > now() - interval '7 days') AS litiges_7j, \
            (SELECT count(*) FROM disputes WHERE opened_at BETWEEN now() - interval '14 days' \
             AND now() - interval '7 days') AS litiges_7j_precedents, \
            (SELECT count(*) FROM trades WHERE created_at > now() - interval '7 days') AS trocs_7j, \
            (SELECT count(*) FROM trades WHERE created_at BETWEEN now() - interval '14 days' \
             AND now() - interval '7 days') AS trocs_7j_precedents, \
            (SELECT count(*) FROM payments WHERE status IN ('echoue', 'expire') \
             AND created_at > now() - interval '7 days') AS echecs_paiement_7j",
    )
    .fetch_one(pool)
    .await
}
