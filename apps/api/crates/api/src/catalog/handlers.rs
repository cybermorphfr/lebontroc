//! Handlers du catalogue : catégories, publication, dressing, photos.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use domain::catalog as regles;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::catalog::dto::{
    CategoryNode, CreateItemRequest, FeedCard, FeedResponse, ItemDetailResponse, ItemOwnerResponse,
    ItemPhotoResponse, ItemResponse, PresignRequest, PresignedPhoto, PublicProfileResponse,
    ReplacePhotosRequest, SearchResponse, UpdateItemRequest, UpdateWishlistRequest,
    WishlistEntryDto,
};
use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::telemetry;
use crate::AppState;

fn map_catalog_error(error: regles::CatalogError) -> ApiError {
    match error {
        regles::CatalogError::TitreInvalide => ApiError::bad_request(
            "titre_invalide",
            "Le titre doit faire entre 3 et 80 caractères.",
        ),
        regles::CatalogError::DescriptionInvalide => ApiError::bad_request(
            "description_invalide",
            "Décris ton objet en 10 à 2 000 caractères.",
        ),
        regles::CatalogError::ValeurInvalide => ApiError::bad_request(
            "valeur_invalide",
            "Indique une valeur entre 1 et 2 000 € — une estimation honnête suffit.",
        ),
        regles::CatalogError::EtatInconnu => {
            ApiError::bad_request("etat_inconnu", "Choisis l'état de ton objet.")
        }
        regles::CatalogError::RemiseInconnue => {
            ApiError::bad_request("remise_inconnue", "Choisis un mode de remise.")
        }
        regles::CatalogError::PhotosInvalides => {
            ApiError::bad_request("photos_invalides", "Ajoute entre 1 et 8 photos.")
        }
        regles::CatalogError::StatutInterdit => ApiError::bad_request(
            "statut_interdit",
            "Tu peux seulement masquer ou remettre en ligne ton objet.",
        ),
    }
}

/// L'utilisateur doit avoir vérifié son e-mail pour publier (Gherkin F1.1).
pub(crate) async fn require_verified(state: &AppState, user: CurrentUser) -> Result<(), ApiError> {
    let compte = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Connecte-toi pour continuer."))?;
    if compte.email_verified_at.is_none() {
        return Err(ApiError::forbidden(
            "email_non_verifie",
            "Vérifie ton e-mail pour publier — regarde ta boîte mail.",
        ));
    }
    Ok(())
}

fn valider_champs_objet(
    title: &str,
    description: &str,
    condition: &str,
    value_cents: i32,
    delivery_pref: &str,
) -> Result<(), ApiError> {
    regles::valider_titre(title).map_err(map_catalog_error)?;
    regles::valider_description(description).map_err(map_catalog_error)?;
    regles::valider_condition(condition).map_err(map_catalog_error)?;
    regles::valider_valeur(value_cents).map_err(map_catalog_error)?;
    regles::valider_remise(delivery_pref).map_err(map_catalog_error)?;
    Ok(())
}

fn item_response(
    state: &AppState,
    item: infra::catalog_repo::Item,
    photos: Vec<infra::catalog_repo::ItemPhoto>,
) -> ItemResponse {
    ItemResponse {
        id: item.id,
        owner_id: item.owner_id,
        title: item.title,
        description: item.description,
        category_id: item.category_id,
        condition: item.condition,
        status: item.status,
        value_cents: item.value_cents,
        delivery_pref: item.delivery_pref,
        exchange_wishes: item.exchange_wishes,
        accepts_soulte: item.accepts_soulte,
        favorites_count: None,
        photos: photos
            .into_iter()
            .map(|p| ItemPhotoResponse {
                photo_id: p.photo_id,
                url: state.photos.public_url(&p.s3_key),
                position: p.position,
            })
            .collect(),
        created_at: item.created_at,
    }
}

/// Arbre des catégories, fourchettes de valeur héritées de la racine.
#[utoipa::path(
    get,
    path = "/categories",
    tag = "catalog",
    responses((status = 200, description = "Arbre des catégories", body = [CategoryNode]))
)]
pub async fn categories(
    State(state): State<AppState>,
) -> Result<Json<Vec<CategoryNode>>, ApiError> {
    let flat = infra::catalog_repo::list_categories(&state.pool).await?;

    // Fourchettes des racines, héritées par les descendants.
    let root_ranges: HashMap<i16, (Option<i32>, Option<i32>)> = flat
        .iter()
        .filter(|c| c.parent_id.is_none())
        .map(|c| (c.id, (c.value_min_cents, c.value_max_cents)))
        .collect();
    let root_of: HashMap<i16, i16> = flat
        .iter()
        .map(|c| {
            let mut current = c;
            while let Some(parent_id) = current.parent_id {
                match flat.iter().find(|p| p.id == parent_id) {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            (c.id, current.id)
        })
        .collect();

    fn build(
        flat: &[infra::catalog_repo::Category],
        parent: Option<i16>,
        root_ranges: &HashMap<i16, (Option<i32>, Option<i32>)>,
        root_of: &HashMap<i16, i16>,
    ) -> Vec<CategoryNode> {
        flat.iter()
            .filter(|c| c.parent_id == parent)
            .map(|c| {
                let (min, max) = c
                    .value_min_cents
                    .map(|m| (Some(m), c.value_max_cents))
                    .unwrap_or_else(|| {
                        root_of
                            .get(&c.id)
                            .and_then(|r| root_ranges.get(r).copied())
                            .unwrap_or((None, None))
                    });
                CategoryNode {
                    id: c.id,
                    slug: c.slug.clone(),
                    label: c.label.clone(),
                    icon: c.icon.clone(),
                    value_min_cents: min,
                    value_max_cents: max,
                    children: build(flat, Some(c.id), root_ranges, root_of),
                }
            })
            .collect()
    }

    Ok(Json(build(&flat, None, &root_ranges, &root_of)))
}

/// URL présignées d'upload (1–8 photos, 15 min).
#[utoipa::path(
    post,
    path = "/items/photos/presign",
    tag = "catalog",
    request_body = PresignRequest,
    responses(
        (status = 200, description = "URL de PUT présignées", body = [PresignedPhoto]),
        (status = 400, description = "Fichier refusé", body = crate::error::ErrorResponse),
        (status = 403, description = "E-mail non vérifié", body = crate::error::ErrorResponse)
    )
)]
pub async fn presign_photos(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<PresignRequest>,
) -> Result<Json<Vec<PresignedPhoto>>, ApiError> {
    require_verified(&state, user).await?;
    regles::valider_nombre_photos(body.files.len()).map_err(map_catalog_error)?;

    let mut result = Vec::with_capacity(body.files.len());
    for file in &body.files {
        let extension = match file.content_type.as_str() {
            "image/webp" => "webp",
            "image/jpeg" => "jpg",
            _ => {
                return Err(ApiError::bad_request(
                    "type_refuse",
                    "On n'a pas réussi à lire cette image. Essaie un autre format (JPG, WebP).",
                ))
            }
        };
        if file.size <= 0 || file.size > 5 * 1024 * 1024 {
            return Err(ApiError::bad_request(
                "fichier_trop_lourd",
                "Cette photo dépasse 5 Mo après compression. Réessaie avec une autre.",
            ));
        }
        let photo_id = Uuid::new_v4();
        let key = format!("items/{photo_id}.{extension}");
        let upload_url = state
            .photos
            .presign_put(&key, &file.content_type, i64::from(file.size))
            .await
            .map_err(ApiError::internal)?;
        infra::catalog_repo::insert_photo_upload(
            &state.pool,
            photo_id,
            user.user_id,
            &key,
            &file.content_type,
            file.size,
        )
        .await?;
        result.push(PresignedPhoto {
            photo_id,
            upload_url,
        });
    }
    Ok(Json(result))
}

/// Résout et vérifie des uploads présignés appartenant à l'utilisateur.
async fn resolve_uploads(
    state: &AppState,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> Result<Vec<infra::catalog_repo::PhotoUpload>, ApiError> {
    let uploads = infra::catalog_repo::find_photo_uploads(&state.pool, user_id, photo_ids).await?;
    let by_id: HashMap<Uuid, infra::catalog_repo::PhotoUpload> =
        uploads.into_iter().map(|u| (u.photo_id, u)).collect();
    let mut ordered = Vec::with_capacity(photo_ids.len());
    for id in photo_ids {
        let upload = by_id.get(id).cloned().ok_or_else(|| {
            ApiError::bad_request(
                "photo_inconnue",
                "Une des photos n'a pas été trouvée. Réessaie l'envoi.",
            )
        })?;
        if !state.photos.object_exists(&upload.s3_key).await {
            return Err(ApiError::bad_request(
                "photo_manquante",
                "L'envoi d'une photo n'est pas terminé. Touche la photo pour réessayer.",
            ));
        }
        ordered.push(upload);
    }
    Ok(ordered)
}

/// Publier un objet.
#[utoipa::path(
    post,
    path = "/items",
    tag = "catalog",
    request_body = CreateItemRequest,
    responses(
        (status = 201, description = "Objet publié", body = ItemResponse),
        (status = 400, description = "Champ invalide", body = crate::error::ErrorResponse),
        (status = 403, description = "E-mail non vérifié", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateItemRequest>,
) -> Result<(StatusCode, Json<ItemResponse>), ApiError> {
    require_verified(&state, user).await?;
    valider_champs_objet(
        &body.title,
        &body.description,
        &body.condition,
        body.value_cents,
        &body.delivery_pref,
    )?;
    regles::valider_nombre_photos(body.photos.len()).map_err(map_catalog_error)?;
    if !infra::catalog_repo::category_exists(&state.pool, body.category_id).await? {
        return Err(ApiError::bad_request(
            "categorie_inconnue",
            "Choisis une catégorie pour que ton objet soit trouvable.",
        ));
    }

    let uploads = resolve_uploads(&state, user.user_id, &body.photos).await?;
    let exchange_wishes = body
        .exchange_wishes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(300).collect::<String>());

    let item = infra::catalog_repo::create_item(
        &state.pool,
        user.user_id,
        infra::catalog_repo::NewItem {
            title: body.title.trim(),
            description: body.description.trim(),
            category_id: body.category_id,
            condition: &body.condition,
            value_cents: body.value_cents,
            delivery_pref: &body.delivery_pref,
            exchange_wishes: exchange_wishes.as_deref(),
            accepts_soulte: body.accepts_soulte,
        },
        &uploads,
    )
    .await?;

    for (index, upload) in uploads.iter().enumerate() {
        telemetry::track(
            &state,
            "item_photo_uploaded",
            Some(user.user_id),
            json!({"draft_id": body.draft_id, "photo_index": index, "size_bytes": upload.byte_size}),
        )
        .await;
    }
    telemetry::track(
        &state,
        "item_published",
        Some(user.user_id),
        json!({
            "draft_id": body.draft_id,
            "category": item.category_id,
            "photo_count": uploads.len(),
            "duration_seconds": body.duration_seconds,
            "has_exchange_wish": item.exchange_wishes.is_some(),
            "delivery_pref": item.delivery_pref,
            "condition": item.condition,
            "value_cents": item.value_cents,
        }),
    )
    .await;

    let photos = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    Ok((
        StatusCode::CREATED,
        Json(item_response(&state, item, photos)),
    ))
}

const FEED_PAGE_SIZE: i64 = 24;

/// Point de vue géographique du visiteur connecté (commune de son code postal).
async fn viewer_coords(
    state: &AppState,
    viewer: Option<&CurrentUser>,
) -> Result<Option<(f64, f64)>, ApiError> {
    let Some(viewer) = viewer else {
        return Ok(None);
    };
    let Some(user) = infra::auth_repo::find_user_by_id(&state.pool, viewer.user_id).await? else {
        return Ok(None);
    };
    Ok(infra::catalog_repo::commune_coords_for_postal_code(&state.pool, &user.postal_code).await?)
}

fn arrondi_km(km: f64) -> f64 {
    (km * 10.0).round() / 10.0
}

#[derive(Deserialize)]
pub struct FeedQuery {
    pub page: Option<u32>,
}

/// Fil d'accueil : objets disponibles triés par proximité et fraîcheur.
/// Connecté, le fil est centré sur la commune du visiteur (ses propres objets
/// sont exclus) ; anonyme, seule la récence ordonne.
#[utoipa::path(
    get,
    path = "/feed",
    tag = "catalog",
    params(("page" = Option<u32>, Query, description = "Page (1 par défaut, 24 objets par page)")),
    responses((status = 200, description = "Fil paginé", body = FeedResponse))
)]
pub async fn feed(
    State(state): State<AppState>,
    viewer: Option<CurrentUser>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<FeedResponse>, ApiError> {
    let page = query.page.unwrap_or(1).clamp(1, 500);
    let coords = viewer_coords(&state, viewer.as_ref()).await?;

    // Une ligne de plus que la page pour savoir s'il en reste.
    let mut rows = infra::catalog_repo::list_feed_items(
        &state.pool,
        coords,
        viewer.as_ref().map(|v| v.user_id),
        FEED_PAGE_SIZE + 1,
        (i64::from(page) - 1) * FEED_PAGE_SIZE,
    )
    .await?;
    let has_more = rows.len() as i64 > FEED_PAGE_SIZE;
    rows.truncate(FEED_PAGE_SIZE as usize);

    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut cover_by_item: HashMap<Uuid, String> = HashMap::new();
    for photo in infra::catalog_repo::photos_for_items(&state.pool, &ids).await? {
        cover_by_item
            .entry(photo.item_id)
            .or_insert_with(|| state.photos.public_url(&photo.s3_key));
    }

    telemetry::track(
        &state,
        "feed_viewed",
        viewer.as_ref().map(|v| v.user_id),
        json!({
            "page": page,
            "logged_in": viewer.is_some(),
            "located": coords.is_some(),
            "items_count": rows.len(),
        }),
    )
    .await;

    Ok(Json(FeedResponse {
        items: rows
            .into_iter()
            .map(|row| FeedCard {
                photo_url: cover_by_item.remove(&row.id),
                id: row.id,
                title: row.title,
                condition: row.condition,
                value_cents: row.value_cents,
                city: row.city,
                distance_km: row.distance_km.map(arrondi_km),
                created_at: row.created_at,
            })
            .collect(),
        page,
        has_more,
    }))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub category_id: Option<i16>,
    pub condition: Option<String>,
    pub delivery: Option<String>,
    pub soulte: Option<bool>,
    pub max_km: Option<f64>,
    pub sort: Option<String>,
    pub page: Option<u32>,
}

/// Recherche d'objets : plein texte tolérant aux fautes + filtres combinables.
/// Le filtre distance et le tri distance supposent un visiteur connecté
/// (localisé par sa commune) — ignorés sinon.
#[utoipa::path(
    get,
    path = "/search",
    tag = "catalog",
    params(
        ("q" = Option<String>, Query, description = "Texte libre (fautes tolérées)"),
        ("category_id" = Option<i16>, Query, description = "Catégorie (les sous-catégories sont incluses)"),
        ("condition" = Option<String>, Query, description = "neuf, tres_bon_etat, bon_etat ou correct"),
        ("delivery" = Option<String>, Query, description = "main_propre ou envoi (les_deux satisfait les deux)"),
        ("soulte" = Option<bool>, Query, description = "true : uniquement les objets acceptant une soulte"),
        ("max_km" = Option<f64>, Query, description = "Distance maximale en km (visiteur connecté)"),
        ("sort" = Option<String>, Query, description = "pertinence (défaut), distance ou recence"),
        ("page" = Option<u32>, Query, description = "Page (1 par défaut, 24 résultats par page)")
    ),
    responses((status = 200, description = "Résultats paginés", body = SearchResponse))
)]
pub async fn search(
    State(state): State<AppState>,
    viewer: Option<CurrentUser>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, ApiError> {
    let page = query.page.unwrap_or(1).clamp(1, 500);
    let coords = viewer_coords(&state, viewer.as_ref()).await?;
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(120).collect::<String>());

    let condition = query
        .condition
        .filter(|c| ["neuf", "tres_bon_etat", "bon_etat", "correct"].contains(&c.as_str()));
    let delivery = query
        .delivery
        .filter(|d| ["main_propre", "envoi"].contains(&d.as_str()));
    let max_km = query.max_km.filter(|km| (1.0..=1000.0).contains(km));

    let sort = match query.sort.as_deref() {
        Some("distance") if coords.is_some() => infra::search::SearchSort::Distance,
        Some("recence") => infra::search::SearchSort::Recence,
        _ => infra::search::SearchSort::Pertinence,
    };

    let params = infra::search::SearchParams {
        query: q.clone(),
        category_id: query.category_id,
        condition,
        delivery_pref: delivery,
        accepts_soulte: query.soulte,
        max_km,
        viewer: coords,
        limit: FEED_PAGE_SIZE + 1,
        offset: (i64::from(page) - 1) * FEED_PAGE_SIZE,
    };
    let mut rows = state
        .search
        .search(&params, sort)
        .await
        .map_err(ApiError::internal)?;
    let has_more = rows.len() as i64 > FEED_PAGE_SIZE;
    rows.truncate(FEED_PAGE_SIZE as usize);

    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut cover_by_item: HashMap<Uuid, String> = HashMap::new();
    for photo in infra::catalog_repo::photos_for_items(&state.pool, &ids).await? {
        cover_by_item
            .entry(photo.item_id)
            .or_insert_with(|| state.photos.public_url(&photo.s3_key));
    }

    if page == 1 {
        telemetry::track(
            &state,
            "search_performed",
            viewer.as_ref().map(|v| v.user_id),
            json!({
                "query_length": q.as_deref().map(str::len).unwrap_or(0),
                "filters": {
                    "category_id": params.category_id,
                    "condition": params.condition,
                    "delivery": params.delivery_pref,
                    "soulte": params.accepts_soulte,
                    "max_km": params.max_km,
                    "sort": match sort {
                        infra::search::SearchSort::Pertinence => "pertinence",
                        infra::search::SearchSort::Distance => "distance",
                        infra::search::SearchSort::Recence => "recence",
                    },
                },
                "results_count": rows.len(),
                "logged_in": viewer.is_some(),
            }),
        )
        .await;
    }

    Ok(Json(SearchResponse {
        items: rows
            .into_iter()
            .map(|row| FeedCard {
                photo_url: cover_by_item.remove(&row.id),
                id: row.id,
                title: row.title,
                condition: row.condition,
                value_cents: row.value_cents,
                city: row.city,
                distance_km: row.distance_km.map(arrondi_km),
                created_at: row.created_at,
            })
            .collect(),
        page,
        has_more,
    }))
}

#[derive(Deserialize)]
pub struct ItemPublicQuery {
    pub source: Option<String>,
}

/// Fiche objet complète : objet, encart propriétaire, distance approximative.
/// Même règle de visibilité que `GET /items/{id}` : le propriétaire voit tout,
/// les autres uniquement `disponible` (404 sinon).
#[utoipa::path(
    get,
    path = "/items/{id}/public",
    tag = "catalog",
    params(
        ("id" = Uuid, Path, description = "Identifiant de l'objet"),
        ("source" = Option<String>, Query, description = "Provenance : feed, search, profile, favorites")
    ),
    responses(
        (status = 200, description = "Fiche objet", body = ItemDetailResponse),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn item_public(
    State(state): State<AppState>,
    viewer: Option<CurrentUser>,
    Path(id): Path<Uuid>,
    Query(query): Query<ItemPublicQuery>,
) -> Result<Json<ItemDetailResponse>, ApiError> {
    let introuvable = || ApiError::not_found("Cet objet n'existe pas (ou plus).");
    let item = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .ok_or_else(introuvable)?;
    let is_owner = viewer
        .as_ref()
        .map(|v| v.user_id == item.owner_id)
        .unwrap_or(false);
    if !is_owner && item.status != "disponible" {
        return Err(introuvable());
    }

    let owner = infra::auth_repo::find_user_by_id(&state.pool, item.owner_id)
        .await?
        .ok_or_else(introuvable)?;
    let city =
        infra::catalog_repo::commune_for_postal_code(&state.pool, &owner.postal_code).await?;

    let distance_km = if is_owner {
        None
    } else {
        let viewer_point = viewer_coords(&state, viewer.as_ref()).await?;
        let owner_point =
            infra::catalog_repo::commune_coords_for_postal_code(&state.pool, &owner.postal_code)
                .await?;
        match (viewer_point, owner_point) {
            (Some(a), Some(b)) => Some(arrondi_km(regles::haversine_km(a, b))),
            _ => None,
        }
    };

    let demande = query.source.as_deref().unwrap_or("direct");
    let source = if ["feed", "search", "profile", "favorites"].contains(&demande) {
        demande
    } else {
        "direct"
    };
    telemetry::track(
        &state,
        "item_viewed",
        viewer.as_ref().map(|v| v.user_id),
        json!({
            "item_id": item.id,
            "source": source,
            "viewer_logged_in": viewer.is_some(),
            "is_owner": is_owner,
        }),
    )
    .await;

    let favorites_count = infra::favorites_repo::favorites_count(&state.pool, item.id).await?;
    let is_favorited = match viewer.as_ref() {
        Some(v) => infra::favorites_repo::is_favorited(&state.pool, v.user_id, item.id).await?,
        None => false,
    };

    let photos = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    Ok(Json(ItemDetailResponse {
        item: item_response(&state, item, photos),
        owner: ItemOwnerResponse {
            pseudo: owner.pseudo,
            city,
            member_since: owner.created_at,
        },
        distance_km,
        is_owner,
        favorites_count,
        is_favorited,
    }))
}

/// Fiche d'un objet. Le propriétaire voit tout ; les autres uniquement
/// `disponible` (404 sinon — ne pas révéler un objet masqué).
#[utoipa::path(
    get,
    path = "/items/{id}",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    responses(
        (status = 200, description = "Objet", body = ItemResponse),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_item(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ItemResponse>, ApiError> {
    let introuvable = || ApiError::not_found("Cet objet n'existe pas (ou plus).");
    let item = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .ok_or_else(introuvable)?;
    let is_owner = user.map(|u| u.user_id == item.owner_id).unwrap_or(false);
    if !is_owner && item.status != "disponible" {
        return Err(introuvable());
    }
    let photos = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    Ok(Json(item_response(&state, item, photos)))
}

/// Profil public d'un troqueur : ville approximative, ancienneté, dressing
/// visible (objets disponibles uniquement — jamais les masqués, Gherkin F1.2).
#[utoipa::path(
    get,
    path = "/troqueurs/{pseudo}",
    tag = "catalog",
    params(("pseudo" = String, Path, description = "Pseudo du troqueur")),
    responses(
        (status = 200, description = "Profil public", body = PublicProfileResponse),
        (status = 404, description = "Troqueur inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn public_profile(
    State(state): State<AppState>,
    viewer: Option<CurrentUser>,
    Path(pseudo): Path<String>,
) -> Result<Json<PublicProfileResponse>, ApiError> {
    let owner = infra::auth_repo::find_user_by_pseudo(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas — ou a plié bagage."))?;

    let city =
        infra::catalog_repo::commune_for_postal_code(&state.pool, &owner.postal_code).await?;
    let stats = infra::review_repo::profile_stats(&state.pool, owner.id).await?;
    let reviews = infra::review_repo::published_reviews_for_user(&state.pool, owner.id).await?;
    let items = infra::catalog_repo::list_public_items_by_owner(&state.pool, owner.id).await?;
    let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
    let mut photos_by_item: HashMap<Uuid, Vec<infra::catalog_repo::ItemPhoto>> = HashMap::new();
    for photo in infra::catalog_repo::photos_for_items(&state.pool, &ids).await? {
        photos_by_item.entry(photo.item_id).or_default().push(photo);
    }

    telemetry::track(
        &state,
        "profile_viewed",
        viewer.map(|v| v.user_id),
        json!({
            "profile_user_id": telemetry::hash_user_id(&state, owner.id),
            "viewer_is_owner": viewer.map(|v| v.user_id == owner.id).unwrap_or(false),
            "viewer_logged_in": viewer.is_some(),
            "items_count": items.len(),
        }),
    )
    .await;

    Ok(Json(PublicProfileResponse {
        pseudo: owner.pseudo,
        city,
        member_since: owner.created_at,
        items: items
            .into_iter()
            .map(|item| {
                let photos = photos_by_item.remove(&item.id).unwrap_or_default();
                item_response(&state, item, photos)
            })
            .collect(),
        rating_avg: stats.rating_avg,
        reviews_count: stats.reviews_count,
        trades_finalized: stats.trades_finalized,
        avg_ship_days: stats.avg_ship_days,
        reviews: reviews
            .into_iter()
            .map(|r| crate::catalog::dto::PublicReviewResponse {
                rating: r.rating,
                comment: r.comment,
                reply: r.reply,
                reviewer_pseudo: r.reviewer_pseudo,
                published_at: r.published_at,
            })
            .collect(),
    }))
}

/// Mon dressing (tous statuts).
#[utoipa::path(
    get,
    path = "/me/items",
    tag = "catalog",
    responses((status = 200, description = "Mes objets", body = [ItemResponse]))
)]
pub async fn my_items(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<ItemResponse>>, ApiError> {
    let items = infra::catalog_repo::list_items_by_owner(&state.pool, user.user_id).await?;
    let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
    let mut photos_by_item: HashMap<Uuid, Vec<infra::catalog_repo::ItemPhoto>> = HashMap::new();
    for photo in infra::catalog_repo::photos_for_items(&state.pool, &ids).await? {
        photos_by_item.entry(photo.item_id).or_default().push(photo);
    }
    // Le propriétaire voit combien de cœurs chaque objet a reçus (F2.3).
    let counts: HashMap<Uuid, i64> = infra::favorites_repo::favorites_counts(&state.pool, &ids)
        .await?
        .into_iter()
        .collect();
    Ok(Json(
        items
            .into_iter()
            .map(|item| {
                let photos = photos_by_item.remove(&item.id).unwrap_or_default();
                let count = counts.get(&item.id).copied().unwrap_or(0);
                let mut response = item_response(&state, item, photos);
                response.favorites_count = Some(count);
                response
            })
            .collect(),
    ))
}

/// Modifier un objet (propriétaire uniquement).
#[utoipa::path(
    patch,
    path = "/items/{id}",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    request_body = UpdateItemRequest,
    responses(
        (status = 200, description = "Objet mis à jour", body = ItemResponse),
        (status = 400, description = "Champ ou transition invalide", body = crate::error::ErrorResponse),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateItemRequest>,
) -> Result<Json<ItemResponse>, ApiError> {
    let introuvable = || ApiError::not_found("Cet objet n'existe pas (ou plus).");
    valider_champs_objet(
        &body.title,
        &body.description,
        &body.condition,
        body.value_cents,
        &body.delivery_pref,
    )?;
    if !infra::catalog_repo::category_exists(&state.pool, body.category_id).await? {
        return Err(ApiError::bad_request(
            "categorie_inconnue",
            "Choisis une catégorie pour que ton objet soit trouvable.",
        ));
    }
    let existing = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .filter(|i| i.owner_id == user.user_id)
        .ok_or_else(introuvable)?;
    regles::transition_statut_autorisee(&existing.status, &body.status)
        .map_err(map_catalog_error)?;

    let exchange_wishes = body
        .exchange_wishes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(300).collect::<String>());
    let item = infra::catalog_repo::update_item(
        &state.pool,
        id,
        user.user_id,
        infra::catalog_repo::ItemUpdate {
            title: body.title.trim(),
            description: body.description.trim(),
            category_id: body.category_id,
            condition: &body.condition,
            value_cents: body.value_cents,
            delivery_pref: &body.delivery_pref,
            exchange_wishes: exchange_wishes.as_deref(),
            accepts_soulte: body.accepts_soulte.unwrap_or(existing.accepts_soulte),
            status: &body.status,
        },
    )
    .await?
    .ok_or_else(introuvable)?;

    let event = if item.status != existing.status && item.status == "masque" {
        "item_hidden"
    } else {
        "item_edited"
    };
    telemetry::track(
        &state,
        event,
        Some(user.user_id),
        json!({"item_id": item.id}),
    )
    .await;

    let photos = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    Ok(Json(item_response(&state, item, photos)))
}

/// Supprimer un objet (propriétaire uniquement). Les photos sont retirées
/// du stockage ; l'objet disparaît du dressing et des fiches publiques.
#[utoipa::path(
    delete,
    path = "/items/{id}",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    responses(
        (status = 204, description = "Objet supprimé"),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn delete_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // Un objet réservé ou troqué fait partie d'un troc : intouchable (F3.3).
    let existing = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .filter(|i| i.owner_id == user.user_id)
        .ok_or_else(|| ApiError::not_found("Cet objet n'existe pas (ou plus)."))?;
    if matches!(existing.status.as_str(), "reserve" | "troque") {
        return Err(ApiError::bad_request(
            "objet_reserve",
            "Cet objet fait partie d'un troc en cours — il ne peut pas être supprimé.",
        ));
    }
    let Some(removed_keys) =
        infra::catalog_repo::soft_delete_item(&state.pool, id, user.user_id).await?
    else {
        return Err(ApiError::not_found("Cet objet n'existe pas (ou plus)."));
    };
    for key in removed_keys {
        state.photos.delete_object(&key).await;
    }
    telemetry::track(
        &state,
        "item_deleted",
        Some(user.user_id),
        json!({"item_id": id}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Remplace la liste ordonnée des photos (réordonnancement, ajout, retrait).
#[utoipa::path(
    put,
    path = "/items/{id}/photos",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    request_body = ReplacePhotosRequest,
    responses(
        (status = 200, description = "Photos mises à jour", body = ItemResponse),
        (status = 400, description = "Liste invalide", body = crate::error::ErrorResponse),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn replace_photos(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReplacePhotosRequest>,
) -> Result<Json<ItemResponse>, ApiError> {
    let introuvable = || ApiError::not_found("Cet objet n'existe pas (ou plus).");
    regles::valider_nombre_photos(body.photos.len()).map_err(map_catalog_error)?;
    let item = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .filter(|i| i.owner_id == user.user_id)
        .ok_or_else(introuvable)?;

    // Chaque id doit être soit une photo déjà rattachée, soit un upload frais.
    let existing = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    let existing_by_id: HashMap<Uuid, &infra::catalog_repo::ItemPhoto> =
        existing.iter().map(|p| (p.photo_id, p)).collect();
    let fresh_ids: Vec<Uuid> = body
        .photos
        .iter()
        .filter(|id| !existing_by_id.contains_key(id))
        .copied()
        .collect();
    let fresh = resolve_uploads(&state, user.user_id, &fresh_ids).await?;
    let fresh_by_id: HashMap<Uuid, &infra::catalog_repo::PhotoUpload> =
        fresh.iter().map(|u| (u.photo_id, u)).collect();

    let mut ordered = Vec::with_capacity(body.photos.len());
    for photo_id in &body.photos {
        if let Some(photo) = existing_by_id.get(photo_id) {
            ordered.push((*photo_id, photo.s3_key.clone(), photo.content_type.clone()));
        } else if let Some(upload) = fresh_by_id.get(photo_id) {
            ordered.push((
                *photo_id,
                upload.s3_key.clone(),
                upload.content_type.clone(),
            ));
        }
    }

    let removed = infra::catalog_repo::replace_item_photos(&state.pool, item.id, &ordered).await?;
    for key in removed {
        state.photos.delete_object(&key).await;
    }

    let photos = infra::catalog_repo::photos_for_items(&state.pool, &[item.id]).await?;
    Ok(Json(item_response(&state, item, photos)))
}

// ————— Favoris (F2.3) —————

/// Mettre un objet en favori (idempotent). On ne met pas son propre objet
/// en favori, ni un objet invisible.
#[utoipa::path(
    put,
    path = "/items/{id}/favorite",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    responses(
        (status = 204, description = "Favori posé"),
        (status = 400, description = "Son propre objet", body = crate::error::ErrorResponse),
        (status = 404, description = "Objet introuvable", body = crate::error::ErrorResponse)
    )
)]
pub async fn favorite_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let item = infra::catalog_repo::get_item(&state.pool, id)
        .await?
        .filter(|i| i.status == "disponible")
        .ok_or_else(|| ApiError::not_found("Cet objet n'existe pas (ou plus)."))?;
    if item.owner_id == user.user_id {
        return Err(ApiError::bad_request(
            "objet_a_soi",
            "C'est ton objet — pas besoin de le mettre en favori.",
        ));
    }
    let nouveau = infra::favorites_repo::add_favorite(&state.pool, user.user_id, id).await?;
    if nouveau {
        telemetry::track(
            &state,
            "item_favorited",
            Some(user.user_id),
            json!({"item_id": id}),
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Retirer un favori (idempotent).
#[utoipa::path(
    delete,
    path = "/items/{id}/favorite",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "Identifiant de l'objet")),
    responses((status = 204, description = "Favori retiré"))
)]
pub async fn unfavorite_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let existait = infra::favorites_repo::remove_favorite(&state.pool, user.user_id, id).await?;
    if existait {
        telemetry::track(
            &state,
            "item_unfavorited",
            Some(user.user_id),
            json!({"item_id": id}),
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Ma page favoris : les objets encore disponibles, cœur le plus récent
/// d'abord — mêmes cartes que le fil.
#[utoipa::path(
    get,
    path = "/me/favorites",
    tag = "catalog",
    responses((status = 200, description = "Mes favoris", body = [FeedCard]))
)]
pub async fn my_favorites(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<FeedCard>>, ApiError> {
    let coords = viewer_coords(&state, Some(&user)).await?;
    let rows =
        infra::favorites_repo::list_favorite_items(&state.pool, user.user_id, coords).await?;

    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut cover_by_item: HashMap<Uuid, String> = HashMap::new();
    for photo in infra::catalog_repo::photos_for_items(&state.pool, &ids).await? {
        cover_by_item
            .entry(photo.item_id)
            .or_insert_with(|| state.photos.public_url(&photo.s3_key));
    }
    Ok(Json(
        rows.into_iter()
            .map(|row| FeedCard {
                photo_url: cover_by_item.remove(&row.id),
                id: row.id,
                title: row.title,
                condition: row.condition,
                value_cents: row.value_cents,
                city: row.city,
                distance_km: row.distance_km.map(arrondi_km),
                created_at: row.created_at,
            })
            .collect(),
    ))
}

// ————— Liste d'envies (F2.3) —————

/// Mes 3 lignes « ce que je cherche ».
#[utoipa::path(
    get,
    path = "/me/wishlist",
    tag = "me",
    responses((status = 200, description = "Liste d'envies", body = [WishlistEntryDto]))
)]
pub async fn my_wishlist(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<WishlistEntryDto>>, ApiError> {
    let entries = infra::favorites_repo::list_wishlist(&state.pool, user.user_id).await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|e| WishlistEntryDto {
                category_id: e.category_id,
                keywords: e.keywords,
            })
            .collect(),
    ))
}

/// Remplace la liste d'envies (0 à 3 lignes ; lignes vides ignorées).
#[utoipa::path(
    put,
    path = "/me/wishlist",
    tag = "me",
    request_body = UpdateWishlistRequest,
    responses(
        (status = 200, description = "Liste enregistrée", body = [WishlistEntryDto]),
        (status = 400, description = "Liste invalide", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_wishlist(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<UpdateWishlistRequest>,
) -> Result<Json<Vec<WishlistEntryDto>>, ApiError> {
    if body.entries.len() > 3 {
        return Err(ApiError::bad_request(
            "envies_trop_nombreuses",
            "Trois envies maximum — garde les plus importantes.",
        ));
    }
    let mut entries: Vec<(Option<i16>, String)> = Vec::new();
    for entry in &body.entries {
        let keywords = entry.keywords.trim().chars().take(120).collect::<String>();
        if keywords.is_empty() && entry.category_id.is_none() {
            continue; // ligne vide
        }
        if let Some(category_id) = entry.category_id {
            if !infra::catalog_repo::category_exists(&state.pool, category_id).await? {
                return Err(ApiError::bad_request(
                    "categorie_inconnue",
                    "Une des catégories choisies n'existe pas.",
                ));
            }
        }
        entries.push((entry.category_id, keywords));
    }
    infra::favorites_repo::replace_wishlist(&state.pool, user.user_id, &entries).await?;

    telemetry::track(
        &state,
        "wishlist_updated",
        Some(user.user_id),
        json!({"filled": entries.len()}),
    )
    .await;

    Ok(Json(
        entries
            .into_iter()
            .map(|(category_id, keywords)| WishlistEntryDto {
                category_id,
                keywords,
            })
            .collect(),
    ))
}
