-- Double authentification TOTP des administrateurs (spec admin §4.3).
ALTER TABLE users
    ADD COLUMN totp_secret TEXT,
    ADD COLUMN totp_enabled_at TIMESTAMPTZ;

-- La session sait si le second facteur a été vérifié : c'est elle qui
-- « élève » l'accès au panneau.
ALTER TABLE sessions ADD COLUMN totp_verified_at TIMESTAMPTZ;

-- Codes de secours à usage unique (hachés, jamais relisibles).
CREATE TABLE totp_recovery_codes (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash  TEXT NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX totp_recovery_user_idx ON totp_recovery_codes (user_id) WHERE used_at IS NULL;
