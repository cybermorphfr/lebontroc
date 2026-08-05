//! Centre de notifications et préférences (F5.3) — plus le hub `notify()`
//! utilisé par tous les modules.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentUser;
use crate::messaging::ws::broadcast_event;
use crate::notification::dto::{
    EmailPrefsResponse, NotificationListResponse, NotificationResponse, UnreadCountResponse,
};
use crate::telemetry;
use crate::AppState;
use domain::notification as regles;

/// Point d'entrée unique des notifications in-app : insère la ligne,
/// pousse le badge en temps réel, trace la télémétrie. L'e-mail éventuel
/// reste au call site (templates dédiés) — gardé par [`email_allowed`].
pub async fn notify(
    state: &AppState,
    user_id: Uuid,
    kind: &'static str,
    title: String,
    body: String,
    link: String,
) {
    debug_assert!(regles::type_valide(kind), "type inconnu : {kind}");
    let payload = json!({"title": title, "body": body});
    match infra::notification_repo::insert_notification(&state.pool, user_id, kind, &payload, &link)
        .await
    {
        Ok(unread) => {
            broadcast_event(
                state,
                [user_id, user_id],
                json!({"type": "notification_new", "unread_count": unread}),
            );
            telemetry::track(
                state,
                "notification_sent",
                Some(user_id),
                json!({"channel": "in_app", "type": kind}),
            )
            .await;
        }
        Err(error) => tracing::error!(%error, kind, "notification in-app en échec"),
    }
}

/// L'e-mail de ce type part-il pour cet utilisateur ? (Gherkin F5.3 : les
/// types verrouillés ignorent les préférences.) Trace `notification_sent`
/// (channel e-mail) quand la réponse est oui.
pub async fn email_allowed(state: &AppState, user_id: Uuid, kind: &'static str) -> bool {
    let prefs = match infra::notification_repo::email_prefs(&state.pool, user_id).await {
        Ok(prefs) => prefs,
        Err(error) => {
            tracing::error!(%error, "lecture des préférences e-mail en échec");
            return true;
        }
    };
    let allowed = regles::email_active(&prefs, kind);
    if allowed {
        telemetry::track(
            state,
            "notification_sent",
            Some(user_id),
            json!({"channel": "email", "type": kind}),
        )
        .await;
    }
    allowed
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// Curseur : notifications strictement antérieures à cet instant.
    pub before: Option<DateTime<Utc>>,
}

/// Mes notifications (30 par page, les plus récentes d'abord).
#[utoipa::path(
    get,
    path = "/notifications",
    tag = "notification",
    params(("before" = Option<String>, Query, description = "Curseur temporel (created_at de la dernière reçue)")),
    responses((status = 200, description = "Notifications", body = NotificationListResponse))
)]
pub async fn list_notifications(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<NotificationListResponse>, ApiError> {
    let notifications =
        infra::notification_repo::list_notifications(&state.pool, user.user_id, query.before)
            .await?;
    let unread_count = infra::notification_repo::unread_count(&state.pool, user.user_id).await?;
    Ok(Json(NotificationListResponse {
        notifications: notifications
            .into_iter()
            .map(|n| NotificationResponse {
                id: n.id,
                title: n.payload["title"].as_str().unwrap_or_default().to_string(),
                body: n.payload["body"].as_str().unwrap_or_default().to_string(),
                r#type: n.r#type,
                link: n.link,
                read: n.read_at.is_some(),
                created_at: n.created_at,
            })
            .collect(),
        unread_count,
    }))
}

/// Le badge de la cloche.
#[utoipa::path(
    get,
    path = "/notifications/unread-count",
    tag = "notification",
    responses((status = 200, description = "Nombre de non-lues", body = UnreadCountResponse))
)]
pub async fn get_unread_count(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<UnreadCountResponse>, ApiError> {
    let unread_count = infra::notification_repo::unread_count(&state.pool, user.user_id).await?;
    Ok(Json(UnreadCountResponse { unread_count }))
}

/// Marquer une notification lue (au clic — trace `notification_opened`).
#[utoipa::path(
    post,
    path = "/notifications/{id}/read",
    tag = "notification",
    params(("id" = Uuid, Path, description = "Identifiant de la notification")),
    responses((status = 204, description = "Marquée lue"))
)]
pub async fn mark_read(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if infra::notification_repo::mark_read(&state.pool, user.user_id, id).await? {
        telemetry::track(
            &state,
            "notification_opened",
            Some(user.user_id),
            json!({"notification_id": id}),
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Tout marquer comme lu.
#[utoipa::path(
    post,
    path = "/notifications/read-all",
    tag = "notification",
    responses((status = 204, description = "Tout est lu"))
)]
pub async fn mark_all_read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<StatusCode, ApiError> {
    infra::notification_repo::mark_all_read(&state.pool, user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn prefs_response(prefs: &serde_json::Value) -> EmailPrefsResponse {
    EmailPrefsResponse {
        proposition_recue: regles::email_active(prefs, "proposition_recue"),
        proposition_cloturee: regles::email_active(prefs, "proposition_cloturee"),
        message_recu: regles::email_active(prefs, "message_recu"),
        evaluation: regles::email_active(prefs, "evaluation"),
        favori: regles::email_active(prefs, "favori"),
    }
}

/// Mes préférences e-mail (types désactivables uniquement).
#[utoipa::path(
    get,
    path = "/me/preferences/notifications",
    tag = "notification",
    responses((status = 200, description = "Préférences e-mail", body = EmailPrefsResponse))
)]
pub async fn get_email_prefs(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<EmailPrefsResponse>, ApiError> {
    let prefs = infra::notification_repo::email_prefs(&state.pool, user.user_id).await?;
    Ok(Json(prefs_response(&prefs)))
}

/// Mettre à jour mes préférences e-mail.
#[utoipa::path(
    put,
    path = "/me/preferences/notifications",
    tag = "notification",
    request_body = EmailPrefsResponse,
    responses((status = 200, description = "Préférences enregistrées", body = EmailPrefsResponse))
)]
pub async fn put_email_prefs(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<EmailPrefsResponse>,
) -> Result<Json<EmailPrefsResponse>, ApiError> {
    let prefs = json!({
        "proposition_recue": body.proposition_recue,
        "proposition_cloturee": body.proposition_cloturee,
        "message_recu": body.message_recu,
        "evaluation": body.evaluation,
        "favori": body.favori,
    });
    infra::notification_repo::set_email_prefs(&state.pool, user.user_id, &prefs).await?;
    Ok(Json(prefs_response(&prefs)))
}

/// Notifie les fans d'un objet du troc (favori réservé / de nouveau
/// disponible) — in-app toujours, e-mail selon préférences (Gherkin F5.3).
pub async fn notify_fans(state: &AppState, trade_id: Uuid, reserved: bool) {
    let fans = match infra::notification_repo::fans_for_trade(&state.pool, trade_id).await {
        Ok(fans) => fans,
        Err(error) => {
            tracing::error!(%error, "recherche des fans en échec");
            return;
        }
    };
    for fan in fans {
        let (title, body) = if reserved {
            (
                "Ton favori vient d'être réservé".to_string(),
                format!("« {} » est en cours de troc. Il peut revenir !", fan.title),
            )
        } else {
            (
                "Ton favori est de nouveau disponible !".to_string(),
                format!("« {} » est reparti dans le fil.", fan.title),
            )
        };
        notify(
            state,
            fan.user_id,
            "favori",
            title,
            body,
            format!("/objet/{}?source=favori", fan.item_id),
        )
        .await;
        if email_allowed(state, fan.user_id, "favori").await {
            let result = if reserved {
                state
                    .mailer
                    .send_favorite_reserved(&fan.email, &fan.pseudo, &fan.title)
                    .await
            } else {
                state
                    .mailer
                    .send_favorite_available(&fan.email, &fan.pseudo, &fan.title)
                    .await
            };
            if let Err(error) = result {
                tracing::error!(%error, "e-mail favori non parti");
            }
        }
    }
}
