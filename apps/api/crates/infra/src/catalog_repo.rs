//! Requêtes SQL du catalogue : catégories, objets, photos, uploads présignés.
//! Convention projet : pas de macros `query!`, chaque fonction est couverte
//! par un test d'intégration `#[sqlx::test]`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Category {
    pub id: i16,
    pub parent_id: Option<i16>,
    pub slug: String,
    pub label: String,
    pub icon: Option<String>,
    pub depth: i16,
    pub sort_order: i16,
    pub value_min_cents: Option<i32>,
    pub value_max_cents: Option<i32>,
}

pub async fn list_categories(pool: &PgPool) -> sqlx::Result<Vec<Category>> {
    sqlx::query_as::<_, Category>("SELECT * FROM categories ORDER BY depth, sort_order, id")
        .fetch_all(pool)
        .await
}

/// Nom de la commune la plus peuplée pour un code postal.
pub async fn commune_for_postal_code(
    pool: &PgPool,
    code_postal: &str,
) -> sqlx::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT nom FROM communes WHERE code_postal = $1")
        .bind(code_postal)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// Coordonnées (lat, lng) de la commune d'un code postal.
pub async fn commune_coords_for_postal_code(
    pool: &PgPool,
    code_postal: &str,
) -> sqlx::Result<Option<(f64, f64)>> {
    sqlx::query_as("SELECT lat, lng FROM communes WHERE code_postal = $1")
        .bind(code_postal)
        .fetch_optional(pool)
        .await
}

pub async fn category_exists(pool: &PgPool, id: i16) -> sqlx::Result<bool> {
    let row: Option<(i16,)> = sqlx::query_as("SELECT id FROM categories WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

// ————— Uploads présignés —————

#[derive(Debug, Clone, FromRow)]
pub struct PhotoUpload {
    pub photo_id: Uuid,
    pub user_id: Uuid,
    pub s3_key: String,
    pub content_type: String,
    pub byte_size: i32,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_photo_upload(
    pool: &PgPool,
    photo_id: Uuid,
    user_id: Uuid,
    s3_key: &str,
    content_type: &str,
    byte_size: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO photo_uploads (photo_id, user_id, s3_key, content_type, byte_size) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(photo_id)
    .bind(user_id)
    .bind(s3_key)
    .bind(content_type)
    .bind(byte_size)
    .execute(pool)
    .await?;
    Ok(())
}

/// Les uploads de CET utilisateur parmi les ids demandés (anti-greffe).
pub async fn find_photo_uploads(
    pool: &PgPool,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> sqlx::Result<Vec<PhotoUpload>> {
    sqlx::query_as::<_, PhotoUpload>(
        "SELECT * FROM photo_uploads WHERE user_id = $1 AND photo_id = ANY($2)",
    )
    .bind(user_id)
    .bind(photo_ids)
    .fetch_all(pool)
    .await
}

pub async fn orphan_uploads_before(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> sqlx::Result<Vec<PhotoUpload>> {
    sqlx::query_as::<_, PhotoUpload>("SELECT * FROM photo_uploads WHERE created_at < $1 LIMIT 200")
        .bind(cutoff)
        .fetch_all(pool)
        .await
}

pub async fn delete_photo_upload(pool: &PgPool, photo_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM photo_uploads WHERE photo_id = $1")
        .bind(photo_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ————— Objets —————

#[derive(Debug, Clone, FromRow)]
pub struct Item {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub description: String,
    pub category_id: i16,
    pub condition: String,
    pub status: String,
    pub value_cents: i32,
    pub delivery_pref: String,
    pub exchange_wishes: Option<String>,
    /// L'objet peut s'échanger avec un complément en euros (filtre F2.2).
    pub accepts_soulte: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ItemPhoto {
    pub photo_id: Uuid,
    pub item_id: Uuid,
    pub position: i16,
    pub s3_key: String,
    pub content_type: String,
}

pub struct NewItem<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub category_id: i16,
    pub condition: &'a str,
    pub value_cents: i32,
    pub delivery_pref: &'a str,
    pub exchange_wishes: Option<&'a str>,
    pub accepts_soulte: bool,
}

/// Crée l'objet et rattache les photos (dans l'ordre fourni) en une
/// transaction ; consomme les lignes `photo_uploads`.
pub async fn create_item(
    pool: &PgPool,
    owner_id: Uuid,
    new_item: NewItem<'_>,
    photos: &[PhotoUpload],
) -> sqlx::Result<Item> {
    let mut tx = pool.begin().await?;
    let item = sqlx::query_as::<_, Item>(
        "INSERT INTO items (owner_id, title, description, category_id, condition, value_cents, delivery_pref, exchange_wishes, accepts_soulte) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
    )
    .bind(owner_id)
    .bind(new_item.title)
    .bind(new_item.description)
    .bind(new_item.category_id)
    .bind(new_item.condition)
    .bind(new_item.value_cents)
    .bind(new_item.delivery_pref)
    .bind(new_item.exchange_wishes)
    .bind(new_item.accepts_soulte)
    .fetch_one(&mut *tx)
    .await?;

    for (position, photo) in photos.iter().enumerate() {
        sqlx::query(
            "INSERT INTO item_photos (photo_id, item_id, position, s3_key, content_type) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(photo.photo_id)
        .bind(item.id)
        .bind(position as i16)
        .bind(&photo.s3_key)
        .bind(&photo.content_type)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM photo_uploads WHERE photo_id = $1")
            .bind(photo.photo_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(item)
}

/// Objets non supprimés parmi les ids demandés (composition d'un troc).
pub async fn items_by_ids(pool: &PgPool, ids: &[Uuid]) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = ANY($1) AND deleted_at IS NULL")
        .bind(ids)
        .fetch_all(pool)
        .await
}

pub async fn get_item(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Item>> {
    sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_items_by_owner(pool: &PgPool, owner_id: Uuid) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as::<_, Item>(
        "SELECT * FROM items WHERE owner_id = $1 AND deleted_at IS NULL \
         ORDER BY created_at DESC LIMIT 200",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

/// Objets visibles d'un dressing public : disponibles, non supprimés.
pub async fn list_public_items_by_owner(pool: &PgPool, owner_id: Uuid) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as::<_, Item>(
        "SELECT * FROM items WHERE owner_id = $1 AND status = 'disponible' \
         AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 200",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

// ————— Fil d'accueil —————

#[derive(Debug, Clone, FromRow)]
pub struct FeedRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub condition: String,
    pub value_cents: i32,
    pub created_at: DateTime<Utc>,
    /// Ville approximative du propriétaire (commune du code postal).
    pub city: Option<String>,
    /// Distance en km depuis le point de vue (`NULL` si non localisable).
    pub distance_km: Option<f64>,
}

/// Fil paginé des objets disponibles, trié par score `distance + récence` :
/// 25 km « coûtent » un jour d'ancienneté ; un objet non localisable est
/// traité comme à 800 km. Sans point de vue, seule la récence ordonne.
pub async fn list_feed_items(
    pool: &PgPool,
    viewer: Option<(f64, f64)>,
    exclude_owner: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<FeedRow>> {
    let (lat, lng) = (viewer.map(|v| v.0), viewer.map(|v| v.1));
    sqlx::query_as::<_, FeedRow>(
        "SELECT t.* FROM ( \
            SELECT i.id, i.owner_id, i.title, i.condition, i.value_cents, i.created_at, \
                   c.nom AS city, \
                   CASE WHEN $1::float8 IS NULL OR c.lat IS NULL THEN NULL \
                        ELSE 2.0 * 6371.0 * asin(sqrt( \
                            pow(sin(radians((c.lat - $1) / 2.0)), 2) \
                            + cos(radians($1)) * cos(radians(c.lat)) \
                              * pow(sin(radians((c.lng - $2) / 2.0)), 2))) \
                   END AS distance_km \
            FROM items i \
            JOIN users u ON u.id = i.owner_id \
            LEFT JOIN communes c ON c.code_postal = u.postal_code \
            WHERE i.status = 'disponible' AND i.deleted_at IS NULL \
              AND ($3::uuid IS NULL OR i.owner_id <> $3) \
              AND ($3::uuid IS NULL OR NOT EXISTS ( \
                  SELECT 1 FROM user_blocks b \
                  WHERE (b.blocker_id = $3 AND b.blocked_id = i.owner_id) \
                     OR (b.blocker_id = i.owner_id AND b.blocked_id = $3))) \
         ) AS t \
         ORDER BY COALESCE(t.distance_km, 800.0) / 25.0 \
                  + EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0 \
         LIMIT $4 OFFSET $5",
    )
    .bind(lat)
    .bind(lng)
    .bind(exclude_owner)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Soft delete d'un objet du propriétaire ; purge les lignes photos et
/// retourne leurs clés S3 (à supprimer du bucket). `None` si introuvable.
pub async fn soft_delete_item(
    pool: &PgPool,
    item_id: Uuid,
    owner_id: Uuid,
) -> sqlx::Result<Option<Vec<String>>> {
    let mut tx = pool.begin().await?;
    let updated: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE items SET deleted_at = now() \
         WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL RETURNING id",
    )
    .bind(item_id)
    .bind(owner_id)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Ok(None);
    }
    let removed: Vec<(String,)> =
        sqlx::query_as("DELETE FROM item_photos WHERE item_id = $1 RETURNING s3_key")
            .bind(item_id)
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(Some(removed.into_iter().map(|r| r.0).collect()))
}

pub async fn photos_for_items(pool: &PgPool, item_ids: &[Uuid]) -> sqlx::Result<Vec<ItemPhoto>> {
    sqlx::query_as::<_, ItemPhoto>(
        "SELECT photo_id, item_id, position, s3_key, content_type FROM item_photos \
         WHERE item_id = ANY($1) ORDER BY item_id, position",
    )
    .bind(item_ids)
    .fetch_all(pool)
    .await
}

pub struct ItemUpdate<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub category_id: i16,
    pub condition: &'a str,
    pub value_cents: i32,
    pub delivery_pref: &'a str,
    pub exchange_wishes: Option<&'a str>,
    pub accepts_soulte: bool,
    pub status: &'a str,
}

/// Met à jour un objet du propriétaire. Retourne `None` si l'objet n'existe
/// pas ou n'appartient pas à l'utilisateur.
pub async fn update_item(
    pool: &PgPool,
    item_id: Uuid,
    owner_id: Uuid,
    update: ItemUpdate<'_>,
) -> sqlx::Result<Option<Item>> {
    sqlx::query_as::<_, Item>(
        "UPDATE items SET title = $3, description = $4, category_id = $5, condition = $6, \
         value_cents = $7, delivery_pref = $8, exchange_wishes = $9, status = $10, \
         accepts_soulte = $11, updated_at = now() \
         WHERE id = $1 AND owner_id = $2 RETURNING *",
    )
    .bind(item_id)
    .bind(owner_id)
    .bind(update.title)
    .bind(update.description)
    .bind(update.category_id)
    .bind(update.condition)
    .bind(update.value_cents)
    .bind(update.delivery_pref)
    .bind(update.exchange_wishes)
    .bind(update.status)
    .bind(update.accepts_soulte)
    .fetch_optional(pool)
    .await
}

/// Remplace l'ensemble ordonné des photos d'un objet.
/// `kept_or_new` mélange photos déjà rattachées et uploads fraîchement
/// consommés. Retourne les clés S3 des photos retirées (à supprimer du bucket).
pub async fn replace_item_photos(
    pool: &PgPool,
    item_id: Uuid,
    ordered: &[(Uuid, String, String)], // (photo_id, s3_key, content_type)
) -> sqlx::Result<Vec<String>> {
    let mut tx = pool.begin().await?;
    let keep_ids: Vec<Uuid> = ordered.iter().map(|(id, _, _)| *id).collect();

    let removed: Vec<(String,)> = sqlx::query_as(
        "DELETE FROM item_photos WHERE item_id = $1 AND NOT (photo_id = ANY($2)) \
         RETURNING s3_key",
    )
    .bind(item_id)
    .bind(&keep_ids)
    .fetch_all(&mut *tx)
    .await?;

    for (position, (photo_id, s3_key, content_type)) in ordered.iter().enumerate() {
        sqlx::query(
            "INSERT INTO item_photos (photo_id, item_id, position, s3_key, content_type) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (photo_id) DO UPDATE SET position = EXCLUDED.position",
        )
        .bind(photo_id)
        .bind(item_id)
        .bind(position as i16)
        .bind(s3_key)
        .bind(content_type)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM photo_uploads WHERE photo_id = $1")
            .bind(photo_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(removed.into_iter().map(|r| r.0).collect())
}
