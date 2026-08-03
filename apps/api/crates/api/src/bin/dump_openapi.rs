//! Imprime le contrat OpenAPI sur stdout.
//! Utilisé par la CI pour régénérer `packages/api-client/openapi.json`
//! et vérifier qu'il ne diverge pas du code.

use utoipa::OpenApi;

fn main() {
    println!(
        "{}",
        api::openapi::ApiDoc::openapi()
            .to_pretty_json()
            .expect("sérialisation du contrat OpenAPI")
    );
}
