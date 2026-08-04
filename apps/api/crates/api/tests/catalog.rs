//! Tests d'intégration F1.1 — publication, dressing, photos, statuts.

use api::AppState;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

type Emails = std::sync::Arc<std::sync::Mutex<Vec<infra::email::CapturedEmail>>>;

fn app(pool: PgPool) -> (Router, Emails) {
    let (state, emails) = AppState::for_tests(pool);
    (api::router(state), emails)
}

async fn call(app: &Router, request: Request<Body>) -> axum::response::Response {
    app.clone()
        .oneshot(request)
        .await
        .expect("appel du routeur")
}

fn request(
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
    cookies: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(cookies) = cookies {
        builder = builder.header(header::COOKIE, cookies);
    }
    let body = match body {
        Some(json) => Body::from(json.to_string()),
        None => Body::empty(),
    };
    builder.body(body).expect("requête")
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corps")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("JSON")
}

fn cookie_header(response: &axum::response::Response) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|raw| raw.split(';').next())
        .filter(|part| !part.ends_with('='))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Inscrit un utilisateur, vérifie son e-mail, retourne l'en-tête Cookie.
async fn verified_user(app: &Router, emails: &Emails, email: &str, pseudo: &str) -> String {
    let response = call(
        app,
        request(
            "POST",
            "/auth/signup",
            Some(serde_json::json!({
                "email": email, "password": "un-bon-mot-de-passe",
                "pseudo": pseudo, "postal_code": "44000"
            })),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let cookies = cookie_header(&response);

    let token: String = {
        let emails = emails.lock().expect("verrou");
        let text = &emails.last().expect("e-mail").text;
        let start = text.find("token=").expect("token") + "token=".len();
        text[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect()
    };
    let response = call(
        app,
        request(
            "GET",
            &format!("/auth/verify-email?token={token}"),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    cookies
}

/// Présigne `n` photos et retourne leurs ids (le mock marque les clés comme uploadées).
async fn presign(app: &Router, cookies: &str, n: usize) -> Vec<String> {
    let files: Vec<_> = (0..n)
        .map(|_| serde_json::json!({"content_type": "image/webp", "size": 200000}))
        .collect();
    let response = call(
        app,
        request(
            "POST",
            "/items/photos/presign",
            Some(serde_json::json!({"files": files})),
            Some(cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response)
        .await
        .as_array()
        .expect("tableau")
        .iter()
        .map(|p| p["photo_id"].as_str().expect("photo_id").to_string())
        .collect()
}

fn item_body(photos: &[String]) -> serde_json::Value {
    serde_json::json!({
        "title": "Poussette Yoyo",
        "description": "Très bon état, pliage une main, avec l'ombrelle.",
        "category_id": 31,
        "condition": "tres_bon_etat",
        "value_cents": 15000,
        "delivery_pref": "main_propre",
        "exchange_wishes": "Un vélo enfant",
        "photos": photos,
        "duration_seconds": 84
    })
}

// ————— Catégories —————

#[sqlx::test(migrations = "../../migrations")]
async fn arbre_des_categories_avec_fourchettes_heritees(pool: PgPool) {
    let (app, _) = app(pool);
    let json = body_json(call(&app, request("GET", "/categories", None, None)).await).await;
    let racines = json.as_array().expect("tableau");
    assert_eq!(racines.len(), 9);

    let enfants = racines
        .iter()
        .find(|c| c["slug"] == "enfants")
        .expect("racine enfants");
    assert_eq!(enfants["icon"], "baby");
    let poussettes = enfants["children"]
        .as_array()
        .expect("sous-catégories")
        .iter()
        .find(|c| c["slug"] == "poussettes-portage")
        .expect("poussettes")
        .clone();
    // Fourchette héritée de la racine « enfants ».
    assert_eq!(poussettes["value_min_cents"], 200);
    assert_eq!(poussettes["value_max_cents"], 25000);
}

// ————— Présignature —————

#[sqlx::test(migrations = "../../migrations")]
async fn presign_exige_un_email_verifie(pool: PgPool) {
    let (app, _) = app(pool);
    // Inscription SANS vérification.
    let response = call(
        &app,
        request(
            "POST",
            "/auth/signup",
            Some(serde_json::json!({
                "email": "nonverifie@exemple.fr", "password": "un-bon-mot-de-passe",
                "pseudo": "nonverifie", "postal_code": "44000"
            })),
            None,
        ),
    )
    .await;
    let cookies = cookie_header(&response);

    let response = call(
        &app,
        request(
            "POST",
            "/items/photos/presign",
            Some(serde_json::json!({"files": [{"content_type": "image/webp", "size": 1000}]})),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "email_non_verifie"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn presign_refuse_les_mauvais_fichiers(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;

    let response = call(
        &app,
        request(
            "POST",
            "/items/photos/presign",
            Some(serde_json::json!({"files": [{"content_type": "image/gif", "size": 1000}]})),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = call(
        &app,
        request(
            "POST",
            "/items/photos/presign",
            Some(serde_json::json!({"files": [{"content_type": "image/webp", "size": 99999999}]})),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ————— Publication (Gherkin F1.1) —————

#[sqlx::test(migrations = "../../migrations")]
async fn publication_complete_et_dressing(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;

    let photos = presign(&app, &cookies, 3).await;
    let response = call(
        &app,
        request("POST", "/items", Some(item_body(&photos)), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let item = body_json(response).await;
    assert_eq!(item["status"], "disponible");
    assert_eq!(item["photos"].as_array().expect("photos").len(), 3);
    // La première photo est la vignette (position 0, ordre d'envoi).
    assert_eq!(item["photos"][0]["position"], 0);
    assert_eq!(item["photos"][0]["photo_id"], photos[0].as_str());

    // L'objet apparaît dans le dressing.
    let dressing =
        body_json(call(&app, request("GET", "/me/items", None, Some(&cookies))).await).await;
    assert_eq!(dressing.as_array().expect("tableau").len(), 1);
    assert_eq!(dressing[0]["title"], "Poussette Yoyo");

    // Fiche publique accessible sans connexion.
    let id = item["id"].as_str().expect("id");
    let response = call(&app, request("GET", &format!("/items/{id}"), None, None)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn publication_impossible_sans_photo(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;

    let aucune: Vec<String> = Vec::new();
    let response = call(
        &app,
        request("POST", "/items", Some(item_body(&aucune)), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "photos_invalides"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn publication_refuse_la_photo_d_autrui(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies_camille = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;
    let photos_camille = presign(&app, &cookies_camille, 1).await;

    let cookies_robin = verified_user(&app, &emails, "robin@exemple.fr", "robin").await;
    let response = call(
        &app,
        request(
            "POST",
            "/items",
            Some(item_body(&photos_camille)),
            Some(&cookies_robin),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "photo_inconnue");
}

// ————— Profil public (Gherkin F1.2) —————

#[sqlx::test(migrations = "../../migrations")]
async fn dressing_public_masque_les_objets_masques(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;

    // 5 objets, dont 1 masqué ensuite.
    let mut ids = Vec::new();
    for _ in 0..5 {
        let photos = presign(&app, &cookies, 1).await;
        let item = body_json(
            call(
                &app,
                request("POST", "/items", Some(item_body(&photos)), Some(&cookies)),
            )
            .await,
        )
        .await;
        ids.push(item["id"].as_str().expect("id").to_string());
    }
    let aucune: Vec<String> = Vec::new();
    let mut update = item_body(&aucune);
    update.as_object_mut().expect("objet").remove("photos");
    update["status"] = "masque".into();
    let response = call(
        &app,
        request(
            "PATCH",
            &format!("/items/{}", ids[0]),
            Some(update),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Un visiteur anonyme voit 4 objets, jamais le masqué.
    let profil =
        body_json(call(&app, request("GET", "/troqueurs/camille", None, None)).await).await;
    assert_eq!(profil["pseudo"], "camille");
    assert_eq!(profil["city"], "Nantes"); // 44000 → commune la plus peuplée
    let visibles = profil["items"].as_array().expect("items");
    assert_eq!(visibles.len(), 4);
    assert!(visibles.iter().all(|i| i["id"] != ids[0].as_str()));

    // Pseudo inconnu → 404.
    let response = call(&app, request("GET", "/troqueurs/fantome", None, None)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ————— Suppression (F1.2) —————

#[sqlx::test(migrations = "../../migrations")]
async fn suppression_d_objet(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;
    let photos = presign(&app, &cookies, 2).await;
    let item = body_json(
        call(
            &app,
            request("POST", "/items", Some(item_body(&photos)), Some(&cookies)),
        )
        .await,
    )
    .await;
    let id = item["id"].as_str().expect("id");

    // Un autre utilisateur ne peut pas supprimer.
    let cookies_robin = verified_user(&app, &emails, "robin@exemple.fr", "robin").await;
    let response = call(
        &app,
        request(
            "DELETE",
            &format!("/items/{id}"),
            None,
            Some(&cookies_robin),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Le propriétaire supprime.
    let response = call(
        &app,
        request("DELETE", &format!("/items/{id}"), None, Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Disparu du dressing et de la fiche ; double suppression → 404.
    let dressing =
        body_json(call(&app, request("GET", "/me/items", None, Some(&cookies))).await).await;
    assert_eq!(dressing.as_array().expect("tableau").len(), 0);
    let response = call(
        &app,
        request("GET", &format!("/items/{id}"), None, Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = call(
        &app,
        request("DELETE", &format!("/items/{id}"), None, Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ————— Statuts et édition —————

#[sqlx::test(migrations = "../../migrations")]
async fn masquage_et_statuts_interdits(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;
    let photos = presign(&app, &cookies, 1).await;
    let item = body_json(
        call(
            &app,
            request("POST", "/items", Some(item_body(&photos)), Some(&cookies)),
        )
        .await,
    )
    .await;
    let id = item["id"].as_str().expect("id");

    let aucune: Vec<String> = Vec::new();
    let mut update = item_body(&aucune);
    update.as_object_mut().expect("objet").remove("photos");
    update["status"] = "masque".into();

    let response = call(
        &app,
        request(
            "PATCH",
            &format!("/items/{id}"),
            Some(update.clone()),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["status"], "masque");

    // Masqué : invisible pour les autres (404, pas 403).
    let response = call(&app, request("GET", &format!("/items/{id}"), None, None)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    // Mais visible du propriétaire.
    let response = call(
        &app,
        request("GET", &format!("/items/{id}"), None, Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // `reserve` n'est jamais posable manuellement.
    update["status"] = "reserve".into();
    let response = call(
        &app,
        request(
            "PATCH",
            &format!("/items/{id}"),
            Some(update),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "statut_interdit"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn reordonnancement_et_retrait_de_photos(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "camille@exemple.fr", "camille").await;
    let photos = presign(&app, &cookies, 3).await;
    let item = body_json(
        call(
            &app,
            request("POST", "/items", Some(item_body(&photos)), Some(&cookies)),
        )
        .await,
    )
    .await;
    let id = item["id"].as_str().expect("id");

    // Inverse l'ordre et retire la photo du milieu.
    let response = call(
        &app,
        request(
            "PUT",
            &format!("/items/{id}/photos"),
            Some(serde_json::json!({"photos": [photos[2], photos[0]]})),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let result = json["photos"].as_array().expect("photos");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["photo_id"], photos[2].as_str());
    assert_eq!(result[0]["position"], 0);
    assert_eq!(result[1]["photo_id"], photos[0].as_str());
}
