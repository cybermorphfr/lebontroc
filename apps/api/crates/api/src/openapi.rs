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
        crate::auth::handlers::export_my_data,
        crate::auth::handlers::delete_my_account,
        crate::admin::handlers::admin_search,
        crate::admin::handlers::admin_list_reports,
        crate::admin::handlers::admin_close_report,
        crate::admin::handlers::admin_list_audit,
        crate::admin::handlers::admin_kpis,
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
        crate::trade::handlers::create_proposal,
        crate::trade::handlers::my_proposals,
        crate::trade::handlers::get_proposal,
        crate::trade::handlers::refuse_proposal,
        crate::trade::handlers::accept_proposal,
        crate::trade::handlers::counter_proposal,
        crate::trade::handlers::get_trade,
        crate::trade::handlers::pay_trade,
        crate::trade::handlers::confirm_trade,
        crate::trade::handlers::cancel_trade,
        crate::trade::handlers::trade_relays,
        crate::trade::handlers::configure_shipping,
        crate::trade::handlers::drop_parcel,
        crate::trade::handlers::pickup_parcel,
        crate::trade::handlers::confirm_parcel,
        crate::trade::handlers::submit_review,
        crate::trade::handlers::reply_review,
        crate::dispute::handlers::open_dispute,
        crate::dispute::handlers::presign_dispute_photos,
        crate::dispute::handlers::respond_dispute,
        crate::dispute::handlers::create_report,
        crate::dispute::handlers::block_user,
        crate::dispute::handlers::unblock_user,
        crate::dispute::handlers::my_blocks,
        crate::dispute::handlers::admin_list_disputes,
        crate::dispute::handlers::admin_get_dispute,
        crate::dispute::handlers::admin_resolve_dispute,
        crate::dispute::handlers::admin_lift_sanctions,
        crate::notification::handlers::list_notifications,
        crate::notification::handlers::get_unread_count,
        crate::notification::handlers::mark_read,
        crate::notification::handlers::mark_all_read,
        crate::notification::handlers::get_email_prefs,
        crate::notification::handlers::put_email_prefs,
        crate::messaging::handlers::list_messages,
        crate::messaging::handlers::send_message,
        crate::messaging::handlers::mark_read,
        crate::messaging::handlers::my_conversations,
    ),
    tags(
        (name = "system", description = "Santé et méta de l'API"),
        (name = "auth", description = "Comptes, sessions et vérification e-mail"),
        (name = "me", description = "Profil de l'utilisateur connecté"),
        (name = "catalog", description = "Catégories, objets et photos"),
        (name = "trade", description = "Propositions de troc"),
        (name = "messaging", description = "Conversations et messages")
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
