-- F5.3 — centre de notifications in-app + préférences e-mail par type.
CREATE TABLE notifications (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Taxonomie fermée (voir domain::notification) : proposition_recue,
    -- proposition_acceptee, proposition_cloturee, message_recu, paiement,
    -- expedition, remise, evaluation, litige, favori.
    type       TEXT NOT NULL,
    -- Titre/corps rendus côté front.
    payload    JSONB NOT NULL DEFAULT '{}',
    -- Lien profond : /trocs/{id}, /objet/{id}…
    link       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at    TIMESTAMPTZ
);
CREATE INDEX notifications_user_idx ON notifications (user_id, created_at DESC);
CREATE INDEX notifications_unread_idx ON notifications (user_id) WHERE read_at IS NULL;

-- Préférences e-mail : {type: false} = coupé, clé absente = activé.
-- Les défauts vivent dans le code, pas en base.
ALTER TABLE users ADD COLUMN email_prefs JSONB NOT NULL DEFAULT '{}';
