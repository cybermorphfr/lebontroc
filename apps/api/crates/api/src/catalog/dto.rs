//! Types du contrat API pour le catalogue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct CategoryNode {
    pub id: i16,
    pub slug: String,
    pub label: String,
    /// Icône Lucide (racines uniquement).
    pub icon: Option<String>,
    /// Fourchette indicative en centimes (héritée de la racine).
    pub value_min_cents: Option<i32>,
    pub value_max_cents: Option<i32>,
    #[schema(no_recursion)]
    pub children: Vec<CategoryNode>,
}

#[derive(Deserialize, ToSchema)]
pub struct PresignRequest {
    /// 1 à 8 fichiers à uploader.
    pub files: Vec<PresignFile>,
}

#[derive(Deserialize, ToSchema)]
pub struct PresignFile {
    /// `image/webp` ou `image/jpeg`.
    #[schema(example = "image/webp")]
    pub content_type: String,
    /// Taille en octets (≤ 5 Mo).
    pub size: i32,
}

#[derive(Serialize, ToSchema)]
pub struct PresignedPhoto {
    pub photo_id: Uuid,
    /// URL de PUT direct vers le stockage (valable 15 min).
    pub upload_url: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateItemRequest {
    #[schema(example = "Poussette Yoyo")]
    pub title: String,
    pub description: String,
    pub category_id: i16,
    /// `neuf`, `tres_bon_etat`, `bon_etat` ou `correct`.
    #[schema(example = "tres_bon_etat")]
    pub condition: String,
    /// Valeur indicative en centimes (100 à 200 000).
    #[schema(example = 15000)]
    pub value_cents: i32,
    /// `main_propre`, `envoi` ou `les_deux`.
    #[schema(example = "main_propre")]
    pub delivery_pref: String,
    pub exchange_wishes: Option<String>,
    /// L'objet peut s'échanger avec un complément en euros (vrai par défaut).
    #[serde(default = "vrai")]
    pub accepts_soulte: bool,
    /// Ids issus de /items/photos/presign, dans l'ordre d'affichage (1–8).
    pub photos: Vec<Uuid>,
    /// Durée du flux de publication côté client (KPI « moins de 2 minutes »).
    pub duration_seconds: Option<i32>,
    /// Identifiant de brouillon côté client (funnel de télémétrie).
    pub draft_id: Option<Uuid>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateItemRequest {
    pub title: String,
    pub description: String,
    pub category_id: i16,
    pub condition: String,
    pub value_cents: i32,
    pub delivery_pref: String,
    pub exchange_wishes: Option<String>,
    /// `None` = inchangé.
    pub accepts_soulte: Option<bool>,
    /// `disponible` ou `masque` uniquement.
    pub status: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ReplacePhotosRequest {
    /// Liste ordonnée (1–8) : photos déjà rattachées et/ou nouveaux uploads.
    pub photos: Vec<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct ItemPhotoResponse {
    pub photo_id: Uuid,
    pub url: String,
    pub position: i16,
}

#[derive(Serialize, ToSchema)]
pub struct PublicProfileResponse {
    /// Identifiant public du troqueur (cible des signalements F5.2).
    pub user_id: Uuid,
    pub pseudo: String,
    /// Ville approximative (jamais le code postal complet).
    pub city: Option<String>,
    /// Date de création du compte (« Troque depuis… »).
    pub member_since: DateTime<Utc>,
    /// Objets disponibles uniquement — jamais les masqués.
    pub items: Vec<ItemResponse>,
    /// Note moyenne publiée (F5.1), absente sans évaluation.
    pub rating_avg: Option<f64>,
    pub reviews_count: i64,
    pub trades_finalized: i64,
    /// Délai moyen entre acceptation et dépôt de ses colis, en jours (F4.3).
    pub avg_ship_days: Option<f64>,
    /// Évaluations publiées, la plus récente d'abord (50 max).
    pub reviews: Vec<PublicReviewResponse>,
}

/// Une évaluation publiée sur un profil.
#[derive(Serialize, ToSchema)]
pub struct PublicReviewResponse {
    pub rating: i16,
    pub comment: Option<String>,
    /// Réponse publique du noté, le cas échéant.
    pub reply: Option<String>,
    pub reviewer_pseudo: String,
    pub published_at: DateTime<Utc>,
}

fn vrai() -> bool {
    true
}

/// Une ligne « ce que je cherche » (3 max).
#[derive(Serialize, Deserialize, ToSchema)]
pub struct WishlistEntryDto {
    pub category_id: Option<i16>,
    /// Mots-clés libres (« poussette yoyo », 120 caractères max).
    pub keywords: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateWishlistRequest {
    /// Remplace la liste complète (0 à 3 lignes).
    pub entries: Vec<WishlistEntryDto>,
}

/// Résultats de recherche — mêmes cartes que le fil.
#[derive(Serialize, ToSchema)]
pub struct SearchResponse {
    pub items: Vec<FeedCard>,
    pub page: u32,
    /// `true` s'il reste des résultats à charger.
    pub has_more: bool,
    /// Commune depuis laquelle les distances sont mesurées.
    pub reference: Option<String>,
}

/// Carte du fil d'accueil — volontairement légère (grille photo).
#[derive(Serialize, ToSchema)]
pub struct FeedCard {
    pub id: Uuid,
    pub title: String,
    pub condition: String,
    pub value_cents: i32,
    /// Ville approximative du propriétaire (jamais le code postal complet).
    pub city: Option<String>,
    /// Distance approximative en km — absente pour un visiteur non localisé.
    pub distance_km: Option<f64>,
    /// Photo de couverture.
    pub photo_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Une carte de favori : la carte du fil, augmentée de sa catégorie —
/// la page des favoris les range par rayon.
#[derive(Serialize, ToSchema)]
pub struct FavoriteCard {
    pub id: Uuid,
    pub title: String,
    pub condition: String,
    pub value_cents: i32,
    pub city: Option<String>,
    pub distance_km: Option<f64>,
    pub photo_url: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Catégorie racine : « Maison et déco », « Électronique »…
    pub rayon: String,
    /// Catégorie exacte : « Meubles », « Consoles et jeux vidéo »…
    pub categorie: String,
}

#[derive(Serialize, ToSchema)]
pub struct FeedResponse {
    pub items: Vec<FeedCard>,
    pub page: u32,
    /// `true` s'il reste des objets à charger (infinite scroll).
    pub has_more: bool,
    /// Commune depuis laquelle les distances sont mesurées.
    pub reference: Option<String>,
}

/// Encart propriétaire d'une fiche objet.
#[derive(Serialize, ToSchema)]
pub struct ItemOwnerResponse {
    pub pseudo: String,
    /// Ville approximative (jamais d'adresse ni de code postal complet).
    pub city: Option<String>,
    pub member_since: DateTime<Utc>,
}

/// Fiche objet complète : objet + encart propriétaire + distance.
#[derive(Serialize, ToSchema)]
pub struct ItemDetailResponse {
    pub item: ItemResponse,
    pub owner: ItemOwnerResponse,
    /// Distance approximative en km depuis le visiteur connecté.
    pub distance_km: Option<f64>,
    pub is_owner: bool,
    /// Nombre de cœurs posés sur l'objet.
    pub favorites_count: i64,
    /// Le visiteur connecté a-t-il mis cet objet en favori ?
    pub is_favorited: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ItemResponse {
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
    pub accepts_soulte: bool,
    /// Nombre de cœurs — rempli dans le dressing du propriétaire.
    pub favorites_count: Option<i64>,
    pub photos: Vec<ItemPhotoResponse>,
    pub created_at: DateTime<Utc>,
}
