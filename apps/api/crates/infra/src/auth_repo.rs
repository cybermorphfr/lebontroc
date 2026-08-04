//! Requêtes SQL des comptes, sessions et tokens.
//!
//! Convention projet : pas de macros `query!` (pas de cargo local pour
//! régénérer `.sqlx`) — chaque fonction est couverte par un test
//! d'intégration `#[sqlx::test]` qui joue le rôle de vérification SQL.
//! Les colonnes CITEXT sont castées en `::text` dans les SELECT.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub pseudo: String,
    pub postal_code: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub failed_login_count: i16,
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

const USER_COLUMNS: &str = "id, email::text AS email, password_hash, pseudo::text AS pseudo, \
     postal_code, email_verified_at, failed_login_count, locked_until, created_at";

#[derive(Debug)]
pub enum UniqueViolation {
    EmailPris,
    PseudoPris,
    Autre(sqlx::Error),
}

fn map_unique_violation(error: sqlx::Error) -> UniqueViolation {
    if let Some(db_error) = error.as_database_error() {
        match db_error.constraint() {
            Some("users_email_key") => return UniqueViolation::EmailPris,
            Some("users_pseudo_key") => return UniqueViolation::PseudoPris,
            _ => {}
        }
    }
    UniqueViolation::Autre(error)
}

pub async fn create_user(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    pseudo: &str,
    postal_code: &str,
) -> Result<User, UniqueViolation> {
    let sql = format!(
        "INSERT INTO users (email, password_hash, pseudo, postal_code) \
         VALUES ($1, $2, $3, $4) RETURNING {USER_COLUMNS}"
    );
    sqlx::query_as::<_, User>(&sql)
        .bind(email)
        .bind(password_hash)
        .bind(pseudo)
        .bind(postal_code)
        .fetch_one(pool)
        .await
        .map_err(map_unique_violation)
}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> sqlx::Result<Option<User>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE email = $1");
    sqlx::query_as::<_, User>(&sql).bind(email).fetch_optional(pool).await
}

pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<User>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1");
    sqlx::query_as::<_, User>(&sql).bind(id).fetch_optional(pool).await
}

pub async fn update_profile(
    pool: &PgPool,
    user_id: Uuid,
    pseudo: &str,
    postal_code: &str,
) -> Result<User, UniqueViolation> {
    let sql = format!(
        "UPDATE users SET pseudo = $2, postal_code = $3, updated_at = now() \
         WHERE id = $1 RETURNING {USER_COLUMNS}"
    );
    sqlx::query_as::<_, User>(&sql)
        .bind(user_id)
        .bind(pseudo)
        .bind(postal_code)
        .fetch_one(pool)
        .await
        .map_err(map_unique_violation)
}

/// Pose `email_verified_at` s'il ne l'est pas déjà. Idempotent.
pub async fn set_email_verified(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1 AND email_verified_at IS NULL")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn record_login_failure(
    pool: &PgPool,
    user_id: Uuid,
    locked_until: Option<DateTime<Utc>>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE users SET failed_login_count = failed_login_count + 1, \
         locked_until = COALESCE($2, locked_until) WHERE id = $1",
    )
    .bind(user_id)
    .bind(locked_until)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_login_failures(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET failed_login_count = 0, locked_until = NULL WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ————— Sessions et refresh tokens —————

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
) -> sqlx::Result<Session> {
    sqlx::query_as::<_, Session>(
        "INSERT INTO sessions (user_id, user_agent) VALUES ($1, $2) RETURNING *",
    )
    .bind(user_id)
    .bind(user_agent)
    .fetch_one(pool)
    .await
}

pub async fn list_active_sessions(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<Session>> {
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE user_id = $1 AND revoked_at IS NULL \
         ORDER BY last_used_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Révoque une session de l'utilisateur. Retourne `false` si elle n'existe pas
/// ou ne lui appartient pas.
pub async fn revoke_session(pool: &PgPool, session_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = now() \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn revoke_all_sessions(pool: &PgPool, user_id: Uuid) -> sqlx::Result<u64> {
    let result =
        sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(user_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

/// Révoque toutes les sessions de l'utilisateur sauf celle indiquée.
pub async fn revoke_other_sessions(
    pool: &PgPool,
    user_id: Uuid,
    keep_session_id: Uuid,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = now() \
         WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(keep_session_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn touch_session(pool: &PgPool, session_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE sessions SET last_used_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_refresh_token(
    pool: &PgPool,
    session_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO refresh_tokens (session_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(session_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
pub struct RefreshTokenLookup {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub session_revoked_at: Option<DateTime<Utc>>,
}

pub async fn find_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> sqlx::Result<Option<RefreshTokenLookup>> {
    sqlx::query_as::<_, RefreshTokenLookup>(
        "SELECT rt.id, rt.session_id, s.user_id, rt.expires_at, rt.used_at, \
                s.revoked_at AS session_revoked_at \
         FROM refresh_tokens rt JOIN sessions s ON s.id = rt.session_id \
         WHERE rt.token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn mark_refresh_token_used(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE refresh_tokens SET used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ————— Vérification e-mail —————

pub async fn create_verification_token(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Invalide les tokens de vérification encore actifs de l'utilisateur.
pub async fn invalidate_verification_tokens(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE email_verification_tokens SET used_at = now() \
         WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
pub struct VerificationTokenLookup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

pub async fn find_verification_token(
    pool: &PgPool,
    token_hash: &str,
) -> sqlx::Result<Option<VerificationTokenLookup>> {
    sqlx::query_as::<_, VerificationTokenLookup>(
        "SELECT id, user_id, expires_at, used_at FROM email_verification_tokens \
         WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn mark_verification_token_used(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE email_verification_tokens SET used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Date d'émission du dernier token de vérification (pour le cooldown de renvoi).
pub async fn last_verification_token_at(
    pool: &PgPool,
    user_id: Uuid,
) -> sqlx::Result<Option<DateTime<Utc>>> {
    let row: Option<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT created_at FROM email_verification_tokens \
         WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}
