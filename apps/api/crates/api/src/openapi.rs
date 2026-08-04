//! Contrat OpenAPI — source de vérité entre l'API Rust et le front TypeScript.
//!
//! Le fichier `packages/api-client/openapi.json` committé est régénéré par
//! `cargo run --bin dump-openapi` ; la CI le resynchronise s'il diverge.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Lebontroc API",
        description = "API de la plateforme de troc Lebontroc.",
        version = "0.1.0"
    ),
    paths(
        crate::health::health,
        crate::auth::handlers::signup,
        crate::auth::handlers::login,
        crate::auth::handlers::refresh,
        crate::auth::handlers::logout,
        crate::auth::handlers::verify_email,
        crate::auth::handlers::resend_verification,
        crate::auth::handlers::list_sessions,
        crate::auth::handlers::revoke_other_sessions,
        crate::auth::handlers::revoke_session,
        crate::auth::handlers::me,
        crate::auth::handlers::update_me,
        crate::auth::handlers::track_event,
        crate::catalog::handlers::categories,
        crate::catalog::handlers::presign_photos,
        crate::catalog::handlers::create_item,
        crate::catalog::handlers::get_item,
        crate::catalog::handlers::my_items,
        crate::catalog::handlers::update_item,
        crate::catalog::handlers::replace_photos,
        crate::catalog::handlers::delete_item,
        crate::catalog::handlers::public_profile,
        crate::catalog::handlers::feed,
        crate::catalog::handlers::item_public,
        crate::catalog::handlers::search,
        crate::catalog::handlers::favorite_item,
        crate::catalog::handlers::unfavorite_item,
        crate::catalog::handlers::my_favorites,
        crate::catalog::handlers::my_wishlist,
        crate::catalog::handlers::update_wishlist,
    ),
    tags(
        (name = "system", description = "Santé et méta de l'API"),
        (name = "auth", description = "Comptes, sessions et vérification e-mail"),
        (name = "me", description = "Profil de l'utilisateur connecté"),
        (name = "catalog", description = "Catégories, objets et photos")
    ),
    modifiers(&CookieSecurity)
)]
pub struct ApiDoc;

/// Déclare l'authentification par cookie httpOnly dans le contrat.
struct CookieSecurity;

impl Modify for CookieSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "cookie_access",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-lbt_access"))),
        );
    }
}
