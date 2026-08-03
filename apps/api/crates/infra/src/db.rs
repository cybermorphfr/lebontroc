//! Pool PostgreSQL, migrations et sonde de disponibilité.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Crée le pool de connexions PostgreSQL.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Applique les migrations SQLx embarquées dans le binaire.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    tracing::info!("migrations appliquées");
    Ok(())
}

/// Sonde de disponibilité : `true` si la base répond.
pub async fn ping(pool: &PgPool) -> bool {
    match sqlx::query("SELECT 1").execute(pool).await {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(%error, "ping base de données en échec");
            false
        }
    }
}
