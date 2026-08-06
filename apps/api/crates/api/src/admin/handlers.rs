//! Handlers du back-office (F6.1/F6.2). Chaque action mutante est
//! journalisée dans `admin_audit` (immuable) + télémétrie `admin_action`.

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::AdminActor;
use crate::telemetry;
use crate::AppState;

/// Journalise une action admin (audit immuable + télémétrie).
pub async fn record_admin_action(
    state: &AppState,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<&str>,
) {
    if let Err(error) = infra::admin_repo::record_audit(
        &state.pool,
        actor_id,
        action,
        target_type,
        target_id,
        details,
    )
    .await
    {
        tracing::error!(%error, action, "journal d'audit en échec");
    }
    telemetry::track(
        state,
        "admin_action",
        None,
        json!({"action_type": action, "target_type": target_type}),
    )
    .await;
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminSearchResponse {
    pub users: Vec<AdminUserDto>,
    pub items: Vec<AdminItemDto>,
    pub trades: Vec<AdminTradeDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminUserDto {
    pub id: Uuid,
    pub pseudo: String,
    /// `utilisateur`, `admin` ou `super_admin`.
    pub role: String,
    /// Compte maître : intouchable par les autres administrateurs.
    pub is_master: bool,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub restricted_until: Option<DateTime<Utc>>,
    pub banned_at: Option<DateTime<Utc>>,
    /// Score de fiabilité interne (jamais public).
    pub score: i32,
}

#[derive(Serialize, ToSchema)]
pub struct AdminItemDto {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub owner_pseudo: String,
}

#[derive(Serialize, ToSchema)]
pub struct AdminTradeDto {
    pub id: Uuid,
    pub status: String,
    pub delivery_mode: String,
    pub proposer_pseudo: String,
    pub recipient_pseudo: String,
    pub created_at: DateTime<Utc>,
}

/// Recherche transverse : utilisateurs, objets, trocs (pseudo, e-mail,
/// titre ou UUID de troc).
#[utoipa::path(
    get,
    path = "/admin/search",
    tag = "admin",
    params(("q" = String, Query, description = "Pseudo, e-mail, titre ou UUID")),
    responses((status = 200, description = "Résultats", body = AdminSearchResponse))
)]
pub async fn admin_search(
    State(state): State<AppState>,
    admin: AdminActor,
    Query(query): Query<SearchQuery>,
) -> Result<Json<AdminSearchResponse>, ApiError> {
    admin.require_super()?;
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(AdminSearchResponse {
            users: Vec::new(),
            items: Vec::new(),
            trades: Vec::new(),
        }));
    }
    let mut users = Vec::new();
    for hit in infra::admin_repo::search_users(&state.pool, q).await? {
        users.push(AdminUserDto {
            score: infra::dispute_repo::reliability_score(&state.pool, hit.id).await?,
            id: hit.id,
            pseudo: hit.pseudo,
            role: hit.role,
            is_master: hit.is_master,
            email: hit.email,
            created_at: hit.created_at,
            restricted_until: hit.restricted_until,
            banned_at: hit.banned_at,
        });
    }
    let items = infra::admin_repo::search_items(&state.pool, q)
        .await?
        .into_iter()
        .map(|i| AdminItemDto {
            id: i.id,
            title: i.title,
            status: i.status,
            owner_pseudo: i.owner_pseudo,
        })
        .collect();
    let trades = infra::admin_repo::search_trades(&state.pool, q)
        .await?
        .into_iter()
        .map(|t| AdminTradeDto {
            id: t.id,
            status: t.status,
            delivery_mode: t.delivery_mode,
            proposer_pseudo: t.proposer_pseudo,
            recipient_pseudo: t.recipient_pseudo,
            created_at: t.created_at,
        })
        .collect();
    Ok(Json(AdminSearchResponse {
        users,
        items,
        trades,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct AdminReportDto {
    pub id: Uuid,
    pub reporter_pseudo: String,
    pub target_type: String,
    pub target_id: Uuid,
    /// Titre de l'annonce ou extrait du message signalé.
    pub target_label: Option<String>,
    /// Pseudo du membre visé — pour ouvrir son dossier en un clic.
    pub target_pseudo: Option<String>,
    pub reason: String,
    pub comment: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ReportsQuery {
    pub status: Option<String>,
}

/// File des signalements.
#[utoipa::path(
    get,
    path = "/admin/reports",
    tag = "admin",
    params(("status" = Option<String>, Query, description = "nouveau | traite")),
    responses((status = 200, description = "Signalements", body = [AdminReportDto]))
)]
pub async fn admin_list_reports(
    State(state): State<AppState>,
    _admin: AdminActor,
    Query(query): Query<ReportsQuery>,
) -> Result<Json<Vec<AdminReportDto>>, ApiError> {
    let reports = infra::admin_repo::list_reports(&state.pool, query.status.as_deref()).await?;
    Ok(Json(
        reports
            .into_iter()
            .map(|r| AdminReportDto {
                id: r.id,
                reporter_pseudo: r.reporter_pseudo,
                target_type: r.target_type,
                target_id: r.target_id,
                target_label: r.target_label,
                target_pseudo: r.target_pseudo,
                reason: r.reason,
                comment: r.comment,
                status: r.status,
                outcome: r.outcome,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct CloseReportRequest {
    /// `fonde` (un signalement fondé sur un utilisateur pèse +2 au score)
    /// ou `rejete`.
    pub outcome: String,
}

/// Clore un signalement. `fonde` sur un utilisateur → score + sanctions.
#[utoipa::path(
    post,
    path = "/admin/reports/{id}/close",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Identifiant du signalement")),
    request_body = CloseReportRequest,
    responses(
        (status = 204, description = "Clos"),
        (status = 400, description = "Issue invalide ou déjà traité", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_close_report(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(id): Path<Uuid>,
    Json(body): Json<CloseReportRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    if !matches!(body.outcome.as_str(), "fonde" | "rejete") {
        return Err(ApiError::bad_request(
            "issue_invalide",
            "L'issue est fonde ou rejete.",
        ));
    }
    let Some(report) = infra::admin_repo::close_report(&state.pool, id, &body.outcome).await?
    else {
        return Err(ApiError::bad_request(
            "deja_traite",
            "Ce signalement n'existe pas ou est déjà traité.",
        ));
    };
    // Un signalement d'utilisateur avéré alimente le score de fiabilité.
    if body.outcome == "fonde" && report.target_type == "utilisateur" {
        if let Err(error) = infra::admin_repo::record_scoring_event(
            &state.pool,
            report.target_id,
            "signalement_fonde",
            &format!("signalement {} fondé", report.reason),
        )
        .await
        {
            tracing::error!(%error, "journal signalement fondé en échec");
        }
        crate::dispute::handlers::apply_score_sanctions(&state, report.target_id).await;
    }
    record_admin_action(
        &state,
        admin.user_id,
        "report_closed",
        &report.target_type,
        &report.target_id.to_string(),
        Some(&format!("{} → {}", report.reason, body.outcome)),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct AuditDto {
    pub id: i64,
    /// Auteur — « service » pour la clé d'exploitation, vide pour les
    /// tâches automatiques.
    pub actor_pseudo: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Journal d'audit (immuable, 200 dernières actions).
#[utoipa::path(
    get,
    path = "/admin/audit",
    tag = "admin",
    responses((status = 200, description = "Journal", body = [AuditDto]))
)]
pub async fn admin_list_audit(
    State(state): State<AppState>,
    _admin: AdminActor,
) -> Result<Json<Vec<AuditDto>>, ApiError> {
    let entries = infra::admin_repo::list_audit(&state.pool).await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|e| AuditDto {
                id: e.id,
                actor_pseudo: e.actor_pseudo,
                action: e.action,
                target_type: e.target_type,
                target_id: e.target_id,
                details: e.details,
                created_at: e.created_at,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct KpisDto {
    pub signups: i64,
    pub items_published: i64,
    pub proposals_sent: i64,
    pub trades_created: i64,
    pub trades_finalized: i64,
    pub trades_with_cash: i64,
    pub disputes_opened: i64,
}

/// KPI des 7 derniers jours (F6.2) — la même source que l'e-mail hebdo.
#[utoipa::path(
    get,
    path = "/admin/kpis",
    tag = "admin",
    responses((status = 200, description = "KPI 7 jours", body = KpisDto))
)]
pub async fn admin_kpis(
    State(state): State<AppState>,
    admin: AdminActor,
) -> Result<Json<KpisDto>, ApiError> {
    admin.require_super()?;
    let kpis = infra::analytics::weekly_kpis(&state.pool).await?;
    Ok(Json(KpisDto {
        signups: kpis.signups,
        items_published: kpis.items_published,
        proposals_sent: kpis.proposals_sent,
        trades_created: kpis.trades_created,
        trades_finalized: kpis.trades_finalized,
        trades_with_cash: kpis.trades_with_cash,
        disputes_opened: kpis.disputes_opened,
    }))
}

// ————— Gestion de l'équipe (super-admin) —————

#[derive(Serialize, ToSchema)]
pub struct StaffMemberDto {
    pub id: Uuid,
    pub pseudo: String,
    /// `admin` ou `super_admin`.
    pub role: String,
    /// Compte maître : ni rétrogradable, ni sanctionnable.
    pub is_master: bool,
}

/// Qui a accès au panneau, et à quel niveau.
#[utoipa::path(
    get,
    path = "/admin/staff",
    tag = "admin",
    responses((status = 200, description = "L'équipe", body = [StaffMemberDto]))
)]
pub async fn admin_list_staff(
    State(state): State<AppState>,
    admin: AdminActor,
) -> Result<Json<Vec<StaffMemberDto>>, ApiError> {
    admin.require_super()?;
    Ok(Json(
        infra::admin_repo::list_staff(&state.pool)
            .await?
            .into_iter()
            .map(|m| StaffMemberDto {
                id: m.id,
                pseudo: m.pseudo,
                role: m.role,
                is_master: m.is_master,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct SetRoleRequest {
    /// `utilisateur` (retrait de l'accès), `admin` ou `super_admin`.
    pub role: String,
}

/// Promouvoir ou rétrograder un compte. Réservé au super-admin ; le compte
/// maître est intouchable et personne ne modifie son propre rôle. Chaque
/// changement est journalisé (auteur, cible, ancien rôle, nouveau rôle).
#[utoipa::path(
    post,
    path = "/admin/users/{pseudo}/role",
    tag = "admin",
    params(("pseudo" = String, Path, description = "Pseudo de la cible")),
    request_body = SetRoleRequest,
    responses(
        (status = 204, description = "Rôle appliqué"),
        (status = 403, description = "Niveau insuffisant, compte maître ou soi-même", body = crate::error::ErrorResponse),
        (status = 404, description = "Inconnu", body = crate::error::ErrorResponse)
    )
)]
pub async fn admin_set_role(
    State(state): State<AppState>,
    admin: AdminActor,
    Path(pseudo): Path<String>,
    Json(body): Json<SetRoleRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let cible = infra::admin_repo::find_role_target(&state.pool, &pseudo)
        .await?
        .ok_or_else(|| ApiError::not_found("Ce troqueur n'existe pas."))?;

    domain::admin::peut_changer_role(
        &admin.role,
        admin.user_id == Some(cible.id),
        cible.is_master,
        &body.role,
    )
    .map_err(map_admin_error)?;

    if cible.role == body.role {
        return Ok(axum::http::StatusCode::NO_CONTENT);
    }
    infra::admin_repo::set_role(&state.pool, cible.id, &body.role).await?;
    record_admin_action(
        &state,
        admin.user_id,
        "role_changed",
        "utilisateur",
        &cible.id.to_string(),
        Some(&format!(
            "{} : {} → {}",
            cible.pseudo, cible.role, body.role
        )),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(crate) fn map_admin_error(error: domain::admin::AdminError) -> ApiError {
    match error {
        domain::admin::AdminError::RoleInvalide => ApiError::bad_request(
            "role_invalide",
            "Les rôles possibles sont : utilisateur, admin, super_admin.",
        ),
        domain::admin::AdminError::NiveauInsuffisant => ApiError::forbidden(
            "super_admin_requis",
            "Seul un super-administrateur gère les rôles.",
        ),
        domain::admin::AdminError::CompteMaitre => ApiError::forbidden(
            "compte_maitre",
            "Le compte maître ne peut être ni rétrogradé ni sanctionné.",
        ),
        domain::admin::AdminError::SoiMeme => ApiError::forbidden(
            "soi_meme",
            "On ne modifie pas son propre rôle — demande à un autre super-administrateur.",
        ),
    }
}

/// Notifie l'équipe d'administration dans l'application (en plus de
/// l'e-mail ADMIN_EMAIL) : nouveau litige, signalement, seuil de sanction.
pub async fn notify_admins(state: &AppState, titre: String, corps: String, lien: String) {
    let equipe = match infra::admin_repo::list_staff(&state.pool).await {
        Ok(equipe) => equipe,
        Err(error) => {
            tracing::error!(%error, "équipe d'administration introuvable");
            return;
        }
    };
    for membre in equipe {
        crate::notification::handlers::notify(
            state,
            membre.id,
            "litige",
            titre.clone(),
            corps.clone(),
            lien.clone(),
        )
        .await;
    }
}

// ————— Tableau de bord (super-admin) —————

#[derive(Serialize, ToSchema)]
pub struct DashboardResponse {
    /// Séries quotidiennes des 30 derniers jours.
    pub series: Vec<DashboardPoint>,
    pub activite: DashboardActivite,
    pub marketplace: DashboardMarketplace,
    pub top_categories: Vec<DashboardTop>,
    pub top_communes: Vec<DashboardTop>,
    pub qualite: DashboardQualite,
    /// Paiements SIMULÉS en bêta : des ordres de grandeur, pas de la
    /// trésorerie.
    pub finances_beta: DashboardFinances,
    pub systeme: DashboardSysteme,
    pub tendance: DashboardTendance,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardPoint {
    pub jour: String,
    pub inscriptions: i64,
    pub annonces: i64,
    pub propositions: i64,
    pub trocs_finalises: i64,
    pub volume_soulte_cents: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardActivite {
    pub inscrits_total: i64,
    pub comptes_supprimes: i64,
    pub comptes_bannis: i64,
    pub comptes_restreints: i64,
    pub dau: i64,
    pub wau: i64,
    pub mau: i64,
    pub recherches_7j: i64,
    pub messages_7j: i64,
    pub favoris_total: i64,
    pub notifications_ouvertes_7j: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardMarketplace {
    pub annonces_actives: i64,
    pub annonces_reservees: i64,
    pub annonces_troquees: i64,
    pub propositions_total: i64,
    pub contre_propositions: i64,
    pub taux_acceptation_pct: f64,
    pub heures_moyennes_avant_accord: Option<f64>,
    pub valeur_echangee_cents: i64,
    pub heures_avant_premier_message: Option<f64>,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardTop {
    pub libelle: String,
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardQualite {
    pub litiges_ouverts: i64,
    pub litiges_en_examen: i64,
    pub litiges_tranches: i64,
    pub heures_moyennes_resolution: Option<f64>,
    pub signalements_en_attente: i64,
    pub note_moyenne: Option<f64>,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardFinances {
    pub soultes_capturees_cents: i64,
    pub soultes_sequestrees_cents: i64,
    pub frais_service_percus_cents: i64,
    pub transport_encaisse_cents: i64,
    pub commissions_cents: i64,
    pub paiements_echoues: i64,
    pub jours_moyens_finalisation: Option<f64>,
    pub colis_expedies: i64,
    pub trocs_envoi_litigieux: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardSysteme {
    pub version: String,
    pub taille_base: String,
    pub evenements_telemetrie: i64,
    pub evenements_non_exportes: i64,
    pub notifications_stockees: i64,
    pub sessions_actives: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DashboardTendance {
    pub litiges_7j: i64,
    pub litiges_7j_precedents: i64,
    pub trocs_7j: i64,
    pub trocs_7j_precedents: i64,
    pub echecs_paiement_7j: i64,
}

/// Le tableau de bord complet — chaque nombre sort des données réelles.
#[utoipa::path(
    get,
    path = "/admin/dashboard",
    tag = "admin",
    responses((status = 200, description = "Toutes les métriques", body = DashboardResponse))
)]
pub async fn admin_dashboard(
    State(state): State<AppState>,
    admin: AdminActor,
) -> Result<Json<DashboardResponse>, ApiError> {
    admin.require_super()?;
    let series = infra::dashboard_repo::daily_series(&state.pool).await?;
    let activite = infra::dashboard_repo::activite(&state.pool).await?;
    let marketplace = infra::dashboard_repo::marketplace(&state.pool).await?;
    let top_categories = infra::dashboard_repo::top_categories(&state.pool).await?;
    let top_communes = infra::dashboard_repo::top_communes(&state.pool).await?;
    let qualite = infra::dashboard_repo::qualite(&state.pool).await?;
    let finances = infra::dashboard_repo::finances_beta(&state.pool).await?;
    let systeme = infra::dashboard_repo::systeme(&state.pool).await?;
    let tendance = infra::dashboard_repo::tendance(&state.pool).await?;
    let to_top = |v: Vec<infra::dashboard_repo::TopEntry>| {
        v.into_iter()
            .map(|t| DashboardTop {
                libelle: t.libelle,
                total: t.total,
            })
            .collect()
    };
    Ok(Json(DashboardResponse {
        series: series
            .into_iter()
            .map(|p| DashboardPoint {
                jour: p.jour.to_string(),
                inscriptions: p.inscriptions,
                annonces: p.annonces,
                propositions: p.propositions,
                trocs_finalises: p.trocs_finalises,
                volume_soulte_cents: p.volume_soulte_cents,
            })
            .collect(),
        activite: DashboardActivite {
            inscrits_total: activite.inscrits_total,
            comptes_supprimes: activite.comptes_supprimes,
            comptes_bannis: activite.comptes_bannis,
            comptes_restreints: activite.comptes_restreints,
            dau: activite.dau,
            wau: activite.wau,
            mau: activite.mau,
            recherches_7j: activite.recherches_7j,
            messages_7j: activite.messages_7j,
            favoris_total: activite.favoris_total,
            notifications_ouvertes_7j: activite.notifications_ouvertes_7j,
        },
        marketplace: DashboardMarketplace {
            annonces_actives: marketplace.annonces_actives,
            annonces_reservees: marketplace.annonces_reservees,
            annonces_troquees: marketplace.annonces_troquees,
            propositions_total: marketplace.propositions_total,
            contre_propositions: marketplace.contre_propositions,
            taux_acceptation_pct: marketplace.taux_acceptation_pct,
            heures_moyennes_avant_accord: marketplace.heures_moyennes_avant_accord,
            valeur_echangee_cents: marketplace.valeur_echangee_cents,
            heures_avant_premier_message: marketplace.heures_avant_premier_message,
        },
        top_categories: to_top(top_categories),
        top_communes: to_top(top_communes),
        qualite: DashboardQualite {
            litiges_ouverts: qualite.litiges_ouverts,
            litiges_en_examen: qualite.litiges_en_examen,
            litiges_tranches: qualite.litiges_tranches,
            heures_moyennes_resolution: qualite.heures_moyennes_resolution,
            signalements_en_attente: qualite.signalements_en_attente,
            note_moyenne: qualite.note_moyenne,
        },
        finances_beta: DashboardFinances {
            soultes_capturees_cents: finances.soultes_capturees_cents,
            soultes_sequestrees_cents: finances.soultes_sequestrees_cents,
            frais_service_percus_cents: finances.frais_service_percus_cents,
            transport_encaisse_cents: finances.transport_encaisse_cents,
            commissions_cents: finances.commissions_cents,
            paiements_echoues: finances.paiements_echoues,
            jours_moyens_finalisation: finances.jours_moyens_finalisation,
            colis_expedies: finances.colis_expedies,
            trocs_envoi_litigieux: finances.trocs_envoi_litigieux,
        },
        systeme: DashboardSysteme {
            version: state.version.clone(),
            taille_base: systeme.taille_base,
            evenements_telemetrie: systeme.evenements_telemetrie,
            evenements_non_exportes: systeme.evenements_non_exportes,
            notifications_stockees: systeme.notifications_stockees,
            sessions_actives: systeme.sessions_actives,
        },
        tendance: DashboardTendance {
            litiges_7j: tendance.litiges_7j,
            litiges_7j_precedents: tendance.litiges_7j_precedents,
            trocs_7j: tendance.trocs_7j,
            trocs_7j_precedents: tendance.trocs_7j_precedents,
            echecs_paiement_7j: tendance.echecs_paiement_7j,
        },
    }))
}

/// Alertes intelligentes : comparaisons hebdomadaires, déclenchées par la
/// maintenance quotidienne — chaque alerte notifie l'équipe (in-app +
/// e-mail) et se journalise pour ne partir qu'une fois par jour.
pub async fn check_alerts(state: &AppState) {
    let tendance = match infra::dashboard_repo::tendance(&state.pool).await {
        Ok(t) => t,
        Err(error) => {
            tracing::error!(%error, "calcul des tendances en échec");
            return;
        }
    };
    let mut alertes: Vec<String> = Vec::new();
    if tendance.litiges_7j >= 3 && tendance.litiges_7j >= tendance.litiges_7j_precedents * 2 {
        alertes.push(format!(
            "Hausse des litiges : {} sur 7 jours (contre {} la semaine précédente).",
            tendance.litiges_7j, tendance.litiges_7j_precedents
        ));
    }
    if tendance.trocs_7j_precedents >= 5 && tendance.trocs_7j * 2 < tendance.trocs_7j_precedents {
        alertes.push(format!(
            "Chute des trocs : {} sur 7 jours (contre {} la semaine précédente).",
            tendance.trocs_7j, tendance.trocs_7j_precedents
        ));
    }
    if tendance.echecs_paiement_7j >= 5 {
        alertes.push(format!(
            "Échecs de paiement en série : {} sur 7 jours.",
            tendance.echecs_paiement_7j
        ));
    }
    for alerte in alertes {
        // Une seule fois par 24 h : le journal fait l'idempotence.
        let deja: Result<Option<(i64,)>, _> = sqlx::query_as(
            "SELECT id FROM admin_audit WHERE action = 'alerte' AND details = $1 \
             AND created_at > now() - interval '24 hours' LIMIT 1",
        )
        .bind(&alerte)
        .fetch_optional(&state.pool)
        .await;
        if !matches!(deja, Ok(None)) {
            continue;
        }
        let _ = infra::admin_repo::record_audit(
            &state.pool,
            None,
            "alerte",
            "system",
            "tendance",
            Some(&alerte),
        )
        .await;
        notify_admins(
            state,
            "📉 Alerte d'exploitation".to_string(),
            alerte.clone(),
            "/admin".to_string(),
        )
        .await;
        if let Err(error) = state
            .mailer
            .send_admin_dispute(&state.config.admin_email, "alerte", &alerte)
            .await
        {
            tracing::error!(%error, "e-mail d'alerte non parti");
        }
    }
}
