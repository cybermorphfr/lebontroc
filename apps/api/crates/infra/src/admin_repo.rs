//! Requêtes du back-office (F6.1) : recherche transverse, file des
//! signalements, journal d'audit — et RGPD (F6.3) : export, anonymisation.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ————— Journal d'audit (immuable : INSERT only) —————

pub async fn record_audit(
    pool: &PgPool,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO admin_audit (actor_id, action, target_type, target_id, details) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    /// Pseudo de l'auteur — « service » quand l'action vient de la clé
    /// d'exploitation, `None` pour les tâches automatiques.
    pub actor_pseudo: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn list_audit(pool: &PgPool) -> sqlx::Result<Vec<AuditEntry>> {
    sqlx::query_as::<_, AuditEntry>(
        "SELECT a.id, u.pseudo::text AS actor_pseudo, a.action, a.target_type, \
                a.target_id, a.details, a.created_at \
         FROM admin_audit a LEFT JOIN users u ON u.id = a.actor_id \
         ORDER BY a.id DESC LIMIT 200",
    )
    .fetch_all(pool)
    .await
}

// ————— File des signalements —————

#[derive(Debug, Clone, FromRow)]
pub struct ReportRow {
    pub id: Uuid,
    pub reporter_pseudo: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub comment: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn list_reports(pool: &PgPool, status: Option<&str>) -> sqlx::Result<Vec<ReportRow>> {
    sqlx::query_as::<_, ReportRow>(
        "SELECT r.id, u.pseudo::text AS reporter_pseudo, r.target_type, r.target_id, \
                r.reason, r.comment, r.status, r.outcome, r.created_at \
         FROM reports r JOIN users u ON u.id = r.reporter_id \
         WHERE ($1::text IS NULL OR r.status = $1) \
         ORDER BY r.created_at DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(pool)
    .await
}

/// Clôt un signalement — retourne false s'il était déjà traité.
pub async fn close_report(
    pool: &PgPool,
    id: Uuid,
    outcome: &str,
) -> sqlx::Result<Option<ReportRow>> {
    sqlx::query_as::<_, ReportRow>(
        "WITH closed AS ( \
            UPDATE reports SET status = 'traite', outcome = $2, resolved_at = now() \
            WHERE id = $1 AND status = 'nouveau' RETURNING * \
         ) \
         SELECT c.id, u.pseudo::text AS reporter_pseudo, c.target_type, c.target_id, \
                c.reason, c.comment, c.status, c.outcome, c.created_at \
         FROM closed c JOIN users u ON u.id = c.reporter_id",
    )
    .bind(id)
    .bind(outcome)
    .fetch_optional(pool)
    .await
}

// ————— Recherche transverse —————

#[derive(Debug, Clone, FromRow)]
pub struct AdminUserHit {
    pub id: Uuid,
    pub pseudo: String,
    pub role: String,
    pub is_master: bool,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub restricted_until: Option<DateTime<Utc>>,
    pub banned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AdminItemHit {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub owner_pseudo: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AdminTradeHit {
    pub id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    pub created_at: DateTime<Utc>,
}

pub async fn search_users(pool: &PgPool, q: &str) -> sqlx::Result<Vec<AdminUserHit>> {
    sqlx::query_as::<_, AdminUserHit>(
        "SELECT id, pseudo::text AS pseudo, role, is_master, email::text AS email, \
                created_at, restricted_until, banned_at \
         FROM users WHERE pseudo::text ILIKE '%' || $1 || '%' \
            OR email::text ILIKE '%' || $1 || '%' \
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(q)
    .fetch_all(pool)
    .await
}

pub async fn search_items(pool: &PgPool, q: &str) -> sqlx::Result<Vec<AdminItemHit>> {
    sqlx::query_as::<_, AdminItemHit>(
        "SELECT i.id, i.title, i.status, u.pseudo::text AS owner_pseudo \
         FROM items i JOIN users u ON u.id = i.owner_id \
         WHERE i.title ILIKE '%' || $1 || '%' AND i.deleted_at IS NULL \
         ORDER BY i.created_at DESC LIMIT 20",
    )
    .bind(q)
    .fetch_all(pool)
    .await
}

/// Les trocs d'un utilisateur trouvé par pseudo (ou un troc par UUID exact).
pub async fn search_trades(pool: &PgPool, q: &str) -> sqlx::Result<Vec<AdminTradeHit>> {
    sqlx::query_as::<_, AdminTradeHit>(
        "SELECT t.id, t.status, t.delivery_mode, \
                p.pseudo::text AS proposer_pseudo, r.pseudo::text AS recipient_pseudo, \
                t.created_at \
         FROM trades t \
         JOIN users p ON p.id = t.proposer_id \
         JOIN users r ON r.id = t.recipient_id \
         WHERE t.id::text = $1 \
            OR p.pseudo::text ILIKE '%' || $1 || '%' \
            OR r.pseudo::text ILIKE '%' || $1 || '%' \
         ORDER BY t.created_at DESC LIMIT 20",
    )
    .bind(q)
    .fetch_all(pool)
    .await
}

/// Événement de score hors troc (signalement fondé) — trade_id NULL.
pub async fn record_scoring_event(
    pool: &PgPool,
    culprit_id: Uuid,
    event_type: &str,
    details: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO dispute_events (trade_id, event_type, culprit_id, details) \
         VALUES (NULL, $1, $2, $3)",
    )
    .bind(event_type)
    .bind(culprit_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

// ————— RGPD (F6.3) —————

/// Export brut de toutes les données d'un utilisateur (JSON agrégé en SQL).
pub async fn export_user_data(pool: &PgPool, user_id: Uuid) -> sqlx::Result<serde_json::Value> {
    let (data,): (serde_json::Value,) = sqlx::query_as(
        "SELECT jsonb_build_object( \
            'profil', (SELECT to_jsonb(u) - 'password_hash' FROM users u WHERE id = $1), \
            'objets', (SELECT COALESCE(jsonb_agg(to_jsonb(i)), '[]'::jsonb) \
                       FROM items i WHERE owner_id = $1), \
            'propositions', (SELECT COALESCE(jsonb_agg(to_jsonb(p)), '[]'::jsonb) \
                       FROM proposals p WHERE proposer_id = $1 OR recipient_id = $1), \
            'trocs', (SELECT COALESCE(jsonb_agg(to_jsonb(t) - 'proposer_code' - 'recipient_code'), '[]'::jsonb) \
                       FROM trades t WHERE proposer_id = $1 OR recipient_id = $1), \
            'messages', (SELECT COALESCE(jsonb_agg(to_jsonb(m)), '[]'::jsonb) \
                       FROM messages m WHERE sender_id = $1), \
            'evaluations', (SELECT COALESCE(jsonb_agg(to_jsonb(r)), '[]'::jsonb) \
                       FROM reviews r WHERE reviewer_id = $1 OR reviewee_id = $1), \
            'paiements', (SELECT COALESCE(jsonb_agg(jsonb_build_object( \
                            'trade_id', trade_id, 'amount_cents', amount_cents, \
                            'status', status, 'created_at', created_at)), '[]'::jsonb) \
                       FROM payments WHERE payer_id = $1), \
            'favoris', (SELECT COALESCE(jsonb_agg(item_id), '[]'::jsonb) \
                       FROM favorites WHERE user_id = $1), \
            'notifications', (SELECT COALESCE(jsonb_agg(to_jsonb(n)), '[]'::jsonb) \
                       FROM notifications n WHERE user_id = $1) \
         )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(data)
}

/// Suppression de compte (Gherkin F6.3) : profil anonymisé, objets
/// disparus, sessions purgées — les trocs finalisés avec soulte restent en
/// base sous forme anonymisée (obligations comptables). Refusée si des
/// trocs sont encore actifs.
pub async fn delete_account(pool: &PgPool, user_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let (active,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM trades \
         WHERE (proposer_id = $1 OR recipient_id = $1) \
           AND status IN ('attente_paiement', 'accepte', 'litige_gele')",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    if active > 0 {
        return Ok(false);
    }
    // Anonymisation : l'e-mail et le pseudo deviennent irréversibles, le
    // compte est marqué supprimé et inutilisable.
    sqlx::query(
        "UPDATE users SET \
            email = ('supprime-' || left(id::text, 8) || '@anonyme.lebontroc')::citext, \
            pseudo = ('parti_' || left(id::text, 8))::citext, \
            password_hash = 'compte-supprime', postal_code = '00000', \
            email_verified_at = NULL, deleted_at = now(), banned_at = now() \
         WHERE id = $1",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE items SET status = 'masque', deleted_at = now(), updated_at = now() \
         WHERE owner_id = $1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM notifications WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM wishlist_entries WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

// ————— Rôles d'administration —————

/// Le rôle et le statut « compte maître » d'un utilisateur.
#[derive(Debug, Clone, FromRow)]
pub struct RoleInfo {
    pub role: String,
    pub is_master: bool,
}

pub async fn role_of(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Option<RoleInfo>> {
    sqlx::query_as::<_, RoleInfo>("SELECT role, is_master FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

/// La cible d'un changement de rôle, avec ce qu'il faut pour décider.
#[derive(Debug, Clone, FromRow)]
pub struct RoleTarget {
    pub id: Uuid,
    pub pseudo: String,
    pub role: String,
    pub is_master: bool,
}

pub async fn find_role_target(pool: &PgPool, pseudo: &str) -> sqlx::Result<Option<RoleTarget>> {
    sqlx::query_as::<_, RoleTarget>(
        "SELECT id, pseudo::text AS pseudo, role, is_master FROM users WHERE pseudo = $1::citext",
    )
    .bind(pseudo)
    .fetch_optional(pool)
    .await
}

/// Applique le nouveau rôle — le garde-fou métier est en amont
/// (`domain::admin::peut_changer_role`).
pub async fn set_role(pool: &PgPool, user_id: Uuid, role: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET role = $2 WHERE id = $1 AND is_master = false")
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

/// L'équipe : tous ceux qui ont un accès au panneau.
pub async fn list_staff(pool: &PgPool) -> sqlx::Result<Vec<RoleTarget>> {
    sqlx::query_as::<_, RoleTarget>(
        "SELECT id, pseudo::text AS pseudo, role, is_master FROM users \
         WHERE role <> 'utilisateur' ORDER BY is_master DESC, role DESC, pseudo",
    )
    .fetch_all(pool)
    .await
}

/// Installe le compte maître au démarrage : super-admin, protégé,
/// unique. Idempotent — sans e-mail configuré, rien ne se passe.
pub async fn ensure_master(pool: &PgPool, email: &str) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE users SET role = 'super_admin', is_master = true \
         WHERE email = $1::citext AND is_master = false \
           AND NOT EXISTS (SELECT 1 FROM users WHERE is_master)",
    )
    .bind(email)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

// ————— Double authentification (TOTP) —————

/// L'état 2FA d'un compte.
#[derive(Debug, Clone, FromRow)]
pub struct TotpState {
    pub totp_secret: Option<String>,
    pub totp_enabled_at: Option<DateTime<Utc>>,
}

pub async fn totp_state(pool: &PgPool, user_id: Uuid) -> sqlx::Result<TotpState> {
    sqlx::query_as::<_, TotpState>("SELECT totp_secret, totp_enabled_at FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Pose un secret en attente de confirmation (2FA pas encore active).
pub async fn totp_start(pool: &PgPool, user_id: Uuid, secret: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET totp_secret = $2, totp_enabled_at = NULL WHERE id = $1")
        .bind(user_id)
        .bind(secret)
        .execute(pool)
        .await?;
    Ok(())
}

/// Active la 2FA (premier code vérifié) et enregistre les codes de secours
/// hachés — atomique. La session courante est élevée dans la foulée.
pub async fn totp_enable(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    recovery_hashes: &[String],
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET totp_enabled_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for hash in recovery_hashes {
        sqlx::query("INSERT INTO totp_recovery_codes (user_id, code_hash) VALUES ($1, $2)")
            .bind(user_id)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE sessions SET totp_verified_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Élève une session : le second facteur vient d'être vérifié.
pub async fn mark_session_totp_verified(pool: &PgPool, session_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE sessions SET totp_verified_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn session_totp_verified(pool: &PgPool, session_id: Uuid) -> sqlx::Result<bool> {
    let verified: Option<(Option<DateTime<Utc>>,)> =
        sqlx::query_as("SELECT totp_verified_at FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(pool)
            .await?;
    Ok(verified.and_then(|(v,)| v).is_some())
}

/// Consomme un code de secours (marqué utilisé s'il correspond).
pub async fn consume_recovery_code(
    pool: &PgPool,
    user_id: Uuid,
    code_hash: &str,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE totp_recovery_codes SET used_at = now() \
         WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL",
    )
    .bind(user_id)
    .bind(code_hash)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

pub async fn recovery_codes_left(pool: &PgPool, user_id: Uuid) -> sqlx::Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM totp_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Récupération (compte maître uniquement) : désactive entièrement la 2FA
/// de la cible, qui pourra la réactiver après reconnexion.
pub async fn totp_reset(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled_at = NULL WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
