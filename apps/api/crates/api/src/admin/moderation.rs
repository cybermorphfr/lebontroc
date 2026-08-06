//! Modération des annonces et fiche d'activité d'un membre : l'outillage
//! qui évite d'ouvrir un client SQL pour retirer une annonce abusive ou
//! comprendre qui est la personne derrière un signalement.

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::handlers::record_admin_action;
use crate::error::ApiError;
use crate::extract::AdminActor;
use crate::AppState;

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ItemsQuery {
    /// Sous-chaîne du titre.
    pub q: Option<String>,
    /// `disponible`, `reserve`, `troque` ou `masque`.
    pub status: Option<String>,
    /// Pseudo exact du propriétaire — la branche de l'arborescence.
    pub owner: Option<String>,
    /// N'afficher que les annonces à signalement ouvert.
    pub signalees: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminItemDto {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub value_cents: i32,
    pub category: String,
    pub condition: String,
    pub owner_pseudo: String,
    pub owner_banned: bool,
    pub photo_url: Option<String>,
    pub signalements: i64,
    pub signalements_ouverts: i64,
    pub created_at: DateTime<Utc>,
}

/// La file des annonces, filtrable. Réservée aux super-administrateurs :
/// c'est de la donnée personnelle en volume.
#[utoipa::path(
    get,
    path = "/admin/items",
    tag = "admin",
    params(ItemsQuery),
    responses((status = 200, description = "Annonces filtrées", body = [AdminItemDto]))
)]
pub async fn admin_list_items(
    State(state): State<AppState>,
    admin: AdminActor,
    Query(query): Query<ItemsQuery>,
) -> Result<Json<Vec<AdminItemDto>>, ApiError> {
    admin.require_super()?;
    let nettoyer = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let filtres = infra::admin_items_repo::ItemFilters {
        q: nettoyer(query.q),
        status: nettoyer(query.status),
        owner: nettoyer(query.owner),
        signalees: query.signalees.unwrap_or(false),
        limit: 200,
    };
    let items = infra::admin_items_repo::list_items(&state.pool, &filtres).await?;
    Ok(Json(
        items
            .into_iter()
            .map(|i| AdminItemDto {
                id: i.id,
                title: i.title,
                status: if i.deleted_at.is_some() {
                    "supprime".to_string()
                } else {
                    i.status
                },
                value_cents: i.value_cents,
                category: i.category,
                condition: i.condition,
                owner_pseudo: i.owner_pseudo,
                owner_banned: i.owner_banned,
                photo_url: i.photo_key.map(|k| state.photos.public_url(&k)),
                signalements: i.signalements,
                signalements_ouverts: i.signalements_ouverts,
                created_at: i.created_at,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct OwnerBranchDto {
    pub pseudo: String,
    pub role: String,
    pub banned: bool,
    pub total: i64,
    pub disponibles: i64,
    pub masquees: i64,
    pub signalees: i64,
    pub derniere_publication: Option<DateTime<Utc>>,
}

/// L'arborescence des annonces par membre — l'entrée normale de la
/// modération : on part de qui publie, pas d'une liste plate.
#[utoipa::path(
    get,
    path = "/admin/items/arborescence",
    tag = "admin",
    responses((status = 200, description = "Annonces groupées par membre", body = [OwnerBranchDto]))
)]
pub async fn admin_owners_tree(
    State(state): State<AppState>,
    admin: AdminActor,
) -> Result<Json<Vec<OwnerBranchDto>>, ApiError> {
    admin.require_super()?;
    let branches = infra::admin_items_repo::owners_tree(&state.pool).await?;
    Ok(Json(
        branches
            .into_iter()
            .map(|b| OwnerBranchDto {
                pseudo: b.pseudo,
                role: b.role,
                banned: b.banned,
                total: b.total,
                disponibles: b.disponibles,
                masquees: b.masquees,
                signalees: b.signalees,
                derniere_publication: b.derniere_publication,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ModerateRequest {
    /// `true` retire l'annonce de la vitrine, `false` la remet en ligne.
    pub masquer: bool,
    /// Motif porté au journal et communiqué au membre.
    pub motif: Option<String>,
}

/// Masque ou remet en ligne une annonce. Le propriétaire est prévenu :
/// une modération silencieuse est une modération contestée.
#[utoipa::path(
    post,
    path = "/admin/items/{id}/moderer",
    tag = "admin",
    request_body = ModerateRequest,
    responses(
        (status = 204, description = "Annonce modérée"),
        (status = 404, description = "Annonce absente ou déjà troquée")
    )
)]
pub async fn admin_moderate_item(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(id): Path<Uuid>,
    Json(body): Json<ModerateRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    admin.require_super()?;
    let motif = body
        .motif
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty());
    let Some((titre, owner_id)) =
        infra::admin_items_repo::moderate_item(&state.pool, id, body.masquer).await?
    else {
        return Err(ApiError::not_found(
            "Cette annonce n'existe plus, ou son troc est engagé.",
        ));
    };
    record_admin_action(
        &state,
        admin.user_id,
        if body.masquer {
            "item_masque"
        } else {
            "item_restaure"
        },
        "item",
        &id.to_string(),
        Some(&format!(
            "{titre} — {}",
            motif.unwrap_or("sans motif précisé")
        )),
    )
    .await;
    let (titre_notif, corps) = if body.masquer {
        (
            "Ton annonce a été retirée".to_string(),
            format!(
                "« {titre} » n'est plus visible : {}. Tu peux la corriger puis nous écrire.",
                motif.unwrap_or("elle ne respecte pas les règles de la plateforme")
            ),
        )
    } else {
        (
            "Ton annonce est de nouveau en ligne".to_string(),
            format!("« {titre} » est de nouveau visible par les autres membres."),
        )
    };
    crate::notification::handlers::notify(
        &state,
        owner_id,
        "litige",
        titre_notif,
        corps,
        "/mes-objets".to_string(),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ————— Fiche d'activité d'un membre —————

#[derive(Serialize, ToSchema)]
pub struct UserActivityResponse {
    pub profil: UserProfileDto,
    pub compteurs: UserCountersDto,
    pub annonces: Vec<AdminItemDto>,
    pub trocs: Vec<UserTradeDto>,
    pub signalements: Vec<UserReportDto>,
    pub sanctions: Vec<UserSanctionDto>,
    /// Score de fiabilité (F5.2) : au-delà de 5, sanctions automatiques.
    pub score: i32,
}

#[derive(Serialize, ToSchema)]
pub struct UserProfileDto {
    pub id: Uuid,
    pub pseudo: String,
    pub email: String,
    pub postal_code: String,
    pub commune: Option<String>,
    pub role: String,
    pub is_master: bool,
    pub email_verified: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub restricted_until: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub totp_actif: bool,
    pub created_at: DateTime<Utc>,
    pub derniere_activite: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct UserCountersDto {
    pub annonces: i64,
    pub annonces_masquees: i64,
    pub propositions_envoyees: i64,
    pub propositions_recues: i64,
    pub trocs: i64,
    pub trocs_finalises: i64,
    pub trocs_annules: i64,
    pub messages: i64,
    pub litiges_ouverts_par_lui: i64,
    pub litiges_subis: i64,
    pub signalements_emis: i64,
    pub signalements_recus: i64,
    pub signalements_fondes: i64,
    pub note_moyenne: Option<f64>,
    pub evaluations: i64,
    pub favoris: i64,
    pub blocages_subis: i64,
}

#[derive(Serialize, ToSchema)]
pub struct UserTradeDto {
    pub id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub role: String,
    pub autre_pseudo: String,
    pub cash_cents: i32,
    pub litige: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct UserReportDto {
    pub id: Uuid,
    /// `recu` (il est visé) ou `emis` (il a signalé).
    pub sens: String,
    pub target_type: String,
    pub cible: Option<String>,
    pub autre_pseudo: Option<String>,
    pub reason: String,
    pub comment: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct UserSanctionDto {
    pub event_type: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Toute l'activité d'un membre en une page : le dossier qu'on ouvre
/// quand un signalement tombe.
#[utoipa::path(
    get,
    path = "/admin/users/{pseudo}/activite",
    tag = "admin",
    responses(
        (status = 200, description = "Dossier complet", body = UserActivityResponse),
        (status = 404, description = "Membre inconnu")
    )
)]
pub async fn admin_user_activity(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(pseudo): Path<String>,
) -> Result<Json<UserActivityResponse>, ApiError> {
    admin.require_super()?;
    let profil = infra::admin_items_repo::user_profile(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce membre n'existe pas."))?;
    let compteurs = infra::admin_items_repo::user_counters(&state.pool, profil.id).await?;
    let filtres = infra::admin_items_repo::ItemFilters {
        owner: Some(profil.pseudo.clone()),
        limit: 200,
        ..Default::default()
    };
    let annonces = infra::admin_items_repo::list_items(&state.pool, &filtres).await?;
    let trocs = infra::admin_items_repo::user_trades(&state.pool, profil.id).await?;
    let signalements = infra::admin_items_repo::user_reports(&state.pool, profil.id).await?;
    let sanctions = infra::admin_items_repo::user_sanctions(&state.pool, profil.id).await?;
    let score = infra::dispute_repo::reliability_score(&state.pool, profil.id).await?;

    Ok(Json(UserActivityResponse {
        profil: UserProfileDto {
            id: profil.id,
            pseudo: profil.pseudo,
            email: profil.email,
            postal_code: profil.postal_code,
            commune: profil.commune,
            role: profil.role,
            is_master: profil.is_master,
            email_verified: profil.email_verified,
            banned_at: profil.banned_at,
            restricted_until: profil.restricted_until,
            deleted_at: profil.deleted_at,
            totp_actif: profil.totp_actif,
            created_at: profil.created_at,
            derniere_activite: profil.derniere_activite,
        },
        compteurs: UserCountersDto {
            annonces: compteurs.annonces,
            annonces_masquees: compteurs.annonces_masquees,
            propositions_envoyees: compteurs.propositions_envoyees,
            propositions_recues: compteurs.propositions_recues,
            trocs: compteurs.trocs,
            trocs_finalises: compteurs.trocs_finalises,
            trocs_annules: compteurs.trocs_annules,
            messages: compteurs.messages,
            litiges_ouverts_par_lui: compteurs.litiges_ouverts_par_lui,
            litiges_subis: compteurs.litiges_subis,
            signalements_emis: compteurs.signalements_emis,
            signalements_recus: compteurs.signalements_recus,
            signalements_fondes: compteurs.signalements_fondes,
            note_moyenne: compteurs.note_moyenne,
            evaluations: compteurs.evaluations,
            favoris: compteurs.favoris,
            blocages_subis: compteurs.blocages_subis,
        },
        annonces: annonces
            .into_iter()
            .map(|i| AdminItemDto {
                id: i.id,
                title: i.title,
                status: if i.deleted_at.is_some() {
                    "supprime".to_string()
                } else {
                    i.status
                },
                value_cents: i.value_cents,
                category: i.category,
                condition: i.condition,
                owner_pseudo: i.owner_pseudo,
                owner_banned: i.owner_banned,
                photo_url: i.photo_key.map(|k| state.photos.public_url(&k)),
                signalements: i.signalements,
                signalements_ouverts: i.signalements_ouverts,
                created_at: i.created_at,
            })
            .collect(),
        trocs: trocs
            .into_iter()
            .map(|t| UserTradeDto {
                id: t.id,
                status: t.status,
                delivery_mode: t.delivery_mode,
                role: t.role,
                autre_pseudo: t.autre_pseudo,
                cash_cents: t.cash_cents,
                litige: t.litige,
                created_at: t.created_at,
            })
            .collect(),
        signalements: signalements
            .into_iter()
            .map(|r| UserReportDto {
                id: r.id,
                sens: r.sens,
                target_type: r.target_type,
                cible: r.cible,
                autre_pseudo: r.autre_pseudo,
                reason: r.reason,
                comment: r.comment,
                status: r.status,
                outcome: r.outcome,
                created_at: r.created_at,
            })
            .collect(),
        sanctions: sanctions
            .into_iter()
            .map(|s| UserSanctionDto {
                event_type: s.event_type,
                details: s.details,
                created_at: s.created_at,
            })
            .collect(),
        score,
    }))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SuggestQuery {
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserSuggestionDto {
    pub pseudo: String,
    pub role: String,
    pub is_master: bool,
    pub annonces: i64,
}

/// Autocomplétion des pseudos : taper trois lettres doit suffire.
#[utoipa::path(
    get,
    path = "/admin/users/suggest",
    tag = "admin",
    params(SuggestQuery),
    responses((status = 200, description = "Dix pseudos au plus", body = [UserSuggestionDto]))
)]
pub async fn admin_suggest_users(
    State(state): State<AppState>,
    admin: AdminActor,
    Query(query): Query<SuggestQuery>,
) -> Result<Json<Vec<UserSuggestionDto>>, ApiError> {
    admin.require_super()?;
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let suggestions = infra::admin_items_repo::suggest_users(&state.pool, q).await?;
    Ok(Json(
        suggestions
            .into_iter()
            .map(|s| UserSuggestionDto {
                pseudo: s.pseudo,
                role: s.role,
                is_master: s.is_master,
                annonces: s.annonces,
            })
            .collect(),
    ))
}

// ————— Conversations (modération) —————

#[derive(Serialize, ToSchema)]
pub struct AdminConversationDto {
    pub proposal_id: Uuid,
    pub autre_pseudo: String,
    pub statut_proposition: String,
    pub messages: i64,
    pub signales: i64,
    pub dernier_message: Option<DateTime<Utc>>,
    pub apercu: Option<String>,
}

/// La liste des fils d'un membre. Lister les fils ne révèle qu'un aperçu ;
/// lire un fil entier est un acte distinct, tracé (voir plus bas).
#[utoipa::path(
    get,
    path = "/admin/users/{pseudo}/conversations",
    tag = "admin",
    responses(
        (status = 200, description = "Fils de discussion", body = [AdminConversationDto]),
        (status = 404, description = "Membre inconnu")
    )
)]
pub async fn admin_user_conversations(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(pseudo): Path<String>,
) -> Result<Json<Vec<AdminConversationDto>>, ApiError> {
    admin.require_super()?;
    let profil = infra::admin_items_repo::user_profile(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce membre n'existe pas."))?;
    let fils = infra::admin_items_repo::user_conversations(&state.pool, profil.id).await?;
    Ok(Json(
        fils.into_iter()
            .map(|c| AdminConversationDto {
                proposal_id: c.proposal_id,
                autre_pseudo: c.autre_pseudo,
                statut_proposition: c.statut_proposition,
                messages: c.messages,
                signales: c.signales,
                dernier_message: c.dernier_message,
                apercu: c.apercu,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct AdminConversationDetail {
    pub proposal_id: Uuid,
    pub statut: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    pub cash_cents: i32,
    pub cash_direction: String,
    pub created_at: DateTime<Utc>,
    pub objets_demandes: Option<String>,
    pub objets_offerts: Option<String>,
    pub messages: Vec<AdminMessageDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminMessageDto {
    pub id: Uuid,
    pub sender_pseudo: String,
    pub body: String,
    pub photo_url: Option<String>,
    /// Le masquage automatique des coordonnées a mordu sur ce message.
    pub redacted: bool,
    pub signale: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

/// Le fil complet d'un échange. C'est de la correspondance privée : la
/// lecture est réservée aux super-administrateurs ET inscrite au journal
/// d'audit — qui a lu quoi, quand. Un modérateur qui fouille sans raison
/// laisse une trace.
#[utoipa::path(
    get,
    path = "/admin/conversations/{proposal_id}",
    tag = "admin",
    responses(
        (status = 200, description = "Fil complet", body = AdminConversationDetail),
        (status = 404, description = "Échange inconnu")
    )
)]
pub async fn admin_conversation(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(proposal_id): Path<Uuid>,
) -> Result<Json<AdminConversationDetail>, ApiError> {
    admin.require_super()?;
    let contexte = infra::admin_items_repo::conversation_context(&state.pool, proposal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Cet échange n'existe pas."))?;
    let messages = infra::admin_items_repo::conversation_messages(&state.pool, proposal_id).await?;
    record_admin_action(
        &state,
        admin.user_id,
        "conversation_consultee",
        "proposal",
        &proposal_id.to_string(),
        Some(&format!(
            "{} ↔ {} ({} messages)",
            contexte.proposer_pseudo,
            contexte.recipient_pseudo,
            messages.len()
        )),
    )
    .await;
    Ok(Json(AdminConversationDetail {
        proposal_id: contexte.proposal_id,
        statut: contexte.statut,
        proposer_pseudo: contexte.proposer_pseudo,
        recipient_pseudo: contexte.recipient_pseudo,
        cash_cents: contexte.cash_cents,
        cash_direction: contexte.cash_direction,
        created_at: contexte.created_at,
        objets_demandes: contexte.objets_demandes,
        objets_offerts: contexte.objets_offerts,
        messages: messages
            .into_iter()
            .map(|m| AdminMessageDto {
                id: m.id,
                sender_pseudo: m.sender_pseudo,
                body: m.body,
                photo_url: m.photo_key.map(|k| state.photos.public_url(&k)),
                redacted: m.redacted,
                signale: m.signale,
                created_at: m.created_at,
                read_at: m.read_at,
            })
            .collect(),
    }))
}
