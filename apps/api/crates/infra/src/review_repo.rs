//! Requêtes SQL des évaluations (F5.1). L'embargo anti-représailles vit
//! dans `published_at` : NULL = invisible pour tout le monde sauf l'auteur.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Review {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub reviewer_id: Uuid,
    pub reviewee_id: Uuid,
    pub rating: i16,
    pub comment: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub reply: Option<String>,
    pub created_at: DateTime<Utc>,
}

const REVIEW_COLUMNS: &str =
    "id, trade_id, reviewer_id, reviewee_id, rating, comment, published_at, reply, created_at";

/// Issue d'un dépôt de note.
pub enum SubmitOutcome {
    /// Note enregistrée ; `published` : l'autre avait déjà noté, les deux
    /// viennent d'être publiées ensemble.
    Created {
        id: Uuid,
        published: bool,
    },
    AlreadyReviewed,
}

/// Enregistre la note et publie les deux si l'autre partie a déjà noté —
/// atomique (Gherkin « publication simultanée anti-représailles »).
pub async fn submit_review(
    pool: &PgPool,
    trade_id: Uuid,
    reviewer_id: Uuid,
    reviewee_id: Uuid,
    rating: i16,
    comment: Option<&str>,
) -> sqlx::Result<SubmitOutcome> {
    let mut tx = pool.begin().await?;
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO reviews (trade_id, reviewer_id, reviewee_id, rating, comment) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (trade_id, reviewer_id) DO NOTHING RETURNING id",
    )
    .bind(trade_id)
    .bind(reviewer_id)
    .bind(reviewee_id)
    .bind(rating)
    .bind(comment)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id,)) = inserted else {
        return Ok(SubmitOutcome::AlreadyReviewed);
    };
    let (both,): (i64,) = sqlx::query_as("SELECT count(*) FROM reviews WHERE trade_id = $1")
        .bind(trade_id)
        .fetch_one(&mut *tx)
        .await?;
    let published = both >= 2;
    if published {
        sqlx::query(
            "UPDATE reviews SET published_at = now() \
             WHERE trade_id = $1 AND published_at IS NULL",
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(SubmitOutcome::Created { id, published })
}

/// Les notes d'un troc (publiées ou non — le handler filtre pour le lecteur).
pub async fn reviews_for_trade(pool: &PgPool, trade_id: Uuid) -> sqlx::Result<Vec<Review>> {
    sqlx::query_as::<_, Review>(&format!(
        "SELECT {REVIEW_COLUMNS} FROM reviews WHERE trade_id = $1"
    ))
    .bind(trade_id)
    .fetch_all(pool)
    .await
}

/// Réponse publique unique du noté, sur une note publiée.
pub async fn reply_to_review(
    pool: &PgPool,
    review_id: Uuid,
    reviewee_id: Uuid,
    reply: &str,
) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE reviews SET reply = $3, reply_at = now() \
         WHERE id = $1 AND reviewee_id = $2 AND published_at IS NOT NULL AND reply IS NULL",
    )
    .bind(review_id)
    .bind(reviewee_id)
    .bind(reply)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Publie les notes orphelines J+14 après la finalisation (l'autre partie
/// n'a jamais noté) — claim atomique, retourne le nombre publié.
pub async fn publish_overdue_reviews(pool: &PgPool, days: i64) -> sqlx::Result<u64> {
    let updated = sqlx::query(
        "UPDATE reviews r SET published_at = now() \
         FROM trades t \
         WHERE t.id = r.trade_id AND r.published_at IS NULL \
           AND t.finalized_at < now() - make_interval(days => $1::int)",
    )
    .bind(days)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected())
}

/// Une note publiée, telle qu'affichée sur un profil.
#[derive(Debug, Clone, FromRow)]
pub struct PublicReview {
    pub rating: i16,
    pub comment: Option<String>,
    pub reply: Option<String>,
    pub reviewer_pseudo: String,
    pub published_at: DateTime<Utc>,
}

pub async fn published_reviews_for_user(
    pool: &PgPool,
    reviewee_id: Uuid,
) -> sqlx::Result<Vec<PublicReview>> {
    sqlx::query_as::<_, PublicReview>(
        "SELECT r.rating, r.comment, r.reply, u.pseudo::text AS reviewer_pseudo, \
                r.published_at \
         FROM reviews r JOIN users u ON u.id = r.reviewer_id \
         WHERE r.reviewee_id = $1 AND r.published_at IS NOT NULL \
         ORDER BY r.published_at DESC LIMIT 50",
    )
    .bind(reviewee_id)
    .fetch_all(pool)
    .await
}

/// La réputation d'un troqueur : note moyenne publiée, volume, délai moyen
/// entre acceptation et dépôt de ses colis (données F4.3).
#[derive(Debug, Clone, FromRow)]
pub struct ProfileStats {
    pub rating_avg: Option<f64>,
    pub reviews_count: i64,
    pub trades_finalized: i64,
    pub avg_ship_days: Option<f64>,
}

pub async fn profile_stats(pool: &PgPool, user_id: Uuid) -> sqlx::Result<ProfileStats> {
    sqlx::query_as::<_, ProfileStats>(
        "SELECT \
            (SELECT avg(rating)::float8 FROM reviews \
             WHERE reviewee_id = $1 AND published_at IS NOT NULL) AS rating_avg, \
            (SELECT count(*) FROM reviews \
             WHERE reviewee_id = $1 AND published_at IS NOT NULL) AS reviews_count, \
            (SELECT count(*) FROM trades \
             WHERE status = 'finalise' AND (proposer_id = $1 OR recipient_id = $1)) \
             AS trades_finalized, \
            (SELECT (avg(EXTRACT(EPOCH FROM s.dropped_at - t.created_at)) / 86400)::float8 \
             FROM shipments s JOIN trades t ON t.id = s.trade_id \
             WHERE s.sender_id = $1 AND s.dropped_at IS NOT NULL) AS avg_ship_days",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}
