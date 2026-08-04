//! Requêtes SQL des favoris et de la liste d'envies (F2.3).

use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::catalog_repo::FeedRow;

/// Ajoute un favori (idempotent). Retourne `true` si c'est une nouveauté.
pub async fn add_favorite(pool: &PgPool, user_id: Uuid, item_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO favorites (user_id, item_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(item_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Retire un favori. Retourne `true` s'il existait.
pub async fn remove_favorite(pool: &PgPool, user_id: Uuid, item_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM favorites WHERE user_id = $1 AND item_id = $2")
        .bind(user_id)
        .bind(item_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn is_favorited(pool: &PgPool, user_id: Uuid, item_id: Uuid) -> sqlx::Result<bool> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT item_id FROM favorites WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

pub async fn favorites_count(pool: &PgPool, item_id: Uuid) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT count(*) FROM favorites WHERE item_id = $1")
        .bind(item_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Compteurs de favoris pour un lot d'objets (dressing du propriétaire).
pub async fn favorites_counts(pool: &PgPool, item_ids: &[Uuid]) -> sqlx::Result<Vec<(Uuid, i64)>> {
    sqlx::query_as(
        "SELECT item_id, count(*) FROM favorites WHERE item_id = ANY($1) GROUP BY item_id",
    )
    .bind(item_ids)
    .fetch_all(pool)
    .await
}

/// Les favoris encore visibles (disponibles, non supprimés), du plus récent
/// cœur au plus ancien — mêmes cartes que le fil (ville, distance).
pub async fn list_favorite_items(
    pool: &PgPool,
    user_id: Uuid,
    viewer: Option<(f64, f64)>,
) -> sqlx::Result<Vec<FeedRow>> {
    let (lat, lng) = (viewer.map(|v| v.0), viewer.map(|v| v.1));
    sqlx::query_as::<_, FeedRow>(
        "SELECT i.id, i.owner_id, i.title, i.condition, i.value_cents, i.created_at, \
                c.nom AS city, \
                CASE WHEN $2::float8 IS NULL OR c.lat IS NULL THEN NULL \
                     ELSE 2.0 * 6371.0 * asin(sqrt( \
                         pow(sin(radians((c.lat - $2) / 2.0)), 2) \
                         + cos(radians($2)) * cos(radians(c.lat)) \
                           * pow(sin(radians((c.lng - $3) / 2.0)), 2))) \
                END AS distance_km \
         FROM favorites f \
         JOIN items i ON i.id = f.item_id \
         JOIN users u ON u.id = i.owner_id \
         LEFT JOIN communes c ON c.code_postal = u.postal_code \
         WHERE f.user_id = $1 AND i.status = 'disponible' AND i.deleted_at IS NULL \
         ORDER BY f.created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .bind(lat)
    .bind(lng)
    .fetch_all(pool)
    .await
}

// ————— Liste d'envies —————

#[derive(Debug, Clone, FromRow)]
pub struct WishlistEntry {
    pub position: i16,
    pub category_id: Option<i16>,
    pub keywords: String,
}

pub async fn list_wishlist(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<WishlistEntry>> {
    sqlx::query_as::<_, WishlistEntry>(
        "SELECT position, category_id, keywords FROM wishlist_entries \
         WHERE user_id = $1 ORDER BY position",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Remplace les 3 lignes « ce que je cherche » (transaction).
pub async fn replace_wishlist(
    pool: &PgPool,
    user_id: Uuid,
    entries: &[(Option<i16>, String)],
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM wishlist_entries WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for (position, (category_id, keywords)) in entries.iter().enumerate() {
        sqlx::query(
            "INSERT INTO wishlist_entries (user_id, position, category_id, keywords) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(position as i16)
        .bind(category_id)
        .bind(keywords)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
