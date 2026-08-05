//! Tests d'intégration F1.1 — publication, dressing, photos, statuts.

use api::AppState;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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
    verified_user_at(app, emails, email, pseudo, "44000").await
}

/// Comme `verified_user`, avec un code postal choisi (tests de proximité F2.1).
async fn verified_user_at(
    app: &Router,
    emails: &Emails,
    email: &str,
    pseudo: &str,
    postal_code: &str,
) -> String {
    let response = call(
        app,
        request(
            "POST",
            "/auth/signup",
            Some(serde_json::json!({
                "email": email, "password": "un-bon-mot-de-passe",
                "pseudo": pseudo, "postal_code": postal_code
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

// ————— Fil d'accueil (F2.1) —————

/// Publie un objet avec un titre choisi, retourne son id.
async fn publish_titled(app: &Router, cookies: &str, title: &str) -> String {
    let photos = presign(app, cookies, 1).await;
    let mut body = item_body(&photos);
    body["title"] = serde_json::Value::String(title.to_string());
    let response = call(app, request("POST", "/items", Some(body), Some(cookies))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string()
}

fn feed_titles(json: &serde_json::Value) -> Vec<String> {
    json["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["title"].as_str().expect("title").to_string())
        .collect()
}

#[sqlx::test(migrations = "../../migrations")]
async fn le_fil_privilegie_le_local(pool: PgPool) {
    let (app, emails) = app(pool);

    // L'objet proche est publié AVANT le lointain : la récence favoriserait le
    // lointain, la distance doit quand même faire gagner le proche (Gherkin F2.1).
    let nantes = verified_user_at(&app, &emails, "nantes@exemple.fr", "pronantes", "44300").await;
    publish_titled(&app, &nantes, "Lampe proche").await;
    let paris = verified_user_at(&app, &emails, "paris@exemple.fr", "proparis", "75001").await;
    publish_titled(&app, &paris, "Lampe lointaine").await;

    let viewer = verified_user_at(&app, &emails, "viewer@exemple.fr", "regardeur", "44000").await;
    let json = body_json(call(&app, request("GET", "/feed", None, Some(&viewer))).await).await;
    assert_eq!(feed_titles(&json), vec!["Lampe proche", "Lampe lointaine"]);
    assert_eq!(json["has_more"], false);

    let proche = &json["items"][0];
    assert_eq!(proche["city"], "Nantes");
    // 44000 et 44300 sont tous deux à Nantes : distance quasi nulle.
    assert!(proche["distance_km"].as_f64().expect("distance") < 20.0);
    let lointaine = &json["items"][1];
    let d = lointaine["distance_km"].as_f64().expect("distance");
    assert!((250.0..450.0).contains(&d), "Paris à {d} km de Nantes ?");

    // Anonyme : pas de point de vue, la récence ordonne et la distance est absente.
    let json = body_json(call(&app, request("GET", "/feed", None, None)).await).await;
    assert_eq!(feed_titles(&json), vec!["Lampe lointaine", "Lampe proche"]);
    assert!(json["items"][0]["distance_km"].is_null());
}

#[sqlx::test(migrations = "../../migrations")]
async fn le_fil_exclut_mes_objets_les_masques_et_les_supprimes(pool: PgPool) {
    let (app, emails) = app(pool);
    let moi = verified_user(&app, &emails, "moi@exemple.fr", "moimeme").await;
    publish_titled(&app, &moi, "Mon propre objet").await;

    let autre = verified_user_at(&app, &emails, "autre@exemple.fr", "lautre", "44300").await;
    publish_titled(&app, &autre, "Objet visible").await;
    let masque = publish_titled(&app, &autre, "Objet masqué").await;
    let supprime = publish_titled(&app, &autre, "Objet supprimé").await;

    let mut body = item_body(&[]);
    body["title"] = serde_json::Value::String("Objet masqué".into());
    body.as_object_mut().expect("objet").remove("photos");
    body["status"] = serde_json::Value::String("masque".into());
    let response = call(
        &app,
        request(
            "PATCH",
            &format!("/items/{masque}"),
            Some(body),
            Some(&autre),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = call(
        &app,
        request("DELETE", &format!("/items/{supprime}"), None, Some(&autre)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Connecté : je ne vois ni mes objets, ni les masqués, ni les supprimés.
    let json = body_json(call(&app, request("GET", "/feed", None, Some(&moi))).await).await;
    assert_eq!(feed_titles(&json), vec!["Objet visible"]);
    // La carte porte la photo de couverture.
    assert!(json["items"][0]["photo_url"]
        .as_str()
        .expect("cover")
        .contains("items/"));

    // Anonyme : tout objet disponible, y compris le mien.
    let json = body_json(call(&app, request("GET", "/feed", None, None)).await).await;
    assert_eq!(
        feed_titles(&json),
        vec!["Objet visible", "Mon propre objet"]
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn le_fil_pagine_par_24(pool: PgPool) {
    let (app, emails) = app(pool);
    let vendeur = verified_user(&app, &emails, "prolixe@exemple.fr", "prolixe").await;
    for i in 0..25 {
        publish_titled(&app, &vendeur, &format!("Objet numéro {i:02}")).await;
    }

    let json = body_json(call(&app, request("GET", "/feed?page=1", None, None)).await).await;
    assert_eq!(json["items"].as_array().expect("items").len(), 24);
    assert_eq!(json["has_more"], true);
    let json = body_json(call(&app, request("GET", "/feed?page=2", None, None)).await).await;
    assert_eq!(json["items"].as_array().expect("items").len(), 1);
    assert_eq!(json["has_more"], false);
}

// ————— Fiche publique (F2.1) —————

#[sqlx::test(migrations = "../../migrations")]
async fn fiche_publique_avec_proprietaire_et_distance(pool: PgPool) {
    let (app, emails) = app(pool);
    let paris = verified_user_at(&app, &emails, "prop@exemple.fr", "parigot", "75001").await;
    let id = publish_titled(&app, &paris, "Vélo pliant").await;

    // Visiteur nantais : ville et distance approximative, jamais le code postal.
    let viewer = verified_user_at(&app, &emails, "visiteur@exemple.fr", "visiteur", "44000").await;
    let response = call(
        &app,
        request(
            "GET",
            &format!("/items/{id}/public?source=feed"),
            None,
            Some(&viewer),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["item"]["title"], "Vélo pliant");
    assert_eq!(json["owner"]["pseudo"], "parigot");
    assert_eq!(json["owner"]["city"], "Paris");
    assert_eq!(json["is_owner"], false);
    let d = json["distance_km"].as_f64().expect("distance");
    assert!((250.0..450.0).contains(&d), "Paris à {d} km de Nantes ?");
    assert!(!json.to_string().contains("75001"), "code postal exposé");

    // Propriétaire : is_owner, pas de distance.
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/items/{id}/public"), None, Some(&paris)),
        )
        .await,
    )
    .await;
    assert_eq!(json["is_owner"], true);
    assert!(json["distance_km"].is_null());

    // Anonyme : fiche visible, distance absente.
    let response = call(
        &app,
        request("GET", &format!("/items/{id}/public"), None, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert!(json["distance_km"].is_null());
    assert_eq!(json["owner"]["city"], "Paris");
}

#[sqlx::test(migrations = "../../migrations")]
async fn fiche_publique_dun_objet_masque_introuvable_pour_les_autres(pool: PgPool) {
    let (app, emails) = app(pool);
    let owner = verified_user(&app, &emails, "cachotier@exemple.fr", "cachotier").await;
    let id = publish_titled(&app, &owner, "Objet discret").await;

    let mut body = item_body(&[]);
    body.as_object_mut().expect("objet").remove("photos");
    body["status"] = serde_json::Value::String("masque".into());
    let response = call(
        &app,
        request("PATCH", &format!("/items/{id}"), Some(body), Some(&owner)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = call(
        &app,
        request("GET", &format!("/items/{id}/public"), None, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    // Le propriétaire, lui, voit sa fiche masquée.
    let response = call(
        &app,
        request("GET", &format!("/items/{id}/public"), None, Some(&owner)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

// ————— Recherche (F2.2) —————

/// Publie un objet en fixant titre + catégorie + remise (+ soulte).
async fn publish_custom(
    app: &Router,
    cookies: &str,
    title: &str,
    category_id: i16,
    delivery: &str,
    soulte: bool,
) -> String {
    let photos = presign(app, cookies, 1).await;
    let mut body = item_body(&photos);
    body["title"] = serde_json::Value::String(title.to_string());
    body["category_id"] = serde_json::json!(category_id);
    body["delivery_pref"] = serde_json::Value::String(delivery.to_string());
    body["accepts_soulte"] = serde_json::Value::Bool(soulte);
    let response = call(app, request("POST", "/items", Some(body), Some(cookies))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string()
}

fn search_titles(json: &serde_json::Value) -> Vec<String> {
    json["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["title"].as_str().expect("title").to_string())
        .collect()
}

#[sqlx::test(migrations = "../../migrations")]
async fn la_recherche_tolere_les_fautes(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "vendeur@exemple.fr", "vendeur").await;
    publish_titled(&app, &cookies, "Poussette Yoyo").await;

    // Gherkin F2.2 : « pousette » trouve « Poussette Yoyo ».
    let json = body_json(call(&app, request("GET", "/search?q=pousette", None, None)).await).await;
    assert_eq!(search_titles(&json), vec!["Poussette Yoyo"]);

    // La forme exacte fonctionne évidemment aussi (FTS français).
    let json = body_json(call(&app, request("GET", "/search?q=poussette", None, None)).await).await;
    assert_eq!(search_titles(&json), vec!["Poussette Yoyo"]);

    // Un terme sans rapport ne renvoie rien.
    let json = body_json(call(&app, request("GET", "/search?q=aquarium", None, None)).await).await;
    assert!(search_titles(&json).is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn les_filtres_combines_sont_tous_respectes(pool: PgPool) {
    let (app, emails) = app(pool);

    // L'id de la racine « enfants » vient du référentiel, pas d'un magic number.
    let categories = body_json(call(&app, request("GET", "/categories", None, None)).await).await;
    let enfants = categories
        .as_array()
        .expect("racines")
        .iter()
        .find(|c| c["slug"] == "enfants")
        .expect("racine enfants")["id"]
        .as_i64()
        .expect("id");

    let nantes = verified_user_at(&app, &emails, "n@exemple.fr", "nantais", "44300").await;
    publish_custom(&app, &nantes, "Poussette proche", 31, "main_propre", true).await;
    publish_custom(&app, &nantes, "Poussette en envoi", 31, "envoi", true).await;
    let paris = verified_user_at(&app, &emails, "p@exemple.fr", "parisien", "75001").await;
    publish_custom(&app, &paris, "Poussette lointaine", 31, "main_propre", true).await;

    // Gherkin F2.2 : « enfants » + « moins de 10 km » + « main propre ».
    let viewer = verified_user_at(&app, &emails, "v@exemple.fr", "chercheur", "44000").await;
    let uri = format!("/search?category_id={enfants}&max_km=10&delivery=main_propre");
    let json = body_json(call(&app, request("GET", &uri, None, Some(&viewer))).await).await;
    assert_eq!(search_titles(&json), vec!["Poussette proche"]);

    // Un objet « les_deux » satisfait le filtre « main_propre ».
    publish_custom(&app, &nantes, "Poussette flexible", 31, "les_deux", true).await;
    let json = body_json(call(&app, request("GET", &uri, None, Some(&viewer))).await).await;
    let titres = search_titles(&json);
    assert!(titres.contains(&"Poussette flexible".to_string()));
    assert!(!titres.contains(&"Poussette en envoi".to_string()));
    assert!(!titres.contains(&"Poussette lointaine".to_string()));

    // Anonyme : le filtre distance est ignoré (pas de point de vue).
    let json = body_json(call(&app, request("GET", "/search?max_km=10", None, None)).await).await;
    assert_eq!(json["items"].as_array().expect("items").len(), 4);
}

#[sqlx::test(migrations = "../../migrations")]
async fn le_filtre_soulte_et_les_tris(pool: PgPool) {
    let (app, emails) = app(pool);
    let nantes = verified_user_at(&app, &emails, "n2@exemple.fr", "nantais2", "44300").await;
    publish_custom(&app, &nantes, "Objet sans argent", 31, "les_deux", false).await;
    let paris = verified_user_at(&app, &emails, "p2@exemple.fr", "parisien2", "75001").await;
    publish_custom(&app, &paris, "Objet avec soulte", 31, "les_deux", true).await;

    // Filtre « accepte une soulte ».
    let json = body_json(call(&app, request("GET", "/search?soulte=true", None, None)).await).await;
    assert_eq!(search_titles(&json), vec!["Objet avec soulte"]);

    // La fiche expose le refus de soulte.
    let json = body_json(call(&app, request("GET", "/search?q=argent", None, None)).await).await;
    let id = json["items"][0]["id"].as_str().expect("id").to_string();
    let fiche = body_json(
        call(
            &app,
            request("GET", &format!("/items/{id}/public"), None, None),
        )
        .await,
    )
    .await;
    assert_eq!(fiche["item"]["accepts_soulte"], false);

    // Tri récence : le plus récent d'abord, quel que soit l'éloignement.
    let viewer = verified_user_at(&app, &emails, "v2@exemple.fr", "chercheur2", "44000").await;
    let json = body_json(
        call(
            &app,
            request("GET", "/search?sort=recence", None, Some(&viewer)),
        )
        .await,
    )
    .await;
    assert_eq!(
        search_titles(&json),
        vec!["Objet avec soulte", "Objet sans argent"]
    );

    // Tri distance : le nantais d'abord pour un chercheur nantais.
    let json = body_json(
        call(
            &app,
            request("GET", "/search?sort=distance", None, Some(&viewer)),
        )
        .await,
    )
    .await;
    assert_eq!(
        search_titles(&json),
        vec!["Objet sans argent", "Objet avec soulte"]
    );
}

// ————— Favoris et liste d'envies (F2.3) —————

#[sqlx::test(migrations = "../../migrations")]
async fn favori_conserve_compteur_et_idempotence(pool: PgPool) {
    let (app, emails) = app(pool);
    let owner = verified_user(&app, &emails, "proprio@exemple.fr", "proprio").await;
    let id = publish_titled(&app, &owner, "Mobile en bois").await;
    let fan = verified_user_at(&app, &emails, "fan@exemple.fr", "fandetroc", "44300").await;

    // Poser deux fois le cœur = un seul favori (idempotent).
    for _ in 0..2 {
        let response = call(
            &app,
            request("PUT", &format!("/items/{id}/favorite"), None, Some(&fan)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    // Gherkin F2.3 : le favori est conservé quand je reviens.
    let json = body_json(call(&app, request("GET", "/me/favorites", None, Some(&fan))).await).await;
    let titres: Vec<&str> = json
        .as_array()
        .expect("favoris")
        .iter()
        .map(|i| i["title"].as_str().expect("titre"))
        .collect();
    assert_eq!(titres, vec!["Mobile en bois"]);

    // Le propriétaire voit son compteur incrémenté — sans savoir qui.
    let fiche = body_json(
        call(
            &app,
            request("GET", &format!("/items/{id}/public"), None, Some(&owner)),
        )
        .await,
    )
    .await;
    assert_eq!(fiche["favorites_count"], 1);
    assert!(!fiche.to_string().contains("fandetroc"));

    // Le fan voit l'état de son cœur ; son dressing à lui n'est pas concerné.
    let fiche = body_json(
        call(
            &app,
            request("GET", &format!("/items/{id}/public"), None, Some(&fan)),
        )
        .await,
    )
    .await;
    assert_eq!(fiche["is_favorited"], true);

    // Le compteur apparaît dans le dressing du propriétaire.
    let dressing =
        body_json(call(&app, request("GET", "/me/items", None, Some(&owner))).await).await;
    assert_eq!(dressing[0]["favorites_count"], 1);

    // Retrait (idempotent) : plus de favori, compteur à zéro.
    for _ in 0..2 {
        let response = call(
            &app,
            request("DELETE", &format!("/items/{id}/favorite"), None, Some(&fan)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    let json = body_json(call(&app, request("GET", "/me/favorites", None, Some(&fan))).await).await;
    assert!(json.as_array().expect("favoris").is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn pas_de_favori_sur_son_propre_objet_ni_sur_un_masque(pool: PgPool) {
    let (app, emails) = app(pool);
    let owner = verified_user(&app, &emails, "ego@exemple.fr", "egotroc").await;
    let id = publish_titled(&app, &owner, "Objet chéri").await;

    let response = call(
        &app,
        request("PUT", &format!("/items/{id}/favorite"), None, Some(&owner)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "objet_a_soi");

    // Un objet masqué disparaît des favoris existants et refuse les nouveaux.
    let fan = verified_user_at(&app, &emails, "fan2@exemple.fr", "fan2", "44300").await;
    let response = call(
        &app,
        request("PUT", &format!("/items/{id}/favorite"), None, Some(&fan)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let mut body = item_body(&[]);
    body.as_object_mut().expect("objet").remove("photos");
    body["status"] = serde_json::Value::String("masque".into());
    let response = call(
        &app,
        request("PATCH", &format!("/items/{id}"), Some(body), Some(&owner)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(call(&app, request("GET", "/me/favorites", None, Some(&fan))).await).await;
    assert!(json.as_array().expect("favoris").is_empty());
    let response = call(
        &app,
        request("PUT", &format!("/items/{id}/favorite"), None, Some(&fan)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn liste_denvies_enregistree_et_bornee(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = verified_user(&app, &emails, "envie@exemple.fr", "envieux").await;

    // Vide au départ.
    let json =
        body_json(call(&app, request("GET", "/me/wishlist", None, Some(&cookies))).await).await;
    assert!(json.as_array().expect("envies").is_empty());

    // Deux lignes + une vide (ignorée).
    let body = serde_json::json!({"entries": [
        {"category_id": 31, "keywords": "poussette yoyo"},
        {"category_id": null, "keywords": "vélo 16 pouces"},
        {"category_id": null, "keywords": "   "}
    ]});
    let response = call(
        &app,
        request("PUT", "/me/wishlist", Some(body), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json =
        body_json(call(&app, request("GET", "/me/wishlist", None, Some(&cookies))).await).await;
    let envies = json.as_array().expect("envies");
    assert_eq!(envies.len(), 2);
    assert_eq!(envies[0]["keywords"], "poussette yoyo");
    assert_eq!(envies[0]["category_id"], 31);
    assert_eq!(envies[1]["keywords"], "vélo 16 pouces");

    // Plus de 3 lignes → refus.
    let body = serde_json::json!({"entries": [
        {"keywords": "a", "category_id": null}, {"keywords": "b", "category_id": null},
        {"keywords": "c", "category_id": null}, {"keywords": "d", "category_id": null}
    ]});
    let response = call(
        &app,
        request("PUT", "/me/wishlist", Some(body), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Catégorie inconnue → refus.
    let body = serde_json::json!({"entries": [{"category_id": 9999, "keywords": "x"}]});
    let response = call(
        &app,
        request("PUT", "/me/wishlist", Some(body), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ————— Propositions de troc (F3.1) —————

/// Publie un objet avec un titre et une valeur choisis, retourne son id.
async fn publish_valued(app: &Router, cookies: &str, title: &str, value_cents: i32) -> String {
    let photos = presign(app, cookies, 1).await;
    let mut body = item_body(&photos);
    body["title"] = serde_json::Value::String(title.to_string());
    body["value_cents"] = serde_json::json!(value_cents);
    let response = call(app, request("POST", "/items", Some(body), Some(cookies))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposition_multi_objets_avec_soulte(pool: PgPool) {
    let (app, emails) = app(pool);
    // Gherkin F3.1 : ma console 120 € + 30 € contre son vélo 150 €.
    let alice = verified_user_at(&app, &emails, "alice@exemple.fr", "alicetroc", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo de ville", 15_000).await;
    let bob = verified_user(&app, &emails, "bob@exemple.fr", "bobtroc").await;
    let console = publish_valued(&app, &bob, "Console rétro", 12_000).await;

    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [console],
                "requested_item_ids": [velo],
                "cash_cents": 3000,
                "cash_direction": "du_proposant",
                "message": "Ma console contre ton vélo ?"
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    assert_eq!(json["status"], "envoyee");
    assert_eq!(json["cash_cents"], 3000);
    assert_eq!(json["cash_direction"], "du_proposant");
    assert_eq!(json["offered"][0]["title"], "Console rétro");
    assert_eq!(json["requested"][0]["title"], "Vélo de ville");
    assert_eq!(json["recipient_pseudo"], "alicetroc");

    // Boîtes : reçue chez Alice, envoyée chez Bob.
    let recues =
        body_json(call(&app, request("GET", "/me/proposals", None, Some(&alice))).await).await;
    assert_eq!(recues.as_array().expect("liste").len(), 1);
    assert_eq!(recues[0]["is_proposer"], false);
    let envoyees = body_json(
        call(
            &app,
            request("GET", "/me/proposals?box=envoyees", None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(envoyees.as_array().expect("liste").len(), 1);
    assert_eq!(envoyees[0]["is_proposer"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn plafond_de_soulte_50_pour_cent(pool: PgPool) {
    let (app, emails) = app(pool);
    // Gherkin : meilleur objet 100 € → plafond 50 €.
    let alice = verified_user_at(&app, &emails, "a2@exemple.fr", "alice2", "44300").await;
    let cible = publish_valued(&app, &alice, "Meilleur objet", 10_000).await;
    let bob = verified_user(&app, &emails, "b2@exemple.fr", "bob2").await;
    let mien = publish_valued(&app, &bob, "Mon objet", 8_000).await;

    let proposition = |cash: i32| {
        serde_json::json!({
            "offered_item_ids": [mien],
            "requested_item_ids": [cible],
            "cash_cents": cash,
            "cash_direction": "du_proposant"
        })
    };
    let response = call(
        &app,
        request("POST", "/proposals", Some(proposition(6_000)), Some(&bob)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert_eq!(json["error"]["code"], "soulte_trop_haute");
    assert!(json["error"]["message"]
        .as_str()
        .expect("message")
        .contains("50 €"));

    // Pile au plafond : accepté.
    let response = call(
        &app,
        request("POST", "/proposals", Some(proposition(5_000)), Some(&bob)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn vue_puis_refus_par_le_destinataire(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "a3@exemple.fr", "alice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo pliable", 15_000).await;
    let bob = verified_user(&app, &emails, "b3@exemple.fr", "bob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu de société", 4_000).await;

    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [jeu], "requested_item_ids": [velo], "cash_cents": 0
            })),
            Some(&bob),
        ),
    )
    .await;
    let id = body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string();

    // Un tiers ne voit rien.
    let carole = verified_user(&app, &emails, "c3@exemple.fr", "carole3").await;
    let response = call(
        &app,
        request("GET", &format!("/proposals/{id}"), None, Some(&carole)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Le proposant consulte : toujours « envoyée ».
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/proposals/{id}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "envoyee");

    // Première ouverture par la destinataire : passe à « vue ».
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/proposals/{id}"), None, Some(&alice)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "vue");

    // Le proposant ne peut pas refuser ; la destinataire oui, une seule fois.
    let response = call(
        &app,
        request("POST", &format!("/proposals/{id}/refuse"), None, Some(&bob)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = body_json(
        call(
            &app,
            request(
                "POST",
                &format!("/proposals/{id}/refuse"),
                None,
                Some(&alice),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "refusee");
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/refuse"),
            None,
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn compositions_interdites(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "a4@exemple.fr", "alice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo cargo", 15_000).await;
    let bob = verified_user(&app, &emails, "b4@exemple.fr", "bob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu d'échecs", 4_000).await;

    // Troc avec soi-même.
    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [jeu], "requested_item_ids": [jeu], "cash_cents": 0
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Offrir un objet qui n'est pas à moi (Bob offre le vélo d'Alice).
    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [velo], "requested_item_ids": [jeu], "cash_cents": 0
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "objet_indisponible"
    );

    // Aucun objet offert.
    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [], "requested_item_ids": [velo], "cash_cents": 0
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "objets_manquants"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn expiration_notifie_le_proposant(pool: PgPool) {
    let (state, emails) = api::AppState::for_tests(pool.clone());
    let app = api::router(state.clone());

    let alice = verified_user_at(&app, &emails, "a5@exemple.fr", "alice5", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo ancien", 15_000).await;
    let bob = verified_user(&app, &emails, "b5@exemple.fr", "bob5").await;
    let jeu = publish_valued(&app, &bob, "Jeu vidéo", 4_000).await;

    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [jeu], "requested_item_ids": [velo], "cash_cents": 0
            })),
            Some(&bob),
        ),
    )
    .await;
    let id = body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string();

    // Sans retard, rien n'expire.
    assert_eq!(api::trade::handlers::expire_and_notify(&state).await, 0);

    // On antidate l'échéance : la proposition expire et Bob est prévenu.
    sqlx::query("UPDATE proposals SET expires_at = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(api::trade::handlers::expire_and_notify(&state).await, 1);

    let json = body_json(
        call(
            &app,
            request("GET", &format!("/proposals/{id}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "expiree");

    {
        let emails = emails.lock().expect("verrou");
        let dernier = emails.last().expect("e-mail");
        assert_eq!(dernier.to, "b5@exemple.fr");
        assert!(dernier.subject.contains("expiré"));
        assert!(dernier.text.contains("alice5"));
    }

    // Une seconde passe n'expire rien de plus.
    assert_eq!(api::trade::handlers::expire_and_notify(&state).await, 0);
}

// ————— Messagerie (F3.2) —————

/// Crée une proposition simple entre deux utilisateurs, retourne son id.
async fn simple_proposal(app: &Router, offerer: &str, offered: &str, requested: &str) -> String {
    let response = call(
        app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [offered], "requested_item_ids": [requested], "cash_cents": 0
            })),
            Some(offerer),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn les_coordonnees_sont_masquees_avant_acceptation(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "m1@exemple.fr", "malice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo à négocier", 15_000).await;
    let bob = verified_user(&app, &emails, "m2@exemple.fr", "mbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu à offrir", 4_000).await;
    let id = simple_proposal(&app, &bob, &jeu, &velo).await;

    // Gherkin F3.2 : le numéro est masqué avant acceptation.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/messages"),
            Some(serde_json::json!({"body": "appelle-moi au 06 12 34 56 78"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    assert_eq!(json["redacted"], true);
    let body = json["body"].as_str().expect("body");
    assert!(!body.contains("06 12"), "numéro visible : {body}");

    // Après acceptation (posée en SQL — l'acceptation UI arrive en F3.3),
    // les coordonnées passent librement.
    sqlx::query("UPDATE proposals SET status = 'acceptee' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .expect("acceptation");
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/messages"),
            Some(serde_json::json!({"body": "super, mon numéro : 06 12 34 56 78"})),
            Some(&alice),
        ),
    )
    .await;
    let json = body_json(response).await;
    assert_eq!(json["redacted"], false);
    assert!(json["body"]
        .as_str()
        .expect("body")
        .contains("06 12 34 56 78"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn fil_lecture_et_compteur_de_non_lus(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "m3@exemple.fr", "malice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo bavard", 15_000).await;
    let bob = verified_user(&app, &emails, "m4@exemple.fr", "mbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu bavard", 4_000).await;
    let id = simple_proposal(&app, &bob, &jeu, &velo).await;

    for texte in ["Salut !", "Mon jeu contre ton vélo ?"] {
        let response = call(
            &app,
            request(
                "POST",
                &format!("/proposals/{id}/messages"),
                Some(serde_json::json!({"body": texte})),
                Some(&bob),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // Un tiers n'accède pas au fil.
    let carole = verified_user(&app, &emails, "m5@exemple.fr", "mcarole").await;
    let response = call(
        &app,
        request(
            "GET",
            &format!("/proposals/{id}/messages"),
            None,
            Some(&carole),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Alice voit 2 non-lus dans sa liste de conversations.
    let json = body_json(
        call(
            &app,
            request("GET", "/me/conversations", None, Some(&alice)),
        )
        .await,
    )
    .await;
    assert_eq!(json[0]["unread_count"], 2);
    assert_eq!(json[0]["last_message"], "Mon jeu contre ton vélo ?");
    assert_eq!(json[0]["last_is_mine"], false);

    // Elle lit : plus de non-lus, et Bob voit ses messages « lus ».
    let response = call(
        &app,
        request("POST", &format!("/proposals/{id}/read"), None, Some(&alice)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let json = body_json(
        call(
            &app,
            request("GET", "/me/conversations", None, Some(&alice)),
        )
        .await,
    )
    .await;
    assert_eq!(json[0]["unread_count"], 0);
    let json = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/proposals/{id}/messages"),
                None,
                Some(&bob),
            ),
        )
        .await,
    )
    .await;
    assert!(json[0]["read_at"].is_string());
    assert!(json[1]["read_at"].is_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn conversation_fermee_et_message_vide(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "m6@exemple.fr", "malice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo fermé", 15_000).await;
    let bob = verified_user(&app, &emails, "m7@exemple.fr", "mbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu fermé", 4_000).await;
    let id = simple_proposal(&app, &bob, &jeu, &velo).await;

    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/messages"),
            Some(serde_json::json!({"body": "   "})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "message_vide");

    // Refusée → conversation close.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/refuse"),
            None,
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/messages"),
            Some(serde_json::json!({"body": "trop tard ?"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "conversation_fermee"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn relance_apres_24_heures_sans_lecture(pool: PgPool) {
    let (state, emails) = api::AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "m8@exemple.fr", "malice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo patient", 15_000).await;
    let bob = verified_user(&app, &emails, "m9@exemple.fr", "mbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu patient", 4_000).await;
    let id = simple_proposal(&app, &bob, &jeu, &velo).await;

    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{id}/messages"),
            Some(serde_json::json!({"body": "Toujours partante ?"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Trop tôt : pas de relance.
    assert_eq!(api::messaging::handlers::remind_unread(&state).await, 0);

    // On antidate le message : Alice est relancée, une seule fois.
    sqlx::query("UPDATE messages SET created_at = now() - interval '25 hours'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(api::messaging::handlers::remind_unread(&state).await, 1);
    {
        let emails = emails.lock().expect("verrou");
        let dernier = emails.last().expect("e-mail");
        assert_eq!(dernier.to, "m8@exemple.fr");
        assert!(dernier.subject.contains("mbob4"));
    }
    assert_eq!(api::messaging::handlers::remind_unread(&state).await, 0);
}

// ————— Acceptation atomique et contre-proposition (F3.3) —————

#[sqlx::test(migrations = "../../migrations")]
async fn acceptation_reserve_invalide_les_concurrentes_et_notifie(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "t1@exemple.fr", "talice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo convoité", 15_000).await;
    let bob = verified_user(&app, &emails, "t2@exemple.fr", "tbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu de Bob", 4_000).await;
    let carole = verified_user_at(&app, &emails, "t3@exemple.fr", "tcarole1", "44300").await;
    let puzzle = publish_valued(&app, &carole, "Puzzle de Carole", 3_000).await;

    // Bob et Carole visent tous deux le vélo d'Alice.
    let p_bob = simple_proposal(&app, &bob, &jeu, &velo).await;
    let p_carole = simple_proposal(&app, &carole, &puzzle, &velo).await;

    // Alice accepte la proposition de Bob (main propre).
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{p_bob}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["status"], "acceptee");
    assert_eq!(json["trade"]["delivery_mode"], "main_propre");

    // Gherkin : tous les objets concernés passent en « réservé ».
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "reserve"));

    // Gherkin : la proposition de Carole devient caduque, Carole est notifiée.
    let json = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/proposals/{p_carole}"),
                None,
                Some(&carole),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "caduque");
    {
        let emails = emails.lock().expect("verrou");
        let dernier = emails.last().expect("e-mail");
        assert_eq!(dernier.to, "t3@exemple.fr");
        assert!(dernier.subject.contains("réservé"));
    }

    // Un objet réservé ne peut plus être supprimé ni proposé.
    let response = call(
        &app,
        request("DELETE", &format!("/items/{velo}"), None, Some(&alice)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "objet_reserve");
}

#[sqlx::test(migrations = "../../migrations")]
async fn course_entre_deux_acceptations_une_seule_gagne(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    // Alice possède le vélo ; Bob et Carole le veulent chacun de leur côté.
    let alice = verified_user_at(&app, &emails, "t4@exemple.fr", "talice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo unique", 15_000).await;
    let bob = verified_user(&app, &emails, "t5@exemple.fr", "tbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu unique", 4_000).await;
    let carole = verified_user_at(&app, &emails, "t6@exemple.fr", "tcarole2", "44300").await;
    let puzzle = publish_valued(&app, &carole, "Puzzle unique", 3_000).await;

    let p_bob = simple_proposal(&app, &bob, &jeu, &velo).await;
    let p_carole = simple_proposal(&app, &carole, &puzzle, &velo).await;

    // Gherkin : les deux acceptations partent au même instant — exactement
    // une aboutit, l'autre échoue proprement (409, message clair).
    let accept = |p: String| {
        let app = app.clone();
        let alice = alice.clone();
        async move {
            call(
                &app,
                request(
                    "POST",
                    &format!("/proposals/{p}/accept"),
                    Some(serde_json::json!({"delivery_mode": "main_propre"})),
                    Some(&alice),
                ),
            )
            .await
            .status()
        }
    };
    let (s1, s2) = tokio::join!(accept(p_bob.clone()), accept(p_carole.clone()));
    let statuts = [s1, s2];
    assert_eq!(
        statuts.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "exactement une acceptation doit réussir : {statuts:?}"
    );
    assert_eq!(
        statuts
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1,
        "l'autre doit échouer en 409 : {statuts:?}"
    );

    // Le vélo n'est réservé qu'une fois, un seul troc existe.
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM trades")
        .fetch_one(&pool)
        .await
        .expect("trades");
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn acceptation_idempotente_au_double_clic(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "t7@exemple.fr", "talice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo cliqué", 15_000).await;
    let bob = verified_user(&app, &emails, "t8@exemple.fr", "tbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu cliqué", 4_000).await;
    let id = simple_proposal(&app, &bob, &jeu, &velo).await;

    let mut trade_ids = Vec::new();
    for _ in 0..2 {
        let response = call(
            &app,
            request(
                "POST",
                &format!("/proposals/{id}/accept"),
                Some(serde_json::json!({"delivery_mode": "main_propre"})),
                Some(&alice),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        trade_ids.push(json["trade"]["id"].as_str().expect("trade id").to_string());
    }
    assert_eq!(trade_ids[0], trade_ids[1], "le même troc est renvoyé");
}

#[sqlx::test(migrations = "../../migrations")]
async fn contre_proposition_remplace_et_garde_la_conversation(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "t9@exemple.fr", "talice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo négocié", 15_000).await;
    let bob = verified_user(&app, &emails, "t10@exemple.fr", "tbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu négocié", 4_000).await;
    let old_id = simple_proposal(&app, &bob, &jeu, &velo).await;

    // Un échange a lieu dans la conversation d'origine.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{old_id}/messages"),
            Some(serde_json::json!({"body": "Intéressée, mais j'aimerais une soulte."})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Alice contre-propose : mêmes objets, 20 € à la charge de Bob.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{old_id}/counter"),
            Some(serde_json::json!({
                "offered_item_ids": [velo],
                "requested_item_ids": [jeu],
                "cash_cents": 2000,
                "cash_direction": "du_destinataire"
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    let new_id = json["id"].as_str().expect("id").to_string();
    assert_eq!(json["counter_of"], old_id.as_str());
    assert_eq!(json["proposer_pseudo"], "talice4");
    assert_eq!(json["recipient_pseudo"], "tbob4");

    // L'ancienne est « contre_proposee », chaînée vers la nouvelle, et ne
    // peut plus être ni refusée ni acceptée.
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/proposals/{old_id}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "contre_proposee");
    assert_eq!(json["superseded_by"], new_id.as_str());
    // Alice (destinataire de l'ancienne) ne peut plus l'accepter non plus.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{old_id}/accept"),
            Some(serde_json::json!({"delivery_mode": "envoi"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // La conversation a suivi la contre-proposition.
    let json = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/proposals/{new_id}/messages"),
                None,
                Some(&bob),
            ),
        )
        .await,
    )
    .await;
    let fils = json.as_array().expect("messages");
    assert_eq!(fils.len(), 1);
    assert!(fils[0]["body"].as_str().expect("body").contains("soulte"));

    // F4.2 : la contre AVEC soulte s'accepte — le troc naît en attente du
    // paiement de Bob (la soulte est à sa charge).
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{new_id}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["status"], "acceptee");
    assert_eq!(json["trade"]["status"], "attente_paiement");
}

// ————— Remise en main propre (F4.1) —————

/// Accepte une proposition en main propre et retourne l'id du troc.
async fn accepted_trade(app: &Router, recipient: &str, proposal_id: &str) -> String {
    let response = call(
        app,
        request(
            "POST",
            &format!("/proposals/{proposal_id}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(recipient),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn finalisation_croisee_par_codes(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "f1@exemple.fr", "falice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo à remettre", 15_000).await;
    let bob = verified_user(&app, &emails, "f2@exemple.fr", "fbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu à remettre", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = accepted_trade(&app, &alice, &proposal).await;

    // Chacun voit SON code (différents), aucun n'a encore confirmé.
    let vue_alice = body_json(
        call(
            &app,
            request("GET", &format!("/trades/{trade}"), None, Some(&alice)),
        )
        .await,
    )
    .await;
    let vue_bob = body_json(
        call(
            &app,
            request("GET", &format!("/trades/{trade}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    let code_alice = vue_alice["my_code"].as_str().expect("code").to_string();
    let code_bob = vue_bob["my_code"].as_str().expect("code").to_string();
    assert_eq!(code_alice.len(), 6);
    assert_ne!(code_alice, code_bob);
    assert_eq!(vue_alice["i_confirmed"], false);

    // Un tiers ne voit rien ; un mauvais code est rejeté.
    let carole = verified_user(&app, &emails, "f3@exemple.fr", "fcarole1").await;
    let response = call(
        &app,
        request("GET", &format!("/trades/{trade}"), None, Some(&carole)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": "000000"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "code_invalide");

    // Gherkin : chacun saisit le code de l'autre → finalisé, objets troqués.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": code_bob})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["i_confirmed"], true);
    assert_eq!(json["status"], "accepte");

    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": code_alice})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["status"], "finalise");

    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "troque"));
}

/// Envoie une proposition avec soulte et retourne son id.
async fn cash_proposal(
    app: &Router,
    proposer: &str,
    offered: &str,
    requested: &str,
    cash_cents: i32,
    cash_direction: &str,
) -> String {
    let response = call(
        app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [offered], "requested_item_ids": [requested],
                "cash_cents": cash_cents, "cash_direction": cash_direction
            })),
            Some(proposer),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"]
        .as_str()
        .expect("id")
        .to_string()
}

// (La garde bêta `envoi_indisponible` est tombée avec F4.3 — l'envoi
// croisé est couvert par les tests dédiés en fin de fichier.)

#[sqlx::test(migrations = "../../migrations")]
async fn annulation_dun_commun_accord(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "f6@exemple.fr", "falice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo annulé", 15_000).await;
    let bob = verified_user(&app, &emails, "f7@exemple.fr", "fbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu annulé", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = accepted_trade(&app, &alice, &proposal).await;

    // Première demande : en attente de l'autre.
    let json = body_json(
        call(
            &app,
            request(
                "POST",
                &format!("/trades/{trade}/cancel"),
                None,
                Some(&alice),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "accepte");
    assert_eq!(json["cancel_requested_by_me"], true);
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/trades/{trade}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["cancel_requested_by_other"], true);

    // L'autre confirme : annulé, objets libérés.
    let json = body_json(
        call(
            &app,
            request("POST", &format!("/trades/{trade}/cancel"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "annule");
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "disponible"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn rendez_vous_fantome_relance_puis_annulation(pool: PgPool) {
    let (state, emails) = api::AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "f8@exemple.fr", "falice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo fantôme", 15_000).await;
    let bob = verified_user(&app, &emails, "f9@exemple.fr", "fbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu fantôme", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = accepted_trade(&app, &alice, &proposal).await;

    use api::trade::handlers::MaintenanceReport;
    // Rien à faire tant que le troc est récent.
    assert_eq!(
        api::trade::handlers::maintain_trades(&state).await,
        MaintenanceReport::default()
    );

    // J+8 : relance des deux parties, une seule fois.
    sqlx::query("UPDATE trades SET created_at = now() - interval '8 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(
        api::trade::handlers::maintain_trades(&state).await,
        MaintenanceReport {
            reminded: 2,
            ..Default::default()
        }
    );
    assert_eq!(
        api::trade::handlers::maintain_trades(&state).await,
        MaintenanceReport::default()
    );
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails
            .last()
            .expect("e-mail")
            .subject
            .contains("rendez-vous"));
    }

    // Gherkin J+14 : annulé automatiquement, objets redevenus disponibles.
    sqlx::query("UPDATE trades SET created_at = now() - interval '15 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(
        api::trade::handlers::maintain_trades(&state).await,
        MaintenanceReport {
            cancelled: 1,
            ..Default::default()
        }
    );
    let json = body_json(
        call(
            &app,
            request("GET", &format!("/trades/{trade}"), None, Some(&bob)),
        )
        .await,
    )
    .await;
    assert_eq!(json["status"], "annule");
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "disponible"));
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.last().expect("e-mail").subject.contains("annulé"));
    }
}

// ————— Soulte séquestrée (F4.2) —————

/// GET du troc, en tant que `who`.
async fn trade_view(app: &Router, who: &str, trade: &str) -> serde_json::Value {
    body_json(
        call(
            app,
            request("GET", &format!("/trades/{trade}"), None, Some(who)),
        )
        .await,
    )
    .await
}

/// POST /pay avec un numéro de carte.
async fn pay(app: &Router, who: &str, trade: &str, card: &str) -> axum::response::Response {
    call(
        app,
        request(
            "POST",
            &format!("/trades/{trade}/pay"),
            Some(serde_json::json!({"card_number": card})),
            Some(who),
        ),
    )
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn soulte_parcours_complet(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "p1@exemple.fr", "palice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo à soulte", 15_000).await;
    let bob = verified_user(&app, &emails, "p2@exemple.fr", "pbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu à soulte", 4_000).await;
    let carole = verified_user(&app, &emails, "p3@exemple.fr", "pcarole1").await;
    let puzzle = publish_valued(&app, &carole, "Puzzle concurrent", 4_000).await;

    // Bob propose jeu + 20 € contre le vélo ; Carole vise le même vélo.
    let p_bob = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let p_carole = simple_proposal(&app, &carole, &puzzle, &velo).await;

    // Alice accepte : le troc naît en attente du paiement de Bob (24 h,
    // le payeur n'est pas l'accepteur).
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{p_bob}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let trade = body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string();

    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "attente_paiement");
    assert!(vue["my_code"].is_null(), "code masqué avant séquestre");
    let paiement = &vue["payment"];
    assert_eq!(paiement["status"], "en_attente");
    assert_eq!(paiement["amount_cents"], 2_000);
    assert_eq!(paiement["fees_cents"], 0);
    assert_eq!(paiement["net_cents"], 2_000);
    assert_eq!(paiement["i_am_payer"], false);
    assert_eq!(
        trade_view(&app, &bob, &trade).await["payment"]["i_am_payer"],
        true
    );
    // Bob (payeur absent au moment de l'acceptation) est prévenu par e-mail.
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.last().expect("e-mail").subject.contains("soulte"));
    }
    // Deadline ~24 h.
    let (minutes,): (f64,) = sqlx::query_as(
        "SELECT (EXTRACT(EPOCH FROM deadline - now()) / 60)::float8 FROM payments \
         WHERE trade_id = $1::uuid",
    )
    .bind(&trade)
    .fetch_one(&pool)
    .await
    .expect("deadline");
    assert!((23.0 * 60.0..=24.0 * 60.0).contains(&minutes), "{minutes}");

    // Pas de remise possible avant le séquestre.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": "123456"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "troc_clos");

    // La concurrente de Carole reste vivante tant que rien n'est payé.
    let vue_carole = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/proposals/{p_carole}"),
                None,
                Some(&carole),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(vue_carole["status"], "envoyee");

    // Seul le payeur peut payer ; la carte magique 0002 est refusée.
    let response = pay(&app, &alice, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "pas_le_payeur");
    let response = pay(&app, &bob, &trade, "4970 0000 0000 0002").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "paiement_refuse"
    );
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["payment"]["status"], "echoue");
    assert_eq!(vue["payment"]["failure_reason"], "carte_refusee");

    // Retentable : la bonne carte séquestre, le troc s'active, les codes
    // apparaissent, la concurrente devient caduque.
    let response = pay(&app, &bob, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["status"], "accepte");
    assert_eq!(vue["payment"]["status"], "sequestre");
    let code_bob = vue["my_code"].as_str().expect("code bob").to_string();
    {
        let emails = emails.lock().expect("verrou");
        let subjects: Vec<&str> = emails.iter().map(|e| e.subject.as_str()).collect();
        assert!(
            subjects.iter().any(|s| s.contains("sécurisée")),
            "séquestre"
        );
        assert!(
            subjects.iter().any(|s| s.contains("vient d'être réservé")),
            "éviction de Carole : {subjects:?}"
        );
    }
    let vue_carole = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/proposals/{p_carole}"),
                None,
                Some(&carole),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(vue_carole["status"], "caduque");

    // Rejouer le paiement ne change rien (idempotence).
    let response = pay(&app, &bob, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["payment"]["status"], "sequestre");

    // Gherkin : à la double confirmation, la soulte est capturée.
    let code_alice = trade_view(&app, &alice, &trade).await["my_code"]
        .as_str()
        .expect("code alice")
        .to_string();
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": code_bob})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": code_alice})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["status"], "finalise");
    // F5.2 : la capture main propre attend la fenêtre de contestation de
    // 48 h — la maintenance capture ensuite.
    assert_eq!(vue["payment"]["status"], "sequestre");
    api::trade::handlers::maintain_trades(&state).await;
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["payment"]["status"], "sequestre", "48 h non écoulées");
    sqlx::query("UPDATE trades SET finalized_at = now() - interval '49 hours'")
        .execute(&pool)
        .await
        .expect("antidatage");
    api::trade::handlers::maintain_trades(&state).await;
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["payment"]["status"], "capture");
    {
        let emails = emails.lock().expect("verrou");
        let subjects: Vec<&str> = emails.iter().map(|e| e.subject.as_str()).collect();
        assert!(subjects.iter().any(|s| s.contains("transférée")), "bénéf");
        assert!(subjects.iter().any(|s| s.contains("débitée")), "payeur");
    }
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "troque"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn soulte_deux_paiements_simultanes(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "p4@exemple.fr", "palice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo course", 15_000).await;
    let bob = verified_user(&app, &emails, "p5@exemple.fr", "pbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu course", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{proposal}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    let trade = body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string();

    // Double clic simultané : les deux réussissent, une seule transition.
    let (r1, r2) = tokio::join!(
        pay(&app, &bob, &trade, "4970 0000 0000 0000"),
        pay(&app, &bob, &trade, "4970 0000 0000 0000")
    );
    assert_eq!(r1.status(), StatusCode::OK);
    assert_eq!(r2.status(), StatusCode::OK);
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["status"], "accepte");
    assert_eq!(vue["payment"]["status"], "sequestre");
    {
        let emails = emails.lock().expect("verrou");
        let escrow_mails = emails
            .iter()
            .filter(|e| e.subject.contains("sécurisée"))
            .count();
        assert_eq!(escrow_mails, 1, "un seul e-mail de séquestre");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn soulte_expiration_paresseuse_a_la_consultation(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "p6@exemple.fr", "palice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo expiré", 15_000).await;
    let bob = verified_user(&app, &emails, "p7@exemple.fr", "pbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu expiré", 4_000).await;
    // Soulte à la charge d'Alice (destinataire) : elle accepte elle-même,
    // la date limite est courte (30 min).
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_destinataire").await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{proposal}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    let trade = body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string();
    let (minutes,): (f64,) = sqlx::query_as(
        "SELECT (EXTRACT(EPOCH FROM deadline - now()) / 60)::float8 FROM payments \
         WHERE trade_id = $1::uuid",
    )
    .bind(&trade)
    .fetch_one(&pool)
    .await
    .expect("deadline");
    assert!((0.0..=30.0).contains(&minutes), "{minutes}");

    // Date limite dépassée : la simple consultation annule le troc.
    sqlx::query("UPDATE payments SET deadline = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["status"], "annule");
    assert_eq!(vue["payment"]["status"], "expire");
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "disponible"));
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails
            .last()
            .expect("e-mail")
            .subject
            .contains("pas été réglée"));
    }
    // Payer trop tard échoue proprement.
    let response = pay(&app, &alice, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "troc_clos");
}

#[sqlx::test(migrations = "../../migrations")]
async fn soulte_expiration_par_maintenance_et_seconde_chance(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "p8@exemple.fr", "palice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo maintenance", 15_000).await;
    let bob = verified_user(&app, &emails, "p9@exemple.fr", "pbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu maintenance", 4_000).await;
    let carole = verified_user(&app, &emails, "p10@exemple.fr", "pcarole4").await;
    let puzzle = publish_valued(&app, &carole, "Puzzle patient", 4_000).await;

    let p_bob = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let p_carole = simple_proposal(&app, &carole, &puzzle, &velo).await;
    call(
        &app,
        request(
            "POST",
            &format!("/proposals/{p_bob}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;

    // Bob ne paie jamais : la maintenance annule le troc.
    sqlx::query("UPDATE payments SET deadline = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(
        api::trade::handlers::maintain_payments(&state).await,
        (1, 0)
    );
    assert_eq!(
        api::trade::handlers::maintain_payments(&state).await,
        (0, 0)
    );

    // La caducité avait été différée : la proposition de Carole est toujours
    // vivante, et le vélo libéré — Alice peut conclure avec elle.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{p_carole}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["trade"]["status"], "accepte");
}

#[sqlx::test(migrations = "../../migrations")]
async fn soulte_liberee_a_l_annulation_mutuelle(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "p11@exemple.fr", "palice5", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo libéré", 15_000).await;
    let bob = verified_user(&app, &emails, "p12@exemple.fr", "pbob5").await;
    let jeu = publish_valued(&app, &bob, "Jeu libéré", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{proposal}/accept"),
            Some(serde_json::json!({"delivery_mode": "main_propre"})),
            Some(&alice),
        ),
    )
    .await;
    let trade = body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string();
    let response = pay(&app, &bob, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Annulation d'un commun accord : la préautorisation est libérée.
    call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/cancel"),
            None,
            Some(&alice),
        ),
    )
    .await;
    let response = call(
        &app,
        request("POST", &format!("/trades/{trade}/cancel"), None, Some(&bob)),
    )
    .await;
    let vue = body_json(response).await;
    assert_eq!(vue["status"], "annule");
    assert_eq!(vue["payment"]["status"], "annule");
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.iter().any(|e| e.subject.contains("pas débité")));
    }
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "disponible"));
}

// ————— Envoi croisé (F4.3) —————

/// Accepte une proposition en mode envoi et retourne l'id du troc.
async fn accepted_shipping_trade(app: &Router, recipient: &str, proposal_id: &str) -> String {
    let response = call(
        app,
        request(
            "POST",
            &format!("/proposals/{proposal_id}/accept"),
            Some(serde_json::json!({"delivery_mode": "envoi"})),
            Some(recipient),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await["trade"]["id"]
        .as_str()
        .expect("trade id")
        .to_string()
}

/// Configure « mon envoi » : format donné + premier relais proposé.
async fn configure_shipping(app: &Router, who: &str, trade: &str, format: &str) {
    let relays = body_json(
        call(
            app,
            request("GET", &format!("/trades/{trade}/relays"), None, Some(who)),
        )
        .await,
    )
    .await;
    let relay_code = relays[0]["code"].as_str().expect("relais").to_string();
    let response = call(
        app,
        request(
            "POST",
            &format!("/trades/{trade}/shipping"),
            Some(serde_json::json!({"format": format, "relay_code": relay_code})),
            Some(who),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

/// L'id de mon colis à envoyer (`mine`) ou de celui que je reçois.
async fn shipment_id(app: &Router, who: &str, trade: &str, mine: bool) -> String {
    let vue = trade_view(app, who, trade).await;
    vue["shipments"]
        .as_array()
        .expect("shipments")
        .iter()
        .find(|s| s["i_am_sender"] == mine)
        .expect("colis")["id"]
        .as_str()
        .expect("id")
        .to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_parcours_complet(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "s1@exemple.fr", "salice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo expédié", 15_000).await;
    let bob = verified_user(&app, &emails, "s2@exemple.fr", "sbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu expédié", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let trade = accepted_shipping_trade(&app, &alice, &proposal).await;

    // Deux colis, deux paiements : chacun service 2 € + sa soulte éventuelle.
    let vue_alice = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue_alice["status"], "attente_paiement");
    assert!(vue_alice["my_code"].is_null());
    assert_eq!(vue_alice["shipments"].as_array().expect("colis").len(), 2);
    assert_eq!(vue_alice["payment"]["i_am_payer"], true);
    assert_eq!(vue_alice["payment"]["amount_cents"], 200);
    let vue_bob = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue_bob["payment"]["amount_cents"], 2_200);
    // L'autre partie (Bob) a été prévenue de préparer son envoi.
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.last().expect("e-mail").subject.contains("envoi"));
    }

    // Payer avant d'avoir choisi le format : refusé.
    let response = pay(&app, &alice, &trade, "4970 0000 0000 0000").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "envoi_non_configure"
    );

    // Alice : format M (6,90 €) → 8,90 € au total. Bob : S → 24,50 €.
    configure_shipping(&app, &alice, &trade, "m").await;
    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["payment"]["amount_cents"], 890);
    assert_eq!(vue["payment"]["shipping_cents"], 690);
    assert_eq!(vue["payment"]["service_cents"], 200);
    configure_shipping(&app, &bob, &trade, "s").await;
    assert_eq!(
        trade_view(&app, &bob, &trade).await["payment"]["amount_cents"],
        2_650
    );

    // Alice paie ; le troc attend encore Bob.
    assert_eq!(
        pay(&app, &alice, &trade, "4970 0000 0000 0000")
            .await
            .status(),
        StatusCode::OK
    );
    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "attente_paiement");
    assert_eq!(vue["payment"]["status"], "sequestre");
    assert_eq!(vue["other_payment_status"], "en_attente");
    // Une fois payé, l'envoi n'est plus reconfigurable.
    let relays = body_json(
        call(
            &app,
            request(
                "GET",
                &format!("/trades/{trade}/relays"),
                None,
                Some(&alice),
            ),
        )
        .await,
    )
    .await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/shipping"),
            Some(serde_json::json!({"format": "l",
                "relay_code": relays[0]["code"].as_str().expect("code")})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Les codes croisés n'existent pas en mode envoi.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/confirm"),
            Some(serde_json::json!({"code": "123456"})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "mode_envoi");

    // Bob paie : troc actif, étiquettes générées, code de dépôt visible
    // par l'expéditeur seulement.
    assert_eq!(
        pay(&app, &bob, &trade, "4970 0000 0000 0000")
            .await
            .status(),
        StatusCode::OK
    );
    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "accepte");
    let colis = vue["shipments"].as_array().expect("colis");
    for c in colis {
        assert_eq!(c["status"], "etiquette");
        if c["i_am_sender"] == true {
            assert!(c["drop_code"].as_str().expect("code").starts_with("LBT"));
        } else {
            assert!(c["drop_code"].is_null());
        }
    }

    // Bob dépose : le simulateur fait arriver le colis chez Alice.
    let colis_bob = shipment_id(&app, &bob, &trade, true).await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/shipments/{colis_bob}/drop"),
            None,
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = trade_view(&app, &alice, &trade).await;
    let entrant = vue["shipments"]
        .as_array()
        .expect("colis")
        .iter()
        .find(|s| s["i_am_sender"] == false)
        .expect("entrant");
    assert_eq!(entrant["status"], "arrive");
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails
            .last()
            .expect("e-mail")
            .subject
            .contains("point relais"));
    }

    // Un colis a voyagé : plus d'annulation amiable.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/cancel"),
            None,
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "colis_en_route");

    // Alice retire puis confirme ; le troc attend l'autre branche.
    let entrant_alice = shipment_id(&app, &alice, &trade, false).await;
    for action in ["pickup", "confirm"] {
        let response = call(
            &app,
            request(
                "POST",
                &format!("/shipments/{entrant_alice}/{action}"),
                None,
                Some(&alice),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert_eq!(trade_view(&app, &alice, &trade).await["status"], "accepte");

    // Alice expédie à son tour ; Bob réceptionne et confirme → finalisé,
    // objets troqués, les deux règlements capturés.
    let colis_alice = shipment_id(&app, &alice, &trade, true).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{colis_alice}/drop"),
            None,
            Some(&alice),
        ),
    )
    .await;
    let entrant_bob = shipment_id(&app, &bob, &trade, false).await;
    for action in ["pickup", "confirm"] {
        call(
            &app,
            request(
                "POST",
                &format!("/shipments/{entrant_bob}/{action}"),
                None,
                Some(&bob),
            ),
        )
        .await;
    }
    let vue = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue["status"], "finalise");
    assert_eq!(vue["payment"]["status"], "capture");
    let captured: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("paiements");
    assert_eq!(captured.len(), 2);
    assert!(captured.iter().all(|(s,)| s == "capture"));
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM items WHERE id IN ($1::uuid, $2::uuid)")
            .bind(&velo)
            .bind(&jeu)
            .fetch_all(&pool)
            .await
            .expect("statuts");
    assert!(statuses.iter().all(|(s,)| s == "troque"));
    {
        let emails = emails.lock().expect("verrou");
        let subjects: Vec<&str> = emails.iter().map(|e| e.subject.as_str()).collect();
        assert!(
            subjects.iter().any(|s| s.contains("ont voyagé")),
            "finalisation"
        );
        assert!(
            subjects.iter().any(|s| s.contains("transférée")),
            "soulte à Alice"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_paiement_partiel_expire_et_libere_l_autre(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "s3@exemple.fr", "salice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo abandonné", 15_000).await;
    let bob = verified_user(&app, &emails, "s4@exemple.fr", "sbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu abandonné", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = accepted_shipping_trade(&app, &alice, &proposal).await;

    // Alice configure et paie ; Bob ne fait rien.
    configure_shipping(&app, &alice, &trade, "s").await;
    assert_eq!(
        pay(&app, &alice, &trade, "4970 0000 0000 0000")
            .await
            .status(),
        StatusCode::OK
    );

    sqlx::query("UPDATE payments SET deadline = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(api::trade::handlers::maintain_payments(&state).await.0, 1);

    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "annule");
    // Le paiement d'Alice (séquestré) est libéré, celui de Bob expiré.
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid ORDER BY status")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("paiements");
    let statuses: Vec<&str> = statuses.iter().map(|(s,)| s.as_str()).collect();
    assert_eq!(statuses, ["annule", "expire"]);
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM shipments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("colis");
    assert!(statuses.iter().all(|(s,)| s == "annule"));
    let (velo_status,): (String,) = sqlx::query_as("SELECT status FROM items WHERE id = $1::uuid")
        .bind(&velo)
        .fetch_one(&pool)
        .await
        .expect("statut");
    assert_eq!(velo_status, "disponible");
    {
        let emails = emails.lock().expect("verrou");
        let subjects: Vec<&str> = emails.iter().map(|e| e.subject.as_str()).collect();
        assert!(
            subjects.iter().any(|s| s.contains("pas débité")),
            "libération Alice"
        );
    }
}

/// Amène un troc envoi jusqu'à l'état actif (configuré et payé des deux
/// côtés, étiquettes générées).
async fn active_shipping_trade(app: &Router, alice: &str, bob: &str, proposal: &str) -> String {
    let trade = accepted_shipping_trade(app, alice, proposal).await;
    configure_shipping(app, alice, &trade, "s").await;
    configure_shipping(app, bob, &trade, "s").await;
    assert_eq!(
        pay(app, alice, &trade, "4970 0000 0000 0000")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        pay(app, bob, &trade, "4970 0000 0000 0000").await.status(),
        StatusCode::OK
    );
    trade
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_aucun_depot_j5_annule(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "s5@exemple.fr", "salice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo oublié", 15_000).await;
    let bob = verified_user(&app, &emails, "s6@exemple.fr", "sbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu oublié", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = active_shipping_trade(&app, &alice, &bob, &proposal).await;

    sqlx::query("UPDATE trades SET created_at = now() - interval '6 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let report = api::trade::handlers::maintain_trades(&state).await;
    assert_eq!(report.shipping_cancelled, 1);
    assert_eq!(report.cancelled, 0, "le J+14 main propre ne s'applique pas");

    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "annule");
    let (available,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM items WHERE id IN ($1::uuid, $2::uuid) AND status = 'disponible'",
    )
    .bind(&velo)
    .bind(&jeu)
    .fetch_one(&pool)
    .await
    .expect("objets");
    assert_eq!(available, 2);
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.iter().any(|e| e.subject.contains("pas été déposés")));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_depot_partiel_j5_gele_le_troc(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "s7@exemple.fr", "salice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo esseulé", 15_000).await;
    let bob = verified_user(&app, &emails, "s8@exemple.fr", "sbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu esseulé", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = active_shipping_trade(&app, &alice, &bob, &proposal).await;

    // Seul Bob dépose son colis.
    let colis_bob = shipment_id(&app, &bob, &trade, true).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{colis_bob}/drop"),
            None,
            Some(&bob),
        ),
    )
    .await;

    sqlx::query("UPDATE trades SET created_at = now() - interval '6 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let report = api::trade::handlers::maintain_trades(&state).await;
    assert_eq!(report.shipping_frozen, 1);

    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "litige_gele");
    // Les préautorisations sont libérées, la défaillance journalisée pour F5.2.
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("paiements");
    assert!(statuses.iter().all(|(s,)| s == "annule"));
    let (event_type, culprit): (String, Option<uuid::Uuid>) = sqlx::query_as(
        "SELECT event_type, culprit_id FROM dispute_events WHERE trade_id = $1::uuid",
    )
    .bind(&trade)
    .fetch_one(&pool)
    .await
    .expect("journal");
    assert_eq!(event_type, "non_depot");
    assert!(culprit.is_some(), "l'expéditeur défaillant est identifié");
    {
        let emails = emails.lock().expect("verrou");
        let subjects: Vec<&str> = emails.iter().map(|e| e.subject.as_str()).collect();
        assert!(
            subjects.iter().any(|s| s.contains("examen manuel")),
            "admin"
        );
        assert!(subjects.iter().any(|s| s.contains("gelé")), "parties");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_auto_confirmation_72h_et_rappels(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "s9@exemple.fr", "salice5", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo silencieux", 15_000).await;
    let bob = verified_user(&app, &emails, "s10@exemple.fr", "sbob5").await;
    let jeu = publish_valued(&app, &bob, "Jeu silencieux", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = active_shipping_trade(&app, &alice, &bob, &proposal).await;

    // Rappel de dépôt J+2 : les deux expéditeurs, une seule fois.
    sqlx::query("UPDATE shipments SET created_at = now() - interval '3 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    sqlx::query("UPDATE trades SET created_at = now() - interval '3 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let report = api::trade::handlers::maintain_trades(&state).await;
    assert_eq!(report.drop_reminders, 2);
    assert_eq!(
        api::trade::handlers::maintain_trades(&state)
            .await
            .drop_reminders,
        0
    );

    // Les deux déposent, retirent — personne ne confirme.
    for (who, mine) in [(&alice, true), (&bob, true)] {
        let colis = shipment_id(&app, who, &trade, mine).await;
        call(
            &app,
            request("POST", &format!("/shipments/{colis}/drop"), None, Some(who)),
        )
        .await;
    }
    for who in [&alice, &bob] {
        let entrant = shipment_id(&app, who, &trade, false).await;
        call(
            &app,
            request(
                "POST",
                &format!("/shipments/{entrant}/pickup"),
                None,
                Some(who),
            ),
        )
        .await;
    }

    // Un troc envoi vieux de 15 jours n'est PAS annulé par le J+14.
    sqlx::query("UPDATE trades SET created_at = now() - interval '15 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let report = api::trade::handlers::maintain_trades(&state).await;
    assert_eq!(report.cancelled, 0);
    assert_eq!(trade_view(&app, &alice, &trade).await["status"], "accepte");

    // 72 h après les retraits : confirmation automatique → finalisé, capturé.
    sqlx::query("UPDATE shipments SET picked_up_at = now() - interval '73 hours'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(
        api::trade::handlers::auto_confirm_shipments(&state).await,
        2
    );
    let vue = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue["status"], "finalise");
    assert_eq!(vue["payment"]["status"], "capture");
}

#[sqlx::test(migrations = "../../migrations")]
async fn envoi_signalement_gele_et_garde_les_fonds(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "s11@exemple.fr", "salice6", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo cabossé", 15_000).await;
    let bob = verified_user(&app, &emails, "s12@exemple.fr", "sbob6").await;
    let jeu = publish_valued(&app, &bob, "Jeu cabossé", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let trade = active_shipping_trade(&app, &alice, &bob, &proposal).await;

    // Bob expédie ; Alice retire un colis abîmé et signale.
    let colis_bob = shipment_id(&app, &bob, &trade, true).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{colis_bob}/drop"),
            None,
            Some(&bob),
        ),
    )
    .await;
    let entrant = shipment_id(&app, &alice, &trade, false).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{entrant}/pickup"),
            None,
            Some(&alice),
        ),
    )
    .await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({
                "reason": "abime",
                "description": "Le jeu est arrivé cassé en deux."
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["status"], "litige_gele");
    assert_eq!(vue["dispute"]["status"], "ouvert");
    assert_eq!(vue["dispute"]["opened_by_me"], true);
    // Un seul dossier par troc.
    let doublon = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({"reason": "abime", "description": "Encore."})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(doublon.status(), StatusCode::CONFLICT);

    // Les règlements restent séquestrés (ni capturés ni libérés) : la
    // résolution est manuelle en attendant F5.2.
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("paiements");
    assert!(statuses.iter().all(|(s,)| s == "sequestre"));
    let (shipment_status,): (String,) =
        sqlx::query_as("SELECT status FROM shipments WHERE id = $1::uuid")
            .bind(&entrant)
            .fetch_one(&pool)
            .await
            .expect("colis");
    assert_eq!(shipment_status, "incident");
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.iter().any(|e| e.subject.contains("examen manuel")));
    }
}

// ————— Évaluations (F5.1) —————

/// Finalise un troc main propre par échange de codes croisés.
async fn finalized_trade(app: &Router, alice: &str, bob: &str, proposal: &str) -> String {
    let trade = accepted_trade(app, alice, proposal).await;
    let code_alice = trade_view(app, alice, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    let code_bob = trade_view(app, bob, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    for (who, code) in [(alice, &code_bob), (bob, &code_alice)] {
        let response = call(
            app,
            request(
                "POST",
                &format!("/trades/{trade}/confirm"),
                Some(serde_json::json!({"code": code})),
                Some(who),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    trade
}

/// Note un troc, en tant que `who`.
async fn review(
    app: &Router,
    who: &str,
    trade: &str,
    rating: i16,
    comment: Option<&str>,
) -> axum::response::Response {
    call(
        app,
        request(
            "POST",
            &format!("/trades/{trade}/review"),
            Some(serde_json::json!({"rating": rating, "comment": comment})),
            Some(who),
        ),
    )
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn evaluation_publication_simultanee_anti_represailles(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "r1@exemple.fr", "ralice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo noté", 15_000).await;
    let bob = verified_user(&app, &emails, "r2@exemple.fr", "rbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu noté", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;

    // Avant la finalisation : pas de note possible.
    let trade = accepted_trade(&app, &alice, &proposal).await;
    let response = review(&app, &alice, &trade, 5, None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "troc_non_finalise"
    );
    // (On repart du même troc, finalisé par codes croisés.)
    let code_alice = trade_view(&app, &alice, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    let code_bob = trade_view(&app, &bob, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    for (who, code) in [(&alice, &code_bob), (&bob, &code_alice)] {
        call(
            &app,
            request(
                "POST",
                &format!("/trades/{trade}/confirm"),
                Some(serde_json::json!({"code": code})),
                Some(who),
            ),
        )
        .await;
    }

    // Notes hors bornes refusées ; un tiers n'a pas accès.
    let response = review(&app, &alice, &trade, 6, None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let carole = verified_user(&app, &emails, "r3@exemple.fr", "rcarole1").await;
    let response = review(&app, &carole, &trade, 3, None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Gherkin : Alice note — sa note reste invisible pour Bob.
    let response = review(&app, &alice, &trade, 5, Some("Impeccable, merci !")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["reviews"]["mine"]["rating"], 5);
    assert_eq!(vue["reviews"]["mine"]["published"], false);
    assert!(vue["reviews"]["received"].is_null());
    let vue_bob = trade_view(&app, &bob, &trade).await;
    assert!(vue_bob["reviews"]["mine"].is_null());
    assert!(
        vue_bob["reviews"]["received"].is_null(),
        "embargo : Bob ne voit rien avant d'avoir noté"
    );
    // Une seule note par troc.
    let response = review(&app, &alice, &trade, 4, None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    // Bob note à son tour : publication simultanée.
    let response = review(&app, &bob, &trade, 4, Some("Bon échange.")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["reviews"]["mine"]["published"], true);
    assert_eq!(vue["reviews"]["received"]["rating"], 5);
    let vue_alice = trade_view(&app, &alice, &trade).await;
    assert_eq!(vue_alice["reviews"]["received"]["rating"], 4);

    // Le profil public de Bob porte la note reçue et les agrégats.
    let profil = body_json(call(&app, request("GET", "/troqueurs/rbob1", None, None)).await).await;
    assert_eq!(profil["rating_avg"], 5.0);
    assert_eq!(profil["reviews_count"], 1);
    assert_eq!(profil["trades_finalized"], 1);
    assert_eq!(profil["reviews"][0]["comment"], "Impeccable, merci !");
    assert_eq!(profil["reviews"][0]["reviewer_pseudo"], "ralice1");

    // Réponse publique unique du noté.
    let review_id = trade_view(&app, &bob, &trade).await["reviews"]["received"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    let response = call(
        &app,
        request(
            "POST",
            &format!("/reviews/{review_id}/reply"),
            Some(serde_json::json!({"reply": "Merci, à refaire !"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = call(
        &app,
        request(
            "POST",
            &format!("/reviews/{review_id}/reply"),
            Some(serde_json::json!({"reply": "Encore moi"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let profil = body_json(call(&app, request("GET", "/troqueurs/rbob1", None, None)).await).await;
    assert_eq!(profil["reviews"][0]["reply"], "Merci, à refaire !");
}

#[sqlx::test(migrations = "../../migrations")]
async fn evaluation_orpheline_publiee_a_j14(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "r4@exemple.fr", "ralice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo patient", 15_000).await;
    let bob = verified_user(&app, &emails, "r5@exemple.fr", "rbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu patient", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = finalized_trade(&app, &alice, &bob, &proposal).await;

    // Gherkin : seule Alice note — invisible tant que J+14 n'est pas passé.
    assert_eq!(
        review(&app, &alice, &trade, 5, None).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        api::trade::handlers::maintain_trades(&state)
            .await
            .reviews_published,
        0
    );
    sqlx::query("UPDATE trades SET finalized_at = now() - interval '15 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    assert_eq!(
        api::trade::handlers::maintain_trades(&state)
            .await
            .reviews_published,
        1
    );
    let vue_bob = trade_view(&app, &bob, &trade).await;
    assert_eq!(vue_bob["reviews"]["received"]["rating"], 5);
    let profil = body_json(call(&app, request("GET", "/troqueurs/rbob2", None, None)).await).await;
    assert_eq!(profil["reviews_count"], 1);
}

// ————— Signalements, blocages, litiges (F5.2) —————

/// Requête d'administration : token d'env de test.
fn admin_request(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-admin-token", "token-admin-de-test");
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let body = match body {
        Some(json) => Body::from(json.to_string()),
        None => Body::empty(),
    };
    builder.body(body).expect("requête admin")
}

#[sqlx::test(migrations = "../../migrations")]
async fn litige_envoi_contradictoire_et_resolution_liberation(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "d1@exemple.fr", "dalice1", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo litigieux", 15_000).await;
    let bob = verified_user(&app, &emails, "d2@exemple.fr", "dbob1").await;
    let jeu = publish_valued(&app, &bob, "Jeu litigieux", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let trade = active_shipping_trade(&app, &alice, &bob, &proposal).await;

    // Bob expédie, Alice retire un colis cassé et ouvre un dossier.
    let colis_bob = shipment_id(&app, &bob, &trade, true).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{colis_bob}/drop"),
            None,
            Some(&bob),
        ),
    )
    .await;
    let entrant = shipment_id(&app, &alice, &trade, false).await;
    call(
        &app,
        request(
            "POST",
            &format!("/shipments/{entrant}/pickup"),
            None,
            Some(&alice),
        ),
    )
    .await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({"reason": "abime", "description": "Cadre fendu."})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["status"], "litige_gele");
    let dispute_id = vue["dispute"]["id"].as_str().expect("dossier").to_string();
    assert_eq!(vue["dispute"]["can_respond"], false);

    // L'auto-confirmation 72 h est suspendue par le dossier.
    sqlx::query(
        "UPDATE shipments SET picked_up_at = now() - interval '80 hours' WHERE id = $1::uuid",
    )
    .bind(&entrant)
    .execute(&pool)
    .await
    .expect("antidatage");
    assert_eq!(
        api::trade::handlers::auto_confirm_shipments(&state).await,
        0
    );

    // Contradictoire : Alice ne peut pas répondre à son propre dossier,
    // Bob si — une seule fois.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/disputes/{dispute_id}/respond"),
            Some(serde_json::json!({"response": "Je conteste."})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = call(
        &app,
        request(
            "POST",
            &format!("/disputes/{dispute_id}/respond"),
            Some(serde_json::json!({"response": "Le colis était nickel au dépôt."})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let vue = body_json(response).await;
    assert_eq!(vue["dispute"]["status"], "en_examen");
    let response = call(
        &app,
        request(
            "POST",
            &format!("/disputes/{dispute_id}/respond"),
            Some(serde_json::json!({"response": "Encore."})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Admin : sans token → 401 ; le dossier apparaît dans la file.
    let response = call(&app, request("GET", "/admin/disputes", None, None)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = call(
        &app,
        admin_request("GET", "/admin/disputes?status=en_examen", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let file = body_json(response).await;
    assert_eq!(file.as_array().expect("liste").len(), 1);
    let response = call(
        &app,
        admin_request("GET", &format!("/admin/disputes/{dispute_id}"), None),
    )
    .await;
    let dossier = body_json(response).await;
    assert_eq!(dossier["reason"], "abime");
    assert_eq!(dossier["response"], "Le colis était nickel au dépôt.");

    // Résolution : libération, Bob en tort → troc annulé, zéro débit,
    // événement au score de Bob.
    let response = call(
        &app,
        admin_request(
            "POST",
            &format!("/admin/disputes/{dispute_id}/resolve"),
            Some(serde_json::json!({
                "outcome": "liberation",
                "penalized_pseudo": "dbob1",
                "note": "photos probantes"
            })),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let verdict = body_json(response).await;
    assert_eq!(verdict["penalized_score"], 6);
    assert_eq!(verdict["sanction"], "avertissement");
    let (trade_status,): (String,) =
        sqlx::query_as("SELECT status FROM trades WHERE id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("troc");
    assert_eq!(trade_status, "annule");
    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_all(&pool)
            .await
            .expect("paiements");
    assert!(statuses.iter().all(|(s,)| s == "annule"));
    // Déjà tranché → 409.
    let response = call(
        &app,
        admin_request(
            "POST",
            &format!("/admin/disputes/{dispute_id}/resolve"),
            Some(serde_json::json!({"outcome": "rejet"})),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails.iter().any(|e| e.subject.contains("tranché")));
        assert!(emails
            .iter()
            .any(|e| e.subject.contains("au sujet de ton compte")));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn capture_main_propre_differee_48h_et_litige_post_remise(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "d3@exemple.fr", "dalice2", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo différé", 15_000).await;
    let bob = verified_user(&app, &emails, "d4@exemple.fr", "dbob2").await;
    let jeu = publish_valued(&app, &bob, "Jeu différé", 4_000).await;
    let proposal = cash_proposal(&app, &bob, &jeu, &velo, 2_000, "du_proposant").await;
    let trade = accepted_trade(&app, &alice, &proposal).await;
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/pay"),
            Some(serde_json::json!({"card_number": "4970 0000 0000 0000"})),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let code_alice = trade_view(&app, &alice, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    let code_bob = trade_view(&app, &bob, &trade).await["my_code"]
        .as_str()
        .expect("code")
        .to_string();
    for (who, code) in [(&alice, &code_bob), (&bob, &code_alice)] {
        call(
            &app,
            request(
                "POST",
                &format!("/trades/{trade}/confirm"),
                Some(serde_json::json!({"code": code})),
                Some(who),
            ),
        )
        .await;
    }

    // La remise ne capture plus : fenêtre de contestation de 48 h.
    let (payment_status,): (String,) =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("paiement");
    assert_eq!(payment_status, "sequestre");
    api::trade::handlers::maintain_trades(&state).await;
    let (payment_status,): (String,) =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("paiement");
    assert_eq!(
        payment_status, "sequestre",
        "48 h non écoulées : pas de capture"
    );

    // Gherkin : vice découvert sous 48 h → dossier, la capture attend.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({
                "reason": "non_conforme",
                "description": "Le cadre est fissuré sous la peinture."
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    sqlx::query("UPDATE trades SET finalized_at = now() - interval '49 hours'")
        .execute(&pool)
        .await
        .expect("antidatage");
    api::trade::handlers::maintain_trades(&state).await;
    let (payment_status,): (String,) =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("paiement");
    assert_eq!(
        payment_status, "sequestre",
        "dossier ouvert : capture suspendue"
    );

    // Dossier rejeté → la capture différée reprend.
    let dispute_id: (Uuid,) = sqlx::query_as("SELECT id FROM disputes WHERE trade_id = $1::uuid")
        .bind(&trade)
        .fetch_one(&pool)
        .await
        .expect("dossier");
    let response = call(
        &app,
        admin_request(
            "POST",
            &format!("/admin/disputes/{}/resolve", dispute_id.0),
            Some(serde_json::json!({"outcome": "rejet", "penalized_pseudo": "dbob2"})),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let verdict = body_json(response).await;
    assert_eq!(verdict["penalized_score"], 2, "plainte abusive : +2");
    api::trade::handlers::maintain_trades(&state).await;
    let (payment_status,): (String,) =
        sqlx::query_as("SELECT status FROM payments WHERE trade_id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("paiement");
    assert_eq!(payment_status, "capture");
    // Hors fenêtre désormais : plus d'ouverture possible (et un dossier a
    // déjà existé de toute façon).
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({"reason": "abime", "description": "Trop tard."})),
            Some(&alice),
        ),
    )
    .await;
    assert!(matches!(
        response.status(),
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_show_j3_et_rejet_degele(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "d5@exemple.fr", "dalice3", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo fantôme", 15_000).await;
    let bob = verified_user(&app, &emails, "d6@exemple.fr", "dbob3").await;
    let jeu = publish_valued(&app, &bob, "Jeu fantôme", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = accepted_trade(&app, &alice, &proposal).await;

    // Trop tôt pour déclarer un no-show.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({"reason": "jamais_venu", "description": "Personne au rendez-vous."})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(response).await["error"]["code"], "hors_fenetre");

    sqlx::query("UPDATE trades SET created_at = now() - interval '4 days'")
        .execute(&pool)
        .await
        .expect("antidatage");
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({"reason": "jamais_venu", "description": "Personne au rendez-vous."})),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["status"], "litige_gele");

    // Rejet (parole contre parole) → le troc reprend, personne n'est pénalisé.
    let dispute_id: (Uuid,) = sqlx::query_as("SELECT id FROM disputes WHERE trade_id = $1::uuid")
        .bind(&trade)
        .fetch_one(&pool)
        .await
        .expect("dossier");
    let response = call(
        &app,
        admin_request(
            "POST",
            &format!("/admin/disputes/{}/resolve", dispute_id.0),
            Some(serde_json::json!({"outcome": "rejet"})),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let (trade_status,): (String,) =
        sqlx::query_as("SELECT status FROM trades WHERE id = $1::uuid")
            .bind(&trade)
            .fetch_one(&pool)
            .await
            .expect("troc");
    assert_eq!(trade_status, "accepte");
}

#[sqlx::test(migrations = "../../migrations")]
async fn blocage_ferme_propositions_messages_et_feed(pool: PgPool) {
    let (app, emails) = app(pool.clone());
    let alice = verified_user_at(&app, &emails, "d7@exemple.fr", "dalice4", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo bloqué", 15_000).await;
    let bob = verified_user(&app, &emails, "d8@exemple.fr", "dbob4").await;
    let jeu = publish_valued(&app, &bob, "Jeu bloqué", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;

    // Alice bloque Bob : la proposition en attente devient caduque.
    let response = call(
        &app,
        request("POST", "/users/dbob4/block", None, Some(&alice)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let (proposal_status,): (String,) =
        sqlx::query_as("SELECT status FROM proposals WHERE id = $1::uuid")
            .bind(&proposal)
            .fetch_one(&pool)
            .await
            .expect("proposition");
    assert_eq!(proposal_status, "caduque");

    // Plus de nouvelle proposition (message neutre), dans les deux sens.
    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [jeu], "requested_item_ids": [velo]
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "propositions_fermees"
    );

    // Plus de message sur la conversation pré-acceptation.
    let response = call(
        &app,
        request(
            "POST",
            &format!("/proposals/{proposal}/messages"),
            Some(serde_json::json!({"body": "Allo ?"})),
            Some(&bob),
        ),
    )
    .await;
    assert!(matches!(
        response.status(),
        StatusCode::FORBIDDEN | StatusCode::BAD_REQUEST
    ));

    // Masquage bidirectionnel de la recherche.
    let response = call(
        &app,
        request("GET", "/search?q=bloqu%C3%A9", None, Some(&bob)),
    )
    .await;
    let resultats = body_json(response).await;
    let titres: Vec<String> = resultats["items"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|i| i["title"].as_str().map(String::from))
        .collect();
    assert!(
        !titres.iter().any(|t| t.contains("Vélo bloqué")),
        "l'objet d'Alice ne doit plus apparaître pour Bob : {titres:?}"
    );

    // Mes blocages, puis déblocage : tout rouvre.
    let response = call(&app, request("GET", "/me/blocks", None, Some(&alice))).await;
    assert_eq!(body_json(response).await["pseudos"][0], "dbob4");
    let response = call(
        &app,
        request("DELETE", "/users/dbob4/block", None, Some(&alice)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = call(
        &app,
        request(
            "POST",
            "/proposals",
            Some(serde_json::json!({
                "offered_item_ids": [jeu], "requested_item_ids": [velo]
            })),
            Some(&bob),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn score_bannissement_automatique_et_levee(pool: PgPool) {
    let (state, emails) = AppState::for_tests(pool.clone());
    let app = api::router(state.clone());
    let alice = verified_user_at(&app, &emails, "d9@exemple.fr", "dalice5", "44300").await;
    let velo = publish_valued(&app, &alice, "Vélo sanction", 15_000).await;
    let bob = verified_user(&app, &emails, "d10@exemple.fr", "dbob5").await;
    let jeu = publish_valued(&app, &bob, "Jeu sanction", 4_000).await;
    let proposal = simple_proposal(&app, &bob, &jeu, &velo).await;
    let trade = finalized_trade(&app, &alice, &bob, &proposal).await;

    // Passif chargé (10 pts) + contrefaçon avérée (15 pts) = bannissement.
    let bob_id: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE pseudo = 'dbob5'")
        .fetch_one(&pool)
        .await
        .expect("bob");
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO dispute_events (trade_id, event_type, culprit_id, details) \
             VALUES ($1::uuid, 'non_depot', $2, 'passif')",
        )
        .bind(&trade)
        .bind(bob_id.0)
        .execute(&pool)
        .await
        .expect("événement");
    }
    let response = call(
        &app,
        request(
            "POST",
            &format!("/trades/{trade}/dispute"),
            Some(serde_json::json!({
                "reason": "contrefacon",
                "description": "Ce n'est pas un vrai Kapla."
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let dispute_id: (Uuid,) = sqlx::query_as("SELECT id FROM disputes WHERE trade_id = $1::uuid")
        .bind(&trade)
        .fetch_one(&pool)
        .await
        .expect("dossier");
    let response = call(
        &app,
        admin_request(
            "POST",
            &format!("/admin/disputes/{}/resolve", dispute_id.0),
            Some(serde_json::json!({"outcome": "liberation", "penalized_pseudo": "dbob5"})),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let verdict = body_json(response).await;
    assert_eq!(verdict["penalized_score"], 25);
    assert_eq!(verdict["sanction"], "bannissement");

    // Sessions révoquées + connexion refusée.
    let response = call(
        &app,
        request(
            "POST",
            "/auth/login",
            Some(serde_json::json!({"email": "d10@exemple.fr", "password": "un-bon-mot-de-passe"})),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "compte_suspendu"
    );

    // Filet admin : levée des sanctions → connexion possible.
    let response = call(
        &app,
        admin_request("POST", "/admin/users/dbob5/lift-sanctions", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = call(
        &app,
        request(
            "POST",
            "/auth/login",
            Some(serde_json::json!({"email": "d10@exemple.fr", "password": "un-bon-mot-de-passe"})),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../../migrations")]
async fn signalements_types_valides(pool: PgPool) {
    let (app, emails) = app(pool);
    let alice = verified_user_at(&app, &emails, "d11@exemple.fr", "dalice6", "44300").await;
    let cible = Uuid::new_v4();

    let ok = call(
        &app,
        request(
            "POST",
            "/reports",
            Some(serde_json::json!({
                "target_type": "utilisateur", "target_id": cible,
                "reason": "arnaque_suspectee", "comment": "Demande un virement hors app."
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(ok.status(), StatusCode::CREATED);

    // Motif d'une autre cible → refusé ; « autre » sans précision → refusé.
    let ko = call(
        &app,
        request(
            "POST",
            "/reports",
            Some(serde_json::json!({
                "target_type": "message", "target_id": cible, "reason": "spam_doublon"
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(ko.status(), StatusCode::BAD_REQUEST);
    let ko = call(
        &app,
        request(
            "POST",
            "/reports",
            Some(serde_json::json!({
                "target_type": "utilisateur", "target_id": cible, "reason": "autre"
            })),
            Some(&alice),
        ),
    )
    .await;
    assert_eq!(ko.status(), StatusCode::BAD_REQUEST);
    {
        let emails = emails.lock().expect("verrou");
        assert!(emails
            .iter()
            .any(|e| e.text.contains("signalement utilisateur")));
    }
}
