//! Test d'intégration du endpoint /health — base éphémère via `sqlx::test`.

use api::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../../migrations")]
async fn health_repond_ok_avec_base_disponible(pool: PgPool) {
    let (state, _emails) = AppState::for_tests(pool);
    let app = api::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("construction de la requête"),
        )
        .await
        .expect("appel du routeur");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("lecture du corps")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("corps JSON");

    assert_eq!(json["status"], "ok");
    assert_eq!(json["db"], "ok");
    assert_eq!(json["version"], "0.1.0+test");
}

#[sqlx::test(migrations = "../../migrations")]
async fn openapi_est_servi(pool: PgPool) {
    let (state, _emails) = AppState::for_tests(pool);
    let app = api::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .expect("construction de la requête"),
        )
        .await
        .expect("appel du routeur");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("lecture du corps")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("corps JSON");

    assert_eq!(json["info"]["title"], "Lebontroc API");
    assert!(json["paths"]["/health"].is_object());
}
