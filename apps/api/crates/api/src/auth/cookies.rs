//! Cookies d'authentification httpOnly.
//!
//! Access : JWT 15 min, `Path=/` (préfixe `__Host-` quand Secure).
//! Refresh : opaque 30 jours, confiné à `Path=/api/auth` (le préfixe /api est
//! vu du navigateur ; Traefik le retire avant l'API).

use axum_extra::extract::cookie::{Cookie, SameSite};

use crate::auth::jwt::ACCESS_TOKEN_MINUTES;
use crate::config::AppConfig;

pub const REFRESH_COOKIE: &str = "lbt_refresh";
pub const REFRESH_DAYS: i64 = 30;

pub fn access_cookie_name(secure: bool) -> &'static str {
    if secure {
        "__Host-lbt_access"
    } else {
        "lbt_access"
    }
}

fn base_cookie(name: &str, value: String, secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(name.to_owned(), value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(secure);
    cookie
}

pub fn access_cookie(config: &AppConfig, token: String) -> Cookie<'static> {
    let mut cookie = base_cookie(access_cookie_name(config.cookie_secure), token, config.cookie_secure);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::minutes(ACCESS_TOKEN_MINUTES));
    cookie
}

pub fn refresh_cookie(config: &AppConfig, token: String) -> Cookie<'static> {
    let mut cookie = base_cookie(REFRESH_COOKIE, token, config.cookie_secure);
    cookie.set_path("/api/auth");
    cookie.set_max_age(time::Duration::days(REFRESH_DAYS));
    cookie
}

pub fn expired_access_cookie(config: &AppConfig) -> Cookie<'static> {
    let mut cookie = access_cookie(config, String::new());
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

pub fn expired_refresh_cookie(config: &AppConfig) -> Cookie<'static> {
    let mut cookie = refresh_cookie(config, String::new());
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}
