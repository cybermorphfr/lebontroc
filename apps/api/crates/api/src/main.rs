//! Point d'entrée du serveur Lebontroc API.

use std::time::Duration;

use api::AppState;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs structurés JSON, filtrables par RUST_LOG.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL manquante"))?;
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let version = match std::env::var("BUILD_SHA") {
        Ok(sha) if !sha.is_empty() => format!("{}+{}", env!("CARGO_PKG_VERSION"), sha),
        _ => env!("CARGO_PKG_VERSION").to_string(),
    };

    let config = api::config::AppConfig::from_env()?;
    let mailer = mailer_from_env()?;
    let photos = photo_store_from_env()?;
    let dispute_photos = dispute_store_from_env()?;

    // La base peut mettre quelques secondes à accepter les connexions au
    // démarrage de la stack : on retente avant d'abandonner.
    let pool = connect_with_retry(&database_url, 10).await?;
    infra::db::run_migrations(&pool).await?;

    if let Err(error) = photos.ensure_bucket().await {
        // Non fatal : MinIO peut être en retard au boot, la présignature
        // échouera proprement tant que le bucket manque.
        tracing::error!(%error, "initialisation du bucket photos en échec");
    }
    if let Err(error) = dispute_photos.ensure_bucket().await {
        tracing::error!(%error, "initialisation du bucket litiges en échec");
    }
    spawn_orphan_purge(pool.clone(), photos.clone());

    let state = AppState::new(pool, version, config, mailer, photos, dispute_photos);
    spawn_proposal_expiry(state.clone());
    spawn_payment_maintenance(state.clone());
    spawn_analytics_jobs(state.clone());
    let app = api::router(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "lebontroc-api démarrée");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn mailer_from_env() -> anyhow::Result<infra::email::EmailSender> {
    let host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(1025);
    let username = std::env::var("SMTP_USERNAME")
        .ok()
        .filter(|v| !v.is_empty());
    let password = std::env::var("SMTP_PASSWORD")
        .ok()
        .filter(|v| !v.is_empty());
    let tls = std::env::var("SMTP_TLS").unwrap_or_else(|_| "none".to_string());
    let from = std::env::var("SMTP_FROM")
        .unwrap_or_else(|_| "Lebontroc <no-reply@lebontroc.brianplus.com>".to_string());
    let reply_to = std::env::var("SMTP_REPLY_TO")
        .ok()
        .filter(|v| !v.is_empty());
    infra::email::EmailSender::smtp(
        &host,
        port,
        username,
        password,
        &tls,
        &from,
        reply_to.as_deref(),
    )
}

fn photo_store_from_env() -> anyhow::Result<infra::s3::PhotoStore> {
    let endpoint =
        std::env::var("S3_ENDPOINT").map_err(|_| anyhow::anyhow!("S3_ENDPOINT manquante"))?;
    let public_endpoint = std::env::var("S3_PUBLIC_ENDPOINT")
        .map_err(|_| anyhow::anyhow!("S3_PUBLIC_ENDPOINT manquante"))?;
    let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "lebontroc-photos".to_string());
    let access_key =
        std::env::var("S3_ACCESS_KEY").map_err(|_| anyhow::anyhow!("S3_ACCESS_KEY manquante"))?;
    let secret_key =
        std::env::var("S3_SECRET_KEY").map_err(|_| anyhow::anyhow!("S3_SECRET_KEY manquante"))?;
    let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    Ok(infra::s3::PhotoStore::s3(
        &endpoint,
        &public_endpoint,
        &bucket,
        &access_key,
        &secret_key,
        &region,
    ))
}

/// Bucket PRIVÉ des pièces de litige (F5.2) — jamais de lecture anonyme.
fn dispute_store_from_env() -> anyhow::Result<infra::s3::PhotoStore> {
    let endpoint =
        std::env::var("S3_ENDPOINT").map_err(|_| anyhow::anyhow!("S3_ENDPOINT manquante"))?;
    let public_endpoint = std::env::var("S3_PUBLIC_ENDPOINT")
        .map_err(|_| anyhow::anyhow!("S3_PUBLIC_ENDPOINT manquante"))?;
    let bucket =
        std::env::var("S3_DISPUTE_BUCKET").unwrap_or_else(|_| "lebontroc-litiges".to_string());
    let access_key =
        std::env::var("S3_ACCESS_KEY").map_err(|_| anyhow::anyhow!("S3_ACCESS_KEY manquante"))?;
    let secret_key =
        std::env::var("S3_SECRET_KEY").map_err(|_| anyhow::anyhow!("S3_SECRET_KEY manquante"))?;
    let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    Ok(infra::s3::PhotoStore::s3_private(
        &endpoint,
        &public_endpoint,
        &bucket,
        &access_key,
        &secret_key,
        &region,
    ))
}

/// Purge des uploads présignés jamais rattachés à un objet (> 24 h).
/// Expire les propositions sans réponse depuis 7 jours (Gherkin F3.1) et
/// notifie les proposants — toutes les heures.
fn spawn_proposal_expiry(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            let count = api::trade::handlers::expire_and_notify(&state).await;
            if count > 0 {
                tracing::info!(count, "propositions expirées");
            }
            let count = api::messaging::handlers::remind_unread(&state).await;
            if count > 0 {
                tracing::info!(count, "relances de messages non lus envoyées");
            }
            let report = api::trade::handlers::maintain_trades(&state).await;
            if report != Default::default() {
                tracing::info!(
                    reminded = report.reminded,
                    cancelled = report.cancelled,
                    payments_expired = report.payments_expired,
                    captures_retried = report.captures_retried,
                    "maintenance des trocs"
                );
            }
        }
    });
}

/// Les paiements ont une date limite courte (30 min quand le payeur accepte
/// lui-même) et l'auto-confirmation des colis peut finaliser un troc : leur
/// maintenance tourne toutes les 10 minutes.
fn spawn_payment_maintenance(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(600)).await;
            let (expired, captures) = api::trade::handlers::maintain_payments(&state).await;
            let confirmed = api::trade::handlers::auto_confirm_shipments(&state).await;
            if expired > 0 || captures > 0 || confirmed > 0 {
                tracing::info!(expired, captures, confirmed, "maintenance paiements/colis");
            }
        }
    });
}

/// F6.2 — export PostHog (si POSTHOG_API_KEY) toutes les 10 minutes, et
/// récap KPI hebdo à l'admin le lundi (idempotent via le journal d'audit).
fn spawn_analytics_jobs(state: AppState) {
    let api_key = std::env::var("POSTHOG_API_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    let host =
        std::env::var("POSTHOG_HOST").unwrap_or_else(|_| "https://eu.i.posthog.com".to_string());
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(600)).await;
            if let Some(api_key) = api_key.as_deref() {
                match infra::analytics::export_to_posthog(&state.pool, api_key, &host).await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "événements exportés vers PostHog"),
                    Err(error) => tracing::error!(%error, "export PostHog en échec"),
                }
            }
            // Lundi : récap KPI, une seule fois (marqueur dans l'audit).
            let now = chrono::Utc::now();
            if chrono::Datelike::weekday(&now) == chrono::Weekday::Mon {
                let already: Result<Option<(i64,)>, _> = sqlx::query_as(
                    "SELECT id FROM admin_audit WHERE action = 'kpi_hebdo' \
                     AND created_at > now() - interval '3 days' LIMIT 1",
                )
                .fetch_optional(&state.pool)
                .await;
                if let Ok(None) = already {
                    if let Ok(kpis) = infra::analytics::weekly_kpis(&state.pool).await {
                        let resume = format!(
                            "inscriptions       : {}\nobjets publiés     : {}\npropositions       : {}\ntrocs créés        : {}\ntrocs finalisés    : {}\n  dont avec soulte : {}\nlitiges ouverts    : {}",
                            kpis.signups, kpis.items_published, kpis.proposals_sent,
                            kpis.trades_created, kpis.trades_finalized,
                            kpis.trades_with_cash, kpis.disputes_opened,
                        );
                        if let Err(error) = state
                            .mailer
                            .send_admin_kpis(&state.config.admin_email, &resume)
                            .await
                        {
                            tracing::error!(%error, "récap KPI hebdo non parti");
                        } else {
                            let _ = infra::admin_repo::record_audit(
                                &state.pool,
                                "kpi_hebdo",
                                "system",
                                "hebdo",
                                None,
                            )
                            .await;
                        }
                    }
                }
            }
        }
    });
}

fn spawn_orphan_purge(pool: sqlx::PgPool, photos: infra::s3::PhotoStore) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
            let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
            match infra::catalog_repo::orphan_uploads_before(&pool, cutoff).await {
                Ok(orphans) => {
                    let count = orphans.len();
                    for orphan in orphans {
                        photos.delete_object(&orphan.s3_key).await;
                        if let Err(error) =
                            infra::catalog_repo::delete_photo_upload(&pool, orphan.photo_id).await
                        {
                            tracing::warn!(%error, "purge d'un upload orphelin en échec");
                        }
                    }
                    if count > 0 {
                        tracing::info!(count, "uploads orphelins purgés");
                    }
                }
                Err(error) => tracing::warn!(%error, "lecture des uploads orphelins en échec"),
            }
        }
    });
}

async fn connect_with_retry(database_url: &str, attempts: u32) -> anyhow::Result<sqlx::PgPool> {
    let mut last_error = None;
    for attempt in 1..=attempts {
        match infra::db::connect(database_url).await {
            Ok(pool) => return Ok(pool),
            Err(error) => {
                tracing::warn!(attempt, %error, "connexion PostgreSQL en échec, nouvelle tentative");
                last_error = Some(error);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("connexion PostgreSQL impossible")))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installation du handler Ctrl+C");
    };
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installation du handler SIGTERM")
            .recv()
            .await;
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("signal d'arrêt reçu, arrêt en douceur");
}
