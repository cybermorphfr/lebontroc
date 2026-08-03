//! Contrat OpenAPI — source de vérité entre l'API Rust et le front TypeScript.
//!
//! Le fichier `packages/api-client/openapi.json` committé est régénéré par
//! `cargo run --bin dump-openapi` ; la CI échoue s'il diverge du code.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Lebontroc API",
        description = "API de la plateforme de troc Lebontroc.",
        version = "0.1.0"
    ),
    paths(crate::health::health),
    tags(
        (name = "system", description = "Santé et méta de l'API")
    )
)]
pub struct ApiDoc;
