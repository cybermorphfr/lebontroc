-- Lot 6 — back-office, télémétrie, RGPD.

-- F6.1 : journal d'audit immuable des actions d'administration.
CREATE TABLE admin_audit (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    action      TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    details     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- F6.1 : les signalements deviennent une file de traitement.
ALTER TABLE reports
    ADD COLUMN status TEXT NOT NULL DEFAULT 'nouveau'
        CHECK (status IN ('nouveau', 'traite')),
    ADD COLUMN outcome TEXT CHECK (outcome IN ('fonde', 'rejete')),
    ADD COLUMN resolved_at TIMESTAMPTZ;

-- F6.2 : curseur d'export batch vers PostHog.
ALTER TABLE analytics_events ADD COLUMN exported_at TIMESTAMPTZ;
CREATE INDEX analytics_events_unexported_idx
    ON analytics_events (id) WHERE exported_at IS NULL;

-- F6.1 : un événement de score peut exister hors troc (signalement fondé).
ALTER TABLE dispute_events ALTER COLUMN trade_id DROP NOT NULL;

-- F6.3 : suppression de compte (anonymisation) — marqueur.
ALTER TABLE users ADD COLUMN deleted_at TIMESTAMPTZ;
