//! Module catalogue : catégories, publication d'objets, dressing, photos.

pub mod dto;
pub mod handlers;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/categories", get(handlers::categories))
        .route("/items/photos/presign", post(handlers::presign_photos))
        .route("/items", post(handlers::create_item))
        .route(
            "/items/{id}",
            get(handlers::get_item).patch(handlers::update_item),
        )
        .route("/items/{id}/photos", put(handlers::replace_photos))
        .route("/me/items", get(handlers::my_items))
}
