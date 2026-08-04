-- F0.2 — comptes, sessions, vérification e-mail, télémétrie.
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE users (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email              CITEXT NOT NULL UNIQUE,
    password_hash      TEXT NOT NULL,
    pseudo             CITEXT NOT NULL UNIQUE,
    postal_code        TEXT NOT NULL CHECK (postal_code ~ '^[0-9]{5}$'),
    email_verified_at  TIMESTAMPTZ,
    failed_login_count SMALLINT NOT NULL DEFAULT 0,
    locked_until       TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Une session = un appareil connecté (famille de refresh tokens).
CREATE TABLE sessions (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);

-- Un enregistrement par refresh token émis ; `used_at` non nul + réutilisation
-- = rejeu détecté → révocation de toutes les sessions de l'utilisateur.
CREATE TABLE refresh_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);
CREATE INDEX refresh_tokens_session_id_idx ON refresh_tokens (session_id);

CREATE TABLE email_verification_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);
CREATE INDEX email_verification_tokens_user_id_idx
    ON email_verification_tokens (user_id);

-- Conventions §0.4 : événements produit snake_case, user_id hashé.
CREATE TABLE analytics_events (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name         TEXT NOT NULL,
    user_id_hash TEXT,
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    properties   JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX analytics_events_name_occurred_idx
    ON analytics_events (name, occurred_at);
