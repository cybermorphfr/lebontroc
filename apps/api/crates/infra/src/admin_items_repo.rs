//! Administration des annonces et vue par utilisateur : ce qu'un
//! modérateur doit pouvoir voir et faire sans ouvrir un client SQL.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Une annonce vue par l'administration : l'objet, son propriétaire et sa
/// charge de signalements.
#[derive(Debug, Clone, FromRow)]
pub struct AdminItem {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub value_cents: i32,
    pub category: String,
    pub condition: String,
    pub owner_id: Uuid,
    pub owner_pseudo: String,
    pub owner_banned: bool,
    pub photo_key: Option<String>,
    pub signalements: i64,
    pub signalements_ouverts: i64,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Filtres de la file d'annonces. `owner` est un pseudo exact,
/// `q` une sous-chaîne du titre.
#[derive(Debug, Default, Clone)]
pub struct ItemFilters {
    pub q: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    /// Ne remonter que les annonces ayant au moins un signalement ouvert.
    pub signalees: bool,
    pub limit: i64,
}

pub async fn list_items(pool: &PgPool, f: &ItemFilters) -> sqlx::Result<Vec<AdminItem>> {
    sqlx::query_as::<_, AdminItem>(
        "SELECT i.id, i.title, i.status, i.value_cents, c.label AS category, \
                i.condition, i.owner_id, u.pseudo::text AS owner_pseudo, \
                (u.banned_at IS NOT NULL) AS owner_banned, \
                (SELECT p.s3_key FROM item_photos p WHERE p.item_id = i.id \
                 ORDER BY p.position LIMIT 1) AS photo_key, \
                (SELECT count(*) FROM reports r \
                 WHERE r.target_type = 'objet' AND r.target_id = i.id) AS signalements, \
                (SELECT count(*) FROM reports r \
                 WHERE r.target_type = 'objet' AND r.target_id = i.id \
                   AND r.status = 'nouveau') AS signalements_ouverts, \
                i.created_at, i.deleted_at \
         FROM items i \
         JOIN users u ON u.id = i.owner_id \
         JOIN categories c ON c.id = i.category_id \
         WHERE ($1::text IS NULL OR i.title ILIKE '%' || $1 || '%') \
           AND ($2::text IS NULL OR i.status = $2) \
           AND ($3::text IS NULL OR u.pseudo::text = $3) \
           AND (NOT $4::bool OR EXISTS (SELECT 1 FROM reports r \
                WHERE r.target_type = 'objet' AND r.target_id = i.id \
                  AND r.status = 'nouveau')) \
         ORDER BY i.created_at DESC LIMIT $5",
    )
    .bind(f.q.as_deref())
    .bind(f.status.as_deref())
    .bind(f.owner.as_deref())
    .bind(f.signalees)
    .bind(f.limit)
    .fetch_all(pool)
    .await
}

/// L'arborescence : combien d'annonces par membre, et dans quel état.
#[derive(Debug, Clone, FromRow)]
pub struct OwnerBranch {
    pub pseudo: String,
    pub role: String,
    pub banned: bool,
    pub total: i64,
    pub disponibles: i64,
    pub masquees: i64,
    pub signalees: i64,
    pub derniere_publication: Option<DateTime<Utc>>,
}

pub async fn owners_tree(pool: &PgPool) -> sqlx::Result<Vec<OwnerBranch>> {
    sqlx::query_as::<_, OwnerBranch>(
        "SELECT u.pseudo::text AS pseudo, u.role, (u.banned_at IS NOT NULL) AS banned, \
                count(i.id) AS total, \
                count(i.id) FILTER (WHERE i.status = 'disponible') AS disponibles, \
                count(i.id) FILTER (WHERE i.status = 'masque') AS masquees, \
                count(DISTINCT r.id) AS signalees, \
                max(i.created_at) AS derniere_publication \
         FROM users u \
         JOIN items i ON i.owner_id = u.id AND i.deleted_at IS NULL \
         LEFT JOIN reports r ON r.target_type = 'objet' AND r.target_id = i.id \
                            AND r.status = 'nouveau' \
         WHERE u.deleted_at IS NULL \
         GROUP BY u.pseudo, u.role, u.banned_at \
         ORDER BY count(DISTINCT r.id) DESC, count(i.id) DESC",
    )
    .fetch_all(pool)
    .await
}

/// Masque ou remet en ligne une annonce, quel qu'en soit le propriétaire.
/// Renvoie le titre et l'identifiant du propriétaire pour la notification.
pub async fn moderate_item(
    pool: &PgPool,
    item_id: Uuid,
    masquer: bool,
) -> sqlx::Result<Option<(String, Uuid)>> {
    let statut = if masquer { "masque" } else { "disponible" };
    sqlx::query_as::<_, (String, Uuid)>(
        "UPDATE items SET status = $2, updated_at = now() \
         WHERE id = $1 AND deleted_at IS NULL \
           AND status IN ('disponible', 'masque') \
         RETURNING title, owner_id",
    )
    .bind(item_id)
    .bind(statut)
    .fetch_optional(pool)
    .await
}

// ————— Fiche d'activité d'un membre —————

#[derive(Debug, Clone, FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub pseudo: String,
    pub email: String,
    pub postal_code: String,
    pub commune: Option<String>,
    pub role: String,
    pub is_master: bool,
    pub email_verified: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub restricted_until: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub totp_actif: bool,
    pub created_at: DateTime<Utc>,
    pub derniere_activite: Option<DateTime<Utc>>,
}

pub async fn user_profile(pool: &PgPool, pseudo: &str) -> sqlx::Result<Option<UserProfile>> {
    sqlx::query_as::<_, UserProfile>(
        "SELECT u.id, u.pseudo::text AS pseudo, u.email::text AS email, u.postal_code, \
                c.nom AS commune, u.role, u.is_master, \
                (u.email_verified_at IS NOT NULL) AS email_verified, \
                u.banned_at, u.restricted_until, u.deleted_at, \
                (u.totp_enabled_at IS NOT NULL) AS totp_actif, u.created_at, \
                (SELECT max(s.created_at) FROM sessions s WHERE s.user_id = u.id) \
                    AS derniere_activite \
         FROM users u LEFT JOIN communes c ON c.code_postal = u.postal_code \
         WHERE u.pseudo = $1::citext",
    )
    .bind(pseudo)
    .fetch_optional(pool)
    .await
}

/// Les compteurs d'activité — une ligne, une requête.
#[derive(Debug, Clone, FromRow)]
pub struct UserCounters {
    pub annonces: i64,
    pub annonces_masquees: i64,
    pub propositions_envoyees: i64,
    pub propositions_recues: i64,
    pub trocs: i64,
    pub trocs_finalises: i64,
    pub trocs_annules: i64,
    pub messages: i64,
    pub litiges_ouverts_par_lui: i64,
    pub litiges_subis: i64,
    pub signalements_emis: i64,
    pub signalements_recus: i64,
    pub signalements_fondes: i64,
    pub note_moyenne: Option<f64>,
    pub evaluations: i64,
    pub favoris: i64,
    pub blocages_subis: i64,
}

pub async fn user_counters(pool: &PgPool, user_id: Uuid) -> sqlx::Result<UserCounters> {
    sqlx::query_as::<_, UserCounters>(
        "SELECT \
            (SELECT count(*) FROM items WHERE owner_id = $1 AND deleted_at IS NULL) AS annonces, \
            (SELECT count(*) FROM items WHERE owner_id = $1 AND status = 'masque' \
             AND deleted_at IS NULL) AS annonces_masquees, \
            (SELECT count(*) FROM proposals WHERE proposer_id = $1) AS propositions_envoyees, \
            (SELECT count(*) FROM proposals WHERE recipient_id = $1) AS propositions_recues, \
            (SELECT count(*) FROM trades WHERE proposer_id = $1 OR recipient_id = $1) AS trocs, \
            (SELECT count(*) FROM trades WHERE (proposer_id = $1 OR recipient_id = $1) \
             AND status = 'finalise') AS trocs_finalises, \
            (SELECT count(*) FROM trades WHERE (proposer_id = $1 OR recipient_id = $1) \
             AND status = 'annule') AS trocs_annules, \
            (SELECT count(*) FROM messages WHERE sender_id = $1) AS messages, \
            (SELECT count(*) FROM disputes WHERE opened_by = $1) AS litiges_ouverts_par_lui, \
            (SELECT count(*) FROM disputes d JOIN trades t ON t.id = d.trade_id \
             WHERE (t.proposer_id = $1 OR t.recipient_id = $1) AND d.opened_by <> $1) \
                AS litiges_subis, \
            (SELECT count(*) FROM reports WHERE reporter_id = $1) AS signalements_emis, \
            (SELECT count(*) FROM reports WHERE target_type = 'utilisateur' AND target_id = $1) \
                AS signalements_recus, \
            (SELECT count(*) FROM reports WHERE target_type = 'utilisateur' AND target_id = $1 \
             AND outcome = 'fonde') AS signalements_fondes, \
            (SELECT avg(rating)::float8 FROM reviews WHERE reviewee_id = $1 \
             AND published_at IS NOT NULL) AS note_moyenne, \
            (SELECT count(*) FROM reviews WHERE reviewee_id = $1 AND published_at IS NOT NULL) \
                AS evaluations, \
            (SELECT count(*) FROM favorites WHERE user_id = $1) AS favoris, \
            (SELECT count(*) FROM user_blocks WHERE blocked_id = $1) AS blocages_subis",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Les trocs du membre, du plus récent au plus ancien.
#[derive(Debug, Clone, FromRow)]
pub struct UserTrade {
    pub id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub role: String,
    pub autre_pseudo: String,
    pub cash_cents: i32,
    pub litige: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn user_trades(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<UserTrade>> {
    sqlx::query_as::<_, UserTrade>(
        "SELECT t.id, t.status, t.delivery_mode, \
                CASE WHEN t.proposer_id = $1 THEN 'proposant' ELSE 'destinataire' END AS role, \
                CASE WHEN t.proposer_id = $1 THEN r.pseudo::text ELSE p.pseudo::text END \
                    AS autre_pseudo, \
                t.cash_cents, \
                EXISTS (SELECT 1 FROM disputes d WHERE d.trade_id = t.id) AS litige, \
                t.created_at \
         FROM trades t \
         JOIN users p ON p.id = t.proposer_id \
         JOIN users r ON r.id = t.recipient_id \
         WHERE t.proposer_id = $1 OR t.recipient_id = $1 \
         ORDER BY t.created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Les signalements visant ce membre (profil ou objets), et ceux qu'il a
/// émis — la réputation dans les deux sens.
#[derive(Debug, Clone, FromRow)]
pub struct UserReport {
    pub id: Uuid,
    pub sens: String,
    pub target_type: String,
    pub cible: Option<String>,
    pub autre_pseudo: Option<String>,
    pub reason: String,
    pub comment: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn user_reports(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<UserReport>> {
    sqlx::query_as::<_, UserReport>(
        "SELECT r.id, 'recu' AS sens, r.target_type, \
                CASE WHEN r.target_type = 'objet' \
                     THEN (SELECT title FROM items WHERE id = r.target_id) END AS cible, \
                (SELECT pseudo::text FROM users WHERE id = r.reporter_id) AS autre_pseudo, \
                r.reason, r.comment, r.status, r.outcome, r.created_at \
         FROM reports r \
         WHERE (r.target_type = 'utilisateur' AND r.target_id = $1) \
            OR (r.target_type = 'objet' \
                AND r.target_id IN (SELECT id FROM items WHERE owner_id = $1)) \
         UNION ALL \
         SELECT r.id, 'emis' AS sens, r.target_type, \
                CASE WHEN r.target_type = 'objet' \
                     THEN (SELECT title FROM items WHERE id = r.target_id) END AS cible, \
                CASE WHEN r.target_type = 'utilisateur' \
                     THEN (SELECT pseudo::text FROM users WHERE id = r.target_id) \
                     WHEN r.target_type = 'objet' \
                     THEN (SELECT u.pseudo::text FROM items i JOIN users u ON u.id = i.owner_id \
                           WHERE i.id = r.target_id) END AS autre_pseudo, \
                r.reason, r.comment, r.status, r.outcome, r.created_at \
         FROM reports r WHERE r.reporter_id = $1 \
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// L'historique de modération : ce que l'équipe a fait à ce membre.
#[derive(Debug, Clone, FromRow)]
pub struct UserSanction {
    pub event_type: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn user_sanctions(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<UserSanction>> {
    sqlx::query_as::<_, UserSanction>(
        "SELECT event_type, details, created_at FROM dispute_events \
         WHERE culprit_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Autocomplétion : les pseudos qui commencent par — puis qui contiennent.
#[derive(Debug, Clone, FromRow)]
pub struct UserSuggestion {
    pub pseudo: String,
    pub role: String,
    pub is_master: bool,
    pub annonces: i64,
}

pub async fn suggest_users(pool: &PgPool, q: &str) -> sqlx::Result<Vec<UserSuggestion>> {
    sqlx::query_as::<_, UserSuggestion>(
        "SELECT u.pseudo::text AS pseudo, u.role, u.is_master, \
                (SELECT count(*) FROM items i WHERE i.owner_id = u.id \
                 AND i.deleted_at IS NULL) AS annonces \
         FROM users u \
         WHERE u.deleted_at IS NULL \
           AND (u.pseudo::text ILIKE $1 || '%' OR u.pseudo::text ILIKE '%' || $1 || '%' \
                OR u.email::text ILIKE $1 || '%') \
         ORDER BY (u.pseudo::text ILIKE $1 || '%') DESC, u.pseudo LIMIT 10",
    )
    .bind(q)
    .fetch_all(pool)
    .await
}
