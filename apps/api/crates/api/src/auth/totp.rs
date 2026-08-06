//! Double authentification TOTP des administrateurs (spec admin §4.3) :
//! enrôlement avec QR code, codes de secours à usage unique, élévation de
//! session, récupération réservée au compte maître.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use qrcode::render::svg;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use totp_rs::{Algorithm, Secret, TOTP};
use utoipa::ToSchema;

use crate::auth::tokens::sha256_hex;
use crate::error::ApiError;
use crate::extract::{AdminActor, CurrentUser};
use crate::telemetry;
use crate::AppState;

/// L'instance TOTP standard (SHA-1, 6 chiffres, période 30 s — ce que
/// lisent Google Authenticator, Aegis, 1Password…).
fn totp_instance(secret_base32: &str, compte: &str) -> Result<TOTP, ApiError> {
    let secret = Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|_| ApiError::internal("secret TOTP illisible"))?;
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("Lebontroc".to_string()),
        compte.to_string(),
    )
    .map_err(ApiError::internal)
}

fn code_valide(totp: &TOTP, code: &str) -> bool {
    totp.check_current(code.trim()).unwrap_or(false)
}

/// Un code de secours lisible : XXXX-XXXX (alphabet sans ambiguïté).
fn code_de_secours() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let tirage = |rng: &mut rand::rngs::ThreadRng| {
        (0..4)
            .map(|_| ALPHABET[rand::Rng::gen_range(rng, 0..ALPHABET.len())] as char)
            .collect::<String>()
    };
    format!("{}-{}", tirage(&mut rng), tirage(&mut rng))
}

#[derive(Serialize, ToSchema)]
pub struct TotpStatusResponse {
    /// La 2FA est active sur ce compte.
    pub enabled: bool,
    /// Un secret attend sa confirmation.
    pub pending: bool,
    /// La session courante a vérifié le second facteur.
    pub session_verified: bool,
    /// Codes de secours restants.
    pub recovery_left: i64,
}

/// Où en est ma double authentification.
#[utoipa::path(
    get,
    path = "/me/totp",
    tag = "auth",
    responses((status = 200, description = "État 2FA", body = TotpStatusResponse))
)]
pub async fn totp_status(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<TotpStatusResponse>, ApiError> {
    let etat = infra::admin_repo::totp_state(&state.pool, user.user_id).await?;
    Ok(Json(TotpStatusResponse {
        enabled: etat.totp_enabled_at.is_some(),
        pending: etat.totp_secret.is_some() && etat.totp_enabled_at.is_none(),
        session_verified: infra::admin_repo::session_totp_verified(&state.pool, user.session_id)
            .await?,
        recovery_left: infra::admin_repo::recovery_codes_left(&state.pool, user.user_id).await?,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct TotpStartResponse {
    /// Secret Base32, à saisir manuellement si le QR ne passe pas.
    pub secret: String,
    /// URI otpauth:// (lien direct vers l'application).
    pub otpauth_url: String,
    /// Le QR code, en SVG prêt à afficher.
    pub qr_svg: String,
}

/// Démarrer l'enrôlement : un nouveau secret, à confirmer avec un premier
/// code avant que la 2FA ne soit active.
#[utoipa::path(
    post,
    path = "/me/totp/start",
    tag = "auth",
    responses((status = 200, description = "Secret à scanner", body = TotpStartResponse))
)]
pub async fn totp_start(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<TotpStartResponse>, ApiError> {
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    let secret = Secret::generate_secret().to_encoded().to_string();
    let totp = totp_instance(&secret, &me.pseudo)?;
    let otpauth_url = totp.get_url();
    let qr_svg = QrCode::new(otpauth_url.as_bytes())
        .map_err(ApiError::internal)?
        .render::<svg::Color>()
        .min_dimensions(192, 192)
        .build();
    infra::admin_repo::totp_start(&state.pool, user.user_id, &secret).await?;
    Ok(Json(TotpStartResponse {
        secret,
        otpauth_url,
        qr_svg,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct TotpCodeRequest {
    /// Code à 6 chiffres de l'application — ou code de secours XXXX-XXXX.
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct TotpEnableResponse {
    /// Codes de secours à usage unique — affichés UNE SEULE fois.
    pub recovery_codes: Vec<String>,
}

/// Confirmer l'enrôlement avec un premier code : la 2FA devient active,
/// les codes de secours sont générés (montrés une seule fois), la session
/// courante est élevée.
#[utoipa::path(
    post,
    path = "/me/totp/enable",
    tag = "auth",
    request_body = TotpCodeRequest,
    responses(
        (status = 200, description = "2FA active", body = TotpEnableResponse),
        (status = 400, description = "Code incorrect ou enrôlement non démarré", body = crate::error::ErrorResponse)
    )
)]
pub async fn totp_enable(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TotpCodeRequest>,
) -> Result<Json<TotpEnableResponse>, ApiError> {
    let etat = infra::admin_repo::totp_state(&state.pool, user.user_id).await?;
    let Some(secret) = etat.totp_secret else {
        return Err(ApiError::bad_request(
            "totp_non_demarre",
            "Commence par générer ton QR code.",
        ));
    };
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    if !code_valide(&totp_instance(&secret, &me.pseudo)?, &body.code) {
        return Err(ApiError::bad_request(
            "code_incorrect",
            "Ce code ne correspond pas. Vérifie l'heure de ton téléphone et réessaie.",
        ));
    }
    let codes: Vec<String> = (0..10).map(|_| code_de_secours()).collect();
    let hashes: Vec<String> = codes.iter().map(|c| sha256_hex(c)).collect();
    infra::admin_repo::totp_enable(&state.pool, user.user_id, user.session_id, &hashes).await?;
    telemetry::track(&state, "totp_enabled", Some(user.user_id), json!({})).await;
    Ok(Json(TotpEnableResponse {
        recovery_codes: codes,
    }))
}

/// Vérifier le second facteur pour la session courante (après connexion) :
/// code TOTP, ou code de secours (consommé).
#[utoipa::path(
    post,
    path = "/auth/totp/verify",
    tag = "auth",
    request_body = TotpCodeRequest,
    responses(
        (status = 204, description = "Session élevée"),
        (status = 400, description = "Code incorrect", body = crate::error::ErrorResponse)
    )
)]
pub async fn totp_verify(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TotpCodeRequest>,
) -> Result<StatusCode, ApiError> {
    let etat = infra::admin_repo::totp_state(&state.pool, user.user_id).await?;
    let (Some(secret), Some(_)) = (etat.totp_secret, etat.totp_enabled_at) else {
        // Pas de 2FA : rien à vérifier, la session vaut déjà.
        return Ok(StatusCode::NO_CONTENT);
    };
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    let code = body.code.trim();
    let ok = code_valide(&totp_instance(&secret, &me.pseudo)?, code)
        || infra::admin_repo::consume_recovery_code(
            &state.pool,
            user.user_id,
            &sha256_hex(&code.to_uppercase()),
        )
        .await?;
    if !ok {
        telemetry::track(&state, "totp_failed", Some(user.user_id), json!({})).await;
        return Err(ApiError::bad_request(
            "code_incorrect",
            "Ce code ne correspond pas.",
        ));
    }
    infra::admin_repo::mark_session_totp_verified(&state.pool, user.session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Désactiver sa propre 2FA (code exigé — pas de désactivation volée).
#[utoipa::path(
    post,
    path = "/me/totp/disable",
    tag = "auth",
    request_body = TotpCodeRequest,
    responses(
        (status = 204, description = "2FA désactivée"),
        (status = 400, description = "Code incorrect", body = crate::error::ErrorResponse)
    )
)]
pub async fn totp_disable(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TotpCodeRequest>,
) -> Result<StatusCode, ApiError> {
    let etat = infra::admin_repo::totp_state(&state.pool, user.user_id).await?;
    let Some(secret) = etat.totp_secret else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let me = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Compte introuvable."))?;
    if !code_valide(&totp_instance(&secret, &me.pseudo)?, &body.code) {
        return Err(ApiError::bad_request(
            "code_incorrect",
            "Ce code ne correspond pas.",
        ));
    }
    infra::admin_repo::totp_reset(&state.pool, user.user_id).await?;
    telemetry::track(&state, "totp_disabled", Some(user.user_id), json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Récupération : réinitialiser la 2FA d'un compte verrouillé. Réservé au
/// compte maître (ou à la clé de service) — spec admin §4.3.
#[utoipa::path(
    post,
    path = "/admin/users/{pseudo}/reset-2fa",
    tag = "admin",
    params(("pseudo" = String, Path, description = "Pseudo du compte à débloquer")),
    responses(
        (status = 204, description = "2FA réinitialisée"),
        (status = 403, description = "Réservé au compte maître", body = crate::error::ErrorResponse),
        (status = 404, description = "Inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_reset_totp(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(pseudo): Path<String>,
) -> Result<StatusCode, ApiError> {
    // Le maître, ou la clé de service (user_id None) : personne d'autre.
    if admin.user_id.is_some() && !admin.is_master {
        return Err(ApiError::forbidden(
            "compte_maitre_requis",
            "La récupération 2FA est réservée au compte maître.",
        ));
    }
    let cible = infra::admin_repo::find_role_target(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;
    infra::admin_repo::totp_reset(&state.pool, cible.id).await?;
    crate::admin::handlers::record_admin_action(
        &state,
        admin.user_id,
        "totp_reset",
        "utilisateur",
        &cible.id.to_string(),
        Some(&cible.pseudo),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
