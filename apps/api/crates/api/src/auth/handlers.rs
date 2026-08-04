//! Handlers d'authentification. Minces : validation → domaine/infra → réponse.

use axum::extract::{Path, Query, State};
use axum::http::header::USER_AGENT;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Redirect;
use axum::Json;
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use domain::auth as regles;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::cookies;
use crate::auth::dto::{
    LoginRequest, SessionResponse, SignupRequest, TrackRequest, UpdateProfileRequest, UserResponse,
};
use crate::auth::{jwt, password, tokens};
use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::telemetry;
use crate::AppState;

const VERIFICATION_TTL_HOURS: i64 = 24;
const RESEND_COOLDOWN_SECONDS: i64 = 60;

fn map_validation(error: regles::ValidationError) -> ApiError {
    match error {
        regles::ValidationError::EmailInvalide => ApiError::bad_request(
            "email_invalide",
            "Il nous faut un e-mail valide pour t'envoyer le lien de vérification.",
        ),
        regles::ValidationError::MotDePasseTropCourt => ApiError::bad_request(
            "mot_de_passe_trop_court",
            "Encore un effort : 8 caractères minimum.",
        ),
        regles::ValidationError::PseudoInvalide => ApiError::bad_request(
            "pseudo_invalide",
            "3 à 30 caractères, lettres, chiffres, tirets ou underscore.",
        ),
        regles::ValidationError::CodePostalInvalide => ApiError::bad_request(
            "code_postal_invalide",
            "Un code postal à 5 chiffres, comme 44000.",
        ),
    }
}

fn map_unique(violation: infra::auth_repo::UniqueViolation) -> ApiError {
    match violation {
        infra::auth_repo::UniqueViolation::EmailPris => ApiError::conflict(
            "email_pris",
            "Un compte existe déjà avec cet e-mail. Connecte-toi plutôt ?",
        ),
        infra::auth_repo::UniqueViolation::PseudoPris => ApiError::conflict(
            "pseudo_pris",
            "Ce pseudo est déjà pris. Tente une variante ?",
        ),
        infra::auth_repo::UniqueViolation::Autre(error) => ApiError::internal(error),
    }
}

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(255).collect())
}

/// Ouvre une session et pose les cookies access + refresh.
async fn open_session(
    state: &AppState,
    jar: CookieJar,
    user_id: Uuid,
    agent: Option<String>,
) -> Result<CookieJar, ApiError> {
    let session = infra::auth_repo::create_session(&state.pool, user_id, agent.as_deref()).await?;
    let refresh = tokens::generate();
    infra::auth_repo::insert_refresh_token(
        &state.pool,
        session.id,
        &refresh.hash,
        Utc::now() + Duration::days(cookies::REFRESH_DAYS),
    )
    .await?;
    let access = jwt::encode_access(&state.config, user_id, session.id)?;
    Ok(jar
        .add(cookies::access_cookie(&state.config, access))
        .add(cookies::refresh_cookie(&state.config, refresh.raw)))
}

/// Émet un token de vérification et envoie l'e-mail.
async fn send_verification_email(
    state: &AppState,
    user_id: Uuid,
    email: &str,
    pseudo: &str,
) -> Result<(), ApiError> {
    let token = tokens::generate();
    infra::auth_repo::create_verification_token(
        &state.pool,
        user_id,
        &token.hash,
        Utc::now() + Duration::hours(VERIFICATION_TTL_HOURS),
    )
    .await?;
    let link = format!(
        "{}/api/auth/verify-email?token={}",
        state.config.app_base_url, token.raw
    );
    if let Err(error) = state.mailer.send_verification(email, pseudo, &link).await {
        // L'inscription ne doit pas échouer si le SMTP tousse : le renvoi existe.
        tracing::error!(%error, "envoi de l'e-mail de vérification en échec");
    }
    Ok(())
}

/// Créer un compte. Connecte immédiatement (cookies posés).
#[utoipa::path(
    post,
    path = "/auth/signup",
    tag = "auth",
    request_body = SignupRequest,
    responses(
        (status = 201, description = "Compte créé et connecté", body = UserResponse),
        (status = 400, description = "Champ invalide", body = crate::error::ErrorResponse),
        (status = 409, description = "E-mail ou pseudo déjà pris", body = crate::error::ErrorResponse)
    )
)]
pub async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(body): Json<SignupRequest>,
) -> Result<(StatusCode, CookieJar, Json<UserResponse>), ApiError> {
    regles::valider_email(&body.email).map_err(map_validation)?;
    regles::valider_mot_de_passe(&body.password).map_err(map_validation)?;
    regles::valider_pseudo(&body.pseudo).map_err(map_validation)?;
    regles::valider_code_postal(&body.postal_code).map_err(map_validation)?;

    let hash = password::hash_password(body.password).await?;
    let email = body.email.trim().to_lowercase();
    let user =
        infra::auth_repo::create_user(&state.pool, &email, &hash, &body.pseudo, &body.postal_code)
            .await
            .map_err(map_unique)?;

    send_verification_email(&state, user.id, &user.email, &user.pseudo).await?;
    let jar = open_session(&state, jar, user.id, user_agent(&headers)).await?;
    telemetry::track(&state, "signup_completed", Some(user.id), json!({})).await;

    Ok((StatusCode::CREATED, jar, Json(user.into())))
}

/// Se connecter.
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Connecté", body = UserResponse),
        (status = 401, description = "Identifiants invalides", body = crate::error::ErrorResponse),
        (status = 429, description = "Compte temporairement verrouillé", body = crate::error::ErrorResponse)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<UserResponse>), ApiError> {
    let identifiants_invalides =
        || ApiError::unauthorized("E-mail ou mot de passe incorrect. Vérifie et réessaie.");

    let Some(user) = infra::auth_repo::find_user_by_email(&state.pool, body.email.trim()).await?
    else {
        // Timing constant : on vérifie quand même un hash factice.
        password::verify_dummy(body.password).await?;
        telemetry::track(
            &state,
            "login_failed",
            None,
            json!({"reason": "unknown_email"}),
        )
        .await;
        return Err(identifiants_invalides());
    };

    if let Some(locked_until) = user.locked_until {
        if locked_until > Utc::now() {
            telemetry::track(
                &state,
                "login_failed",
                Some(user.id),
                json!({"reason": "locked"}),
            )
            .await;
            return Err(ApiError::too_many(
                "compte_verrouille",
                "Trop de tentatives. Réessaie dans 15 minutes.",
            ));
        }
    }

    if !password::verify_password(user.password_hash.clone(), body.password).await? {
        let lock = if regles::doit_verrouiller(user.failed_login_count + 1) {
            Some(Utc::now() + Duration::minutes(regles::VERROU_DUREE_MINUTES))
        } else {
            None
        };
        infra::auth_repo::record_login_failure(&state.pool, user.id, lock).await?;
        telemetry::track(
            &state,
            "login_failed",
            Some(user.id),
            json!({"reason": "bad_password"}),
        )
        .await;
        return Err(identifiants_invalides());
    }

    infra::auth_repo::reset_login_failures(&state.pool, user.id).await?;
    let jar = open_session(&state, jar, user.id, user_agent(&headers)).await?;
    telemetry::track(&state, "login_succeeded", Some(user.id), json!({})).await;

    Ok((jar, Json(user.into())))
}

/// Faire tourner le refresh token (rotation + détection de rejeu).
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "auth",
    responses(
        (status = 204, description = "Nouveaux cookies posés"),
        (status = 401, description = "Refresh token absent, expiré ou rejoué", body = crate::error::ErrorResponse)
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(StatusCode, CookieJar), ApiError> {
    let expiree = || ApiError::unauthorized("Ta session a expiré. Reconnecte-toi.");

    let raw = jar
        .get(cookies::REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(expiree)?;
    let lookup = infra::auth_repo::find_refresh_token(&state.pool, &tokens::sha256_hex(&raw))
        .await?
        .ok_or_else(expiree)?;

    if lookup.used_at.is_some() {
        // Rejeu : un token déjà consommé revient — on révoque tout (Gherkin F0.2).
        infra::auth_repo::revoke_all_sessions(&state.pool, lookup.user_id).await?;
        tracing::warn!(user_id = %lookup.user_id, "rejeu de refresh token détecté, sessions révoquées");
        return Err(expiree());
    }
    if lookup.session_revoked_at.is_some() || lookup.expires_at < Utc::now() {
        return Err(expiree());
    }

    infra::auth_repo::mark_refresh_token_used(&state.pool, lookup.id).await?;
    let refresh = tokens::generate();
    infra::auth_repo::insert_refresh_token(
        &state.pool,
        lookup.session_id,
        &refresh.hash,
        Utc::now() + Duration::days(cookies::REFRESH_DAYS),
    )
    .await?;
    infra::auth_repo::touch_session(&state.pool, lookup.session_id).await?;

    let access = jwt::encode_access(&state.config, lookup.user_id, lookup.session_id)?;
    let jar = jar
        .add(cookies::access_cookie(&state.config, access))
        .add(cookies::refresh_cookie(&state.config, refresh.raw));
    Ok((StatusCode::NO_CONTENT, jar))
}

/// Se déconnecter (révoque la session courante, vide les cookies).
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    responses((status = 204, description = "Déconnecté"))
)]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    user: Option<CurrentUser>,
) -> Result<(StatusCode, CookieJar), ApiError> {
    if let Some(user) = user {
        infra::auth_repo::revoke_session(&state.pool, user.session_id, user.user_id).await?;
    } else if let Some(raw) = jar
        .get(cookies::REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
    {
        if let Some(lookup) =
            infra::auth_repo::find_refresh_token(&state.pool, &tokens::sha256_hex(&raw)).await?
        {
            infra::auth_repo::revoke_session(&state.pool, lookup.session_id, lookup.user_id)
                .await?;
        }
    }
    let jar = jar
        .add(cookies::expired_access_cookie(&state.config))
        .add(cookies::expired_refresh_cookie(&state.config));
    Ok((StatusCode::NO_CONTENT, jar))
}

#[derive(Deserialize)]
pub struct VerifyEmailQuery {
    token: Option<String>,
}

/// Lien cliqué depuis l'e-mail. Redirige vers le front avec un statut.
#[utoipa::path(
    get,
    path = "/auth/verify-email",
    tag = "auth",
    params(("token" = Option<String>, Query, description = "Token du lien e-mail")),
    responses((status = 303, description = "Redirection vers /verification?statut=ok|expire|invalide"))
)]
pub async fn verify_email(
    State(state): State<AppState>,
    Query(query): Query<VerifyEmailQuery>,
) -> Result<Redirect, ApiError> {
    let destination = |statut: &str| {
        Redirect::to(&format!(
            "{}/verification?statut={statut}",
            state.config.app_base_url
        ))
    };

    let Some(raw) = query.token else {
        return Ok(destination("invalide"));
    };
    let Some(lookup) =
        infra::auth_repo::find_verification_token(&state.pool, &tokens::sha256_hex(&raw)).await?
    else {
        return Ok(destination("invalide"));
    };

    if lookup.used_at.is_some() {
        // Double clic sur un compte déjà vérifié : on montre un succès.
        let deja_verifie = infra::auth_repo::find_user_by_id(&state.pool, lookup.user_id)
            .await?
            .map(|u| u.email_verified_at.is_some())
            .unwrap_or(false);
        return Ok(destination(if deja_verifie { "ok" } else { "invalide" }));
    }
    if lookup.expires_at < Utc::now() {
        return Ok(destination("expire"));
    }

    infra::auth_repo::mark_verification_token_used(&state.pool, lookup.id).await?;
    infra::auth_repo::set_email_verified(&state.pool, lookup.user_id).await?;
    telemetry::track(&state, "email_verified", Some(lookup.user_id), json!({})).await;
    Ok(destination("ok"))
}

/// Renvoyer l'e-mail de vérification (cooldown 60 s).
#[utoipa::path(
    post,
    path = "/auth/resend-verification",
    tag = "auth",
    responses(
        (status = 204, description = "E-mail renvoyé (ou compte déjà vérifié)"),
        (status = 429, description = "Renvoi trop rapproché", body = crate::error::ErrorResponse)
    )
)]
pub async fn resend_verification(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<StatusCode, ApiError> {
    let Some(compte) = infra::auth_repo::find_user_by_id(&state.pool, user.user_id).await? else {
        return Err(ApiError::unauthorized("Connecte-toi pour continuer."));
    };
    if compte.email_verified_at.is_some() {
        return Ok(StatusCode::NO_CONTENT);
    }
    if let Some(dernier) =
        infra::auth_repo::last_verification_token_at(&state.pool, user.user_id).await?
    {
        if Utc::now() - dernier < Duration::seconds(RESEND_COOLDOWN_SECONDS) {
            return Err(ApiError::too_many(
                "renvoi_trop_rapide",
                "On vient d'en envoyer un ! Regarde ta boîte, puis réessaie dans une minute.",
            ));
        }
    }
    infra::auth_repo::invalidate_verification_tokens(&state.pool, user.user_id).await?;
    send_verification_email(&state, compte.id, &compte.email, &compte.pseudo).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lister ses appareils connectés.
#[utoipa::path(
    get,
    path = "/auth/sessions",
    tag = "auth",
    responses((status = 200, description = "Sessions actives", body = [SessionResponse]))
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<SessionResponse>>, ApiError> {
    let sessions = infra::auth_repo::list_active_sessions(&state.pool, user.user_id).await?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionResponse::from_session(s, user.session_id))
            .collect(),
    ))
}

/// Déconnecter tous les autres appareils.
#[utoipa::path(
    delete,
    path = "/auth/sessions",
    tag = "auth",
    responses((status = 204, description = "Autres sessions révoquées"))
)]
pub async fn revoke_other_sessions(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<StatusCode, ApiError> {
    infra::auth_repo::revoke_other_sessions(&state.pool, user.user_id, user.session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Déconnecter un appareil précis.
#[utoipa::path(
    delete,
    path = "/auth/sessions/{id}",
    tag = "auth",
    params(("id" = Uuid, Path, description = "Identifiant de session")),
    responses(
        (status = 204, description = "Session révoquée"),
        (status = 404, description = "Session inconnue", body = crate::error::ErrorResponse)
    )
)]
pub async fn revoke_session(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if infra::auth_repo::revoke_session(&state.pool, id, user.user_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("Cette session n'existe pas (ou plus)."))
    }
}

/// Événement d'interface émis par le front (§0.4) — whitelist stricte.
#[utoipa::path(
    post,
    path = "/analytics/track",
    tag = "system",
    request_body = TrackRequest,
    responses(
        (status = 204, description = "Événement enregistré"),
        (status = 400, description = "Événement non autorisé", body = crate::error::ErrorResponse)
    )
)]
pub async fn track_event(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Json(body): Json<TrackRequest>,
) -> Result<StatusCode, ApiError> {
    const AUTORISES: [&str; 3] = [
        "signup_started",
        "item_publish_started",
        "item_publish_abandoned",
    ];
    if !AUTORISES.contains(&body.name.as_str()) {
        return Err(ApiError::bad_request(
            "evenement_inconnu",
            "Événement non autorisé.",
        ));
    }
    telemetry::track(
        &state,
        &body.name,
        user.map(|u| u.user_id),
        json!({"source": "front"}),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Son propre profil.
#[utoipa::path(
    get,
    path = "/me",
    tag = "me",
    responses(
        (status = 200, description = "Profil de l'utilisateur connecté", body = UserResponse),
        (status = 401, description = "Non connecté", body = crate::error::ErrorResponse)
    )
)]
pub async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<UserResponse>, ApiError> {
    let compte = infra::auth_repo::find_user_by_id(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Connecte-toi pour continuer."))?;
    Ok(Json(compte.into()))
}

/// Mettre à jour pseudo et code postal.
#[utoipa::path(
    patch,
    path = "/me",
    tag = "me",
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profil mis à jour", body = UserResponse),
        (status = 400, description = "Champ invalide", body = crate::error::ErrorResponse),
        (status = 409, description = "Pseudo déjà pris", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_me(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    regles::valider_pseudo(&body.pseudo).map_err(map_validation)?;
    regles::valider_code_postal(&body.postal_code).map_err(map_validation)?;
    let compte = infra::auth_repo::update_profile(
        &state.pool,
        user.user_id,
        &body.pseudo,
        &body.postal_code,
    )
    .await
    .map_err(map_unique)?;
    Ok(Json(compte.into()))
}
