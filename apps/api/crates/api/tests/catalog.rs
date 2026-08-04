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
