-- F5.2 — signalements, blocages, dossiers de litige, sanctions.

-- Un dossier de litige par troc, ouvert par une partie (ou par le système
-- pour les gels F4.3 : opened_by NULL).
CREATE TABLE disputes (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id      UUID NOT NULL UNIQUE REFERENCES trades(id),
    opened_by     UUID REFERENCES users(id),
    reason        TEXT NOT NULL CHECK (reason IN
        ('non_conforme', 'abime', 'manquant', 'contrefacon', 'jamais_venu', 'non_depot')),
    description   TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 1000),
    -- ouvert → (réponse contradictoire ou 72 h) → en_examen → tranche
    status        TEXT NOT NULL DEFAULT 'ouvert' CHECK (status IN ('ouvert', 'en_examen', 'tranche')),
    response      TEXT CHECK (char_length(response) <= 1000),
    responded_at  TIMESTAMPTZ,
    outcome       TEXT CHECK (outcome IN ('capture', 'liberation', 'rejet')),
    penalty       TEXT CHECK (penalty IN ('avertissement', 'restriction', 'bannissement')),
    penalized_id  UUID REFERENCES users(id),
    admin_note    TEXT,
    opened_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at   TIMESTAMPTZ
);
CREATE INDEX disputes_status_idx ON disputes (status) WHERE status <> 'tranche';

-- Pièces photo des deux parties (bucket privé, jamais d'URL publique).
CREATE TABLE dispute_photos (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dispute_id  UUID NOT NULL REFERENCES disputes(id) ON DELETE CASCADE,
    uploader_id UUID NOT NULL REFERENCES users(id),
    s3_key      TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX dispute_photos_dispute_idx ON dispute_photos (dispute_id);

-- Signalements (objet, utilisateur, message) — enregistrés + e-mail admin ;
-- le workflow de traitement arrive avec le back-office (F6.1).
CREATE TABLE reports (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reporter_id UUID NOT NULL REFERENCES users(id),
    target_type TEXT NOT NULL CHECK (target_type IN ('objet', 'utilisateur', 'message')),
    target_id   UUID NOT NULL,
    reason      TEXT NOT NULL,
    comment     TEXT CHECK (char_length(comment) <= 1000),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Blocage : plus de nouvelles propositions ni de messages dans les deux
-- sens, masquage bidirectionnel du feed et de la recherche.
CREATE TABLE user_blocks (
    blocker_id UUID NOT NULL REFERENCES users(id),
    blocked_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);
CREATE INDEX user_blocks_blocked_idx ON user_blocks (blocked_id);

-- Sanctions (restriction 30 j / bannissement), déclenchées par le score.
ALTER TABLE users
    ADD COLUMN restricted_until TIMESTAMPTZ,
    ADD COLUMN banned_at TIMESTAMPTZ;

-- Les trocs gelés par F4.3 avant F5.2 deviennent des dossiers système.
INSERT INTO disputes (trade_id, opened_by, reason, description, status)
SELECT t.id, NULL, 'non_depot',
       'Dossier créé automatiquement : un seul colis déposé à J+5 (F4.3).',
       'en_examen'
FROM trades t
WHERE t.status = 'litige_gele'
  AND NOT EXISTS (SELECT 1 FROM disputes d WHERE d.trade_id = t.id);
