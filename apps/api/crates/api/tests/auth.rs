//! Tests d'intégration F0.2 — un scénario par endpoint, base éphémère.

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

fn post_json(uri: &str, body: serde_json::Value, cookies: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookies) = cookies {
        builder = builder.header(header::COOKIE, cookies);
    }
    builder.body(Body::from(body.to_string())).expect("requête")
}

fn get(uri: &str, cookies: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    if let Some(cookies) = cookies {
        builder = builder.header(header::COOKIE, cookies);
    }
    builder.body(Body::empty()).expect("requête")
}

/// Extrait les cookies posés (`name=value`) d'une réponse.
fn set_cookies(response: &axum::response::Response) -> Vec<(String, String)> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|raw| {
            let first = raw.split(';').next()?;
            let (name, value) = first.split_once('=')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn cookie_header(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
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

fn signup_body(email: &str, pseudo: &str) -> serde_json::Value {
    serde_json::json!({
        "email": email,
        "password": "un-bon-mot-de-passe",
        "pseudo": pseudo,
        "postal_code": "44000"
    })
}

/// Inscrit un utilisateur et retourne l'en-tête Cookie prêt à l'emploi.
async fn signup(app: &Router, email: &str, pseudo: &str) -> String {
    let response = call(
        app,
        post_json("/auth/signup", signup_body(email, pseudo), None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    cookie_header(&set_cookies(&response))
}

fn extract_token(emails: &Emails) -> String {
    let emails = emails.lock().expect("verrou");
    let text = &emails.last().expect("au moins un e-mail").text;
    let start = text.find("token=").expect("lien avec token") + "token=".len();
    text[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// ————— Inscription —————

#[sqlx::test(migrations = "../../migrations")]
async fn signup_cree_connecte_et_envoie_l_email(pool: PgPool) {
    let (app, emails) = app(pool);
    let response = call(
        &app,
        post_json(
            "/auth/signup",
            signup_body("camille@exemple.fr", "camille"),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let cookies = set_cookies(&response);
    assert!(cookies.iter().any(|(n, _)| n == "lbt_access"));
    assert!(cookies.iter().any(|(n, _)| n == "lbt_refresh"));

    let json = body_json(response).await;
    assert_eq!(json["pseudo"], "camille");
    assert_eq!(json["email_verified"], false);

    let captured = emails.lock().expect("verrou");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].to, "camille@exemple.fr");
    assert!(captured[0].text.contains("token="));
}

#[sqlx::test(migrations = "../../migrations")]
async fn signup_rejette_email_et_pseudo_pris(pool: PgPool) {
    let (app, _) = app(pool);
    signup(&app, "camille@exemple.fr", "camille").await;

    let response = call(
        &app,
        post_json(
            "/auth/signup",
            signup_body("camille@exemple.fr", "autre"),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(response).await["error"]["code"], "email_pris");

    let response = call(
        &app,
        post_json(
            "/auth/signup",
            signup_body("autre@exemple.fr", "Camille"),
            None,
        ),
    )
    .await;
    // Unicité insensible à la casse (citext).
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(response).await["error"]["code"], "pseudo_pris");
}

#[sqlx::test(migrations = "../../migrations")]
async fn signup_rejette_les_champs_invalides(pool: PgPool) {
    let (app, _) = app(pool);
    for (body, code) in [
        (signup_body("pas-un-email", "camille"), "email_invalide"),
        (
            serde_json::json!({"email":"a@b.fr","password":"court","pseudo":"camille","postal_code":"44000"}),
            "mot_de_passe_trop_court",
        ),
        (signup_body("a@b.fr", "x"), "pseudo_invalide"),
        (
            serde_json::json!({"email":"a@b.fr","password":"un-bon-mot-de-passe","pseudo":"camille","postal_code":"440"}),
            "code_postal_invalide",
        ),
    ] {
        let response = call(&app, post_json("/auth/signup", body, None)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(response).await["error"]["code"], code);
    }
}

// ————— Connexion et verrouillage —————

#[sqlx::test(migrations = "../../migrations")]
async fn login_ok_puis_echec_puis_verrouillage(pool: PgPool) {
    let (app, _) = app(pool);
    signup(&app, "camille@exemple.fr", "camille").await;

    let ok = serde_json::json!({"email": "camille@exemple.fr", "password": "un-bon-mot-de-passe"});
    let mauvais = serde_json::json!({"email": "camille@exemple.fr", "password": "pas-le-bon-mdp"});

    let response = call(&app, post_json("/auth/login", ok.clone(), None)).await;
    assert_eq!(response.status(), StatusCode::OK);

    // E-mail inconnu : même réponse générique.
    let response = call(
        &app,
        post_json(
            "/auth/login",
            serde_json::json!({"email":"inconnu@exemple.fr","password":"nimporte-quoi"}),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    for _ in 0..5 {
        let response = call(&app, post_json("/auth/login", mauvais.clone(), None)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    // 5 échecs consécutifs → verrouillé, même avec le bon mot de passe.
    let response = call(&app, post_json("/auth/login", ok, None)).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "compte_verrouille"
    );
}

// ————— Vérification e-mail —————

#[sqlx::test(migrations = "../../migrations")]
async fn verification_email_de_bout_en_bout(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = signup(&app, "camille@exemple.fr", "camille").await;

    let json = body_json(call(&app, get("/me", Some(&cookies))).await).await;
    assert_eq!(json["email_verified"], false);

    let token = extract_token(&emails);
    let response = call(
        &app,
        get(&format!("/auth/verify-email?token={token}"), None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .expect("location");
    assert!(
        location.ends_with("/verification?statut=ok"),
        "location: {location}"
    );

    let json = body_json(call(&app, get("/me", Some(&cookies))).await).await;
    assert_eq!(json["email_verified"], true);

    // Rejouer le même lien sur un compte vérifié → succès quand même.
    let response = call(
        &app,
        get(&format!("/auth/verify-email?token={token}"), None),
    )
    .await;
    let location = response.headers()[header::LOCATION]
        .to_str()
        .expect("location");
    assert!(location.ends_with("statut=ok"));

    // Token inconnu → invalide.
    let response = call(&app, get("/auth/verify-email?token=nimporte-quoi", None)).await;
    let location = response.headers()[header::LOCATION]
        .to_str()
        .expect("location");
    assert!(location.ends_with("statut=invalide"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn renvoi_de_verification_avec_cooldown(pool: PgPool) {
    let (app, emails) = app(pool);
    let cookies = signup(&app, "camille@exemple.fr", "camille").await;
    assert_eq!(emails.lock().expect("verrou").len(), 1);

    // Renvoi immédiat : bloqué par le cooldown de 60 s.
    let response = call(
        &app,
        post_json(
            "/auth/resend-verification",
            serde_json::json!({}),
            Some(&cookies),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body_json(response).await["error"]["code"],
        "renvoi_trop_rapide"
    );
}

// ————— Refresh : rotation et rejeu (Gherkin « sécurité des sessions ») —————

#[sqlx::test(migrations = "../../migrations")]
async fn refresh_rotation_puis_rejeu_revoque_tout(pool: PgPool) {
    let (app, _) = app(pool);
    let cookies_initiaux = signup(&app, "camille@exemple.fr", "camille").await;

    // Rotation : le refresh fournit de nouveaux cookies.
    let response = call(
        &app,
        post_json(
            "/auth/refresh",
            serde_json::json!({}),
            Some(&cookies_initiaux),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let nouveaux = cookie_header(&set_cookies(&response));
    assert!(nouveaux.contains("lbt_refresh="));

    // Rejeu de l'ancien refresh token → 401 et toutes les sessions révoquées.
    let response = call(
        &app,
        post_json(
            "/auth/refresh",
            serde_json::json!({}),
            Some(&cookies_initiaux),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Le nouveau token, pourtant jamais utilisé, est lui aussi mort.
    let response = call(
        &app,
        post_json("/auth/refresh", serde_json::json!({}), Some(&nouveaux)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ————— Logout et sessions —————

#[sqlx::test(migrations = "../../migrations")]
async fn logout_revoque_la_session(pool: PgPool) {
    let (app, _) = app(pool);
    let cookies = signup(&app, "camille@exemple.fr", "camille").await;

    let response = call(
        &app,
        post_json("/auth/logout", serde_json::json!({}), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Le refresh de cette session ne fonctionne plus.
    let response = call(
        &app,
        post_json("/auth/refresh", serde_json::json!({}), Some(&cookies)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn sessions_liste_et_revocation(pool: PgPool) {
    let (app, _) = app(pool);
    let premiere = signup(&app, "camille@exemple.fr", "camille").await;
    // Deuxième appareil.
    let login =
        serde_json::json!({"email": "camille@exemple.fr", "password": "un-bon-mot-de-passe"});
    let response = call(&app, post_json("/auth/login", login, None)).await;
    let seconde = cookie_header(&set_cookies(&response));

    let sessions = body_json(call(&app, get("/auth/sessions", Some(&seconde))).await).await;
    let sessions = sessions.as_array().expect("tableau").clone();
    assert_eq!(sessions.len(), 2);
    let courante = sessions
        .iter()
        .find(|s| s["current"] == true)
        .expect("session courante");
    let autre = sessions
        .iter()
        .find(|s| s["current"] == false)
        .expect("autre session");

    // Révoquer l'autre appareil individuellement.
    let request = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/auth/sessions/{}",
            autre["id"].as_str().expect("id")
        ))
        .header(header::COOKIE, &seconde)
        .body(Body::empty())
        .expect("requête");
    let response = call(&app, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // L'ancien appareil ne peut plus rafraîchir.
    let response = call(
        &app,
        post_json("/auth/refresh", serde_json::json!({}), Some(&premiere)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // « Déconnecter tous les autres » ne touche pas la session courante.
    let request = Request::builder()
        .method("DELETE")
        .uri("/auth/sessions")
        .header(header::COOKIE, &seconde)
        .body(Body::empty())
        .expect("requête");
    let response = call(&app, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let sessions = body_json(call(&app, get("/auth/sessions", Some(&seconde))).await).await;
    assert_eq!(sessions.as_array().expect("tableau").len(), 1);
    assert_eq!(sessions[0]["id"], courante["id"]);
}

// ————— Télémétrie front —————

#[sqlx::test(migrations = "../../migrations")]
async fn track_event_whitelist(pool: PgPool) {
    let (app, _) = app(pool.clone());

    let response = call(
        &app,
        post_json(
            "/analytics/track",
            serde_json::json!({"name": "signup_started"}),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = call(
        &app,
        post_json(
            "/analytics/track",
            serde_json::json!({"name": "evenement_pirate"}),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM analytics_events WHERE name = 'signup_started'")
            .fetch_one(&pool)
            .await
            .expect("comptage");
    assert_eq!(count.0, 1);
}

// ————— Profil —————

#[sqlx::test(migrations = "../../migrations")]
async fn me_protege_lit_et_met_a_jour(pool: PgPool) {
    let (app, _) = app(pool);

    // Sans cookie → 401.
    let response = call(&app, get("/me", None)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let cookies = signup(&app, "camille@exemple.fr", "camille").await;
    let json = body_json(call(&app, get("/me", Some(&cookies))).await).await;
    assert_eq!(json["email"], "camille@exemple.fr");
    assert_eq!(json["postal_code"], "44000");

    // Mise à jour du profil.
    let request = Request::builder()
        .method("PATCH")
        .uri("/me")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookies)
        .body(Body::from(
            serde_json::json!({"pseudo": "camille_troc", "postal_code": "35000"}).to_string(),
        ))
        .expect("requête");
    let json = body_json(call(&app, request).await).await;
    assert_eq!(json["pseudo"], "camille_troc");
    assert_eq!(json["postal_code"], "35000");
}
