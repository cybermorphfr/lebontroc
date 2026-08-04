//! Recherche d'objets — derrière le trait `SearchRepository` pour pouvoir
//! substituer Postgres (FTS français + pg_trgm) par Meilisearch en V2 sans
//! toucher au domaine ni aux handlers.

use async_trait::async_trait;
use sqlx::PgPool;

use crate::catalog_repo::FeedRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSort {
    /// Rang FTS + similarité trigramme ; sans requête, retombe sur la récence.
    Pertinence,
    Distance,
    Recence,
}

#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    /// Texte libre (websearch : `poussette -lit`, guillemets…).
    pub query: Option<String>,
    /// Catégorie racine ou feuille — les descendants sont inclus.
    pub category_id: Option<i16>,
    pub condition: Option<String>,
    /// Mode souhaité : un objet `les_deux` satisfait `main_propre` et `envoi`.
    pub delivery_pref: Option<String>,
    pub accepts_soulte: Option<bool>,
    /// Ignoré sans point de vue (visiteur non localisé).
    pub max_km: Option<f64>,
    pub viewer: Option<(f64, f64)>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
pub trait SearchRepository: Send + Sync {
    async fn search(&self, params: &SearchParams, sort: SearchSort)
        -> anyhow::Result<Vec<FeedRow>>;
}

/// Implémentation PostgreSQL : `search_tsv` (généré, indexé GIN) pour la
/// pertinence, `word_similarity` pour la tolérance aux fautes.
pub struct PgSearchRepository {
    pool: PgPool,
}

impl PgSearchRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Seuil de similarité trigramme : « pousette » trouve « Poussette Yoyo »,
/// mais « table » ne trouve pas « câble ».
const SIMILARITE_MIN: f64 = 0.35;

#[async_trait]
impl SearchRepository for PgSearchRepository {
    async fn search(
        &self,
        params: &SearchParams,
        sort: SearchSort,
    ) -> anyhow::Result<Vec<FeedRow>> {
        let order = match sort {
            SearchSort::Pertinence => "t.rank DESC, t.created_at DESC",
            SearchSort::Distance => "COALESCE(t.distance_km, 1e6) ASC, t.created_at DESC",
            SearchSort::Recence => "t.created_at DESC",
        };
        let sql = format!(
            "SELECT t.id, t.owner_id, t.title, t.condition, t.value_cents, t.created_at, \
                    t.city, t.distance_km \
             FROM ( \
                SELECT i.id, i.owner_id, i.title, i.condition, i.value_cents, i.created_at, \
                       c.nom AS city, \
                       CASE WHEN $1::float8 IS NULL OR c.lat IS NULL THEN NULL \
                            ELSE 2.0 * 6371.0 * asin(sqrt( \
                                pow(sin(radians((c.lat - $1) / 2.0)), 2) \
                                + cos(radians($1)) * cos(radians(c.lat)) \
                                  * pow(sin(radians((c.lng - $2) / 2.0)), 2))) \
                       END AS distance_km, \
                       CASE WHEN $3::text IS NULL THEN 0 \
                            ELSE ts_rank(i.search_tsv, websearch_to_tsquery('french', $3)) \
                                 + word_similarity($3, i.title) \
                       END AS rank \
                FROM items i \
                JOIN users u ON u.id = i.owner_id \
                LEFT JOIN communes c ON c.code_postal = u.postal_code \
                WHERE i.status = 'disponible' AND i.deleted_at IS NULL \
                  AND ($3::text IS NULL \
                       OR i.search_tsv @@ websearch_to_tsquery('french', $3) \
                       OR word_similarity($3, i.title) >= $4) \
                  AND ($5::int2 IS NULL OR i.category_id IN ( \
                       WITH RECURSIVE sous_arbre AS ( \
                           SELECT id FROM categories WHERE id = $5 \
                           UNION ALL \
                           SELECT c2.id FROM categories c2 \
                           JOIN sous_arbre s ON c2.parent_id = s.id \
                       ) SELECT id FROM sous_arbre)) \
                  AND ($6::text IS NULL OR i.condition = $6) \
                  AND ($7::text IS NULL OR i.delivery_pref = $7 OR i.delivery_pref = 'les_deux') \
                  AND ($8::bool IS NULL OR i.accepts_soulte = $8) \
             ) AS t \
             WHERE ($9::float8 IS NULL OR $1::float8 IS NULL OR t.distance_km <= $9) \
             ORDER BY {order} \
             LIMIT $10 OFFSET $11"
        );
        let (lat, lng) = (params.viewer.map(|v| v.0), params.viewer.map(|v| v.1));
        let rows = sqlx::query_as::<_, FeedRow>(&sql)
            .bind(lat)
            .bind(lng)
            .bind(params.query.as_deref())
            .bind(SIMILARITE_MIN)
            .bind(params.category_id)
            .bind(params.condition.as_deref())
            .bind(params.delivery_pref.as_deref())
            .bind(params.accepts_soulte)
            .bind(params.max_km)
            .bind(params.limit)
            .bind(params.offset)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }
}
