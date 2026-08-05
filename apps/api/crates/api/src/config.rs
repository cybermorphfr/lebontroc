//! Configuration applicative, lue une fois au démarrage.

use jsonwebtoken::{DecodingKey, EncodingKey};

pub struct AppConfig {
    pub jwt_encoding: EncodingKey,
    pub jwt_decoding: DecodingKey,
    /// Cookies `Secure` (désactivable en dev http).
    pub cookie_secure: bool,
    /// Base publique du site, ex. `https://lebontroc.brianplus.com`.
    pub app_base_url: String,
    /// Sel du hash des user_id en télémétrie (§0.4).
    pub analytics_salt: String,
    /// Commission plateforme sur les soultes, en basis points (0 en bêta).
    pub payment_fees_bps: u16,
    /// Destinataire des alertes d'exploitation (litiges gelés).
    pub admin_email: String,
}

impl AppConfig {
    pub fn new(
        jwt_secret: &str,
        cookie_secure: bool,
        app_base_url: String,
        analytics_salt: String,
    ) -> Self {
        Self {
            jwt_encoding: EncodingKey::from_secret(jwt_secret.as_bytes()),
            jwt_decoding: DecodingKey::from_secret(jwt_secret.as_bytes()),
            cookie_secure,
            app_base_url,
            analytics_salt,
            payment_fees_bps: 0,
            admin_email: "admin@lebontroc.brianplus.com".to_string(),
        }
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| anyhow::anyhow!("JWT_SECRET manquante"))?;
        if jwt_secret.len() < 32 {
            anyhow::bail!("JWT_SECRET trop courte (32 caractères minimum)");
        }
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v != "false")
            .unwrap_or(true);
        let app_base_url =
            std::env::var("APP_BASE_URL").map_err(|_| anyhow::anyhow!("APP_BASE_URL manquante"))?;
        let analytics_salt = std::env::var("ANALYTICS_SALT")
            .map_err(|_| anyhow::anyhow!("ANALYTICS_SALT manquante"))?;
        let mut config = Self::new(&jwt_secret, cookie_secure, app_base_url, analytics_salt);
        if let Ok(bps) = std::env::var("PAYMENT_FEES_BPS") {
            config.payment_fees_bps = bps
                .parse()
                .map_err(|_| anyhow::anyhow!("PAYMENT_FEES_BPS invalide"))?;
        }
        if let Ok(admin_email) = std::env::var("ADMIN_EMAIL") {
            config.admin_email = admin_email;
        }
        Ok(config)
    }
}
