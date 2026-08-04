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

    // La base peut mettre quelques secondes à accepter les connexions au
    // démarrage de la stack : on retente avant d'abandonner.
    let pool = connect_with_retry(&database_url, 10).await?;
    infra::db::run_migrations(&pool).await?;

    let app = api::router(AppState::new(pool, version, config, mailer));

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
    infra::email::EmailSender::smtp(&host, port, username, password, &tls, &from)
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
