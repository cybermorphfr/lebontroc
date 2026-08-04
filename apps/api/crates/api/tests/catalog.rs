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
                Some(serde_json::json!({"delivery_mode": "envoi"})),
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

    // Bob accepte la contre-proposition : le troc porte bien la soulte.
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
    assert_eq!(json["cash_cents"], 2000);
}
