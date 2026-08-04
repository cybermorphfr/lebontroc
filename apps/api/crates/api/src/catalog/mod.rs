//! Module catalogue : catégories, publication d'objets, dressing, photos.

pub mod dto;
pub mod handlers;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/categories", get(handlers::categories))
        .route("/feed", get(handlers::feed))
        .route("/search", get(handlers::search))
        .route("/items/{id}/public", get(handlers::item_public))
        .route("/items/photos/presign", post(handlers::presign_photos))
        .route("/items", post(handlers::create_item))
        .route(
            "/items/{id}",
            get(handlers::get_item)
                .patch(handlers::update_item)
                .delete(handlers::delete_item),
        )
        .route("/items/{id}/photos", put(handlers::replace_photos))
        .route(
            "/items/{id}/favorite",
            put(handlers::favorite_item).delete(handlers::unfavorite_item),
        )
        .route("/me/items", get(handlers::my_items))
        .route("/me/favorites", get(handlers::my_favorites))
        .route(
            "/me/wishlist",
            get(handlers::my_wishlist).put(handlers::update_wishlist),
        )
        .route("/troqueurs/{pseudo}", get(handlers::public_profile))
}
