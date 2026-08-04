-- F3.3 — acceptation atomique : le Trade, le statut « caduque » (proposition
-- évincée quand un de ses objets est réservé ailleurs) et la chaîne de
-- contre-propositions.
ALTER TABLE proposals DROP CONSTRAINT proposals_status_check;
ALTER TABLE proposals ADD CONSTRAINT proposals_status_check
    CHECK (status IN ('envoyee','vue','acceptee','refusee','contre_proposee','expiree','caduque'));
ALTER TABLE proposals ADD COLUMN counter_of UUID REFERENCES proposals(id);

CREATE TABLE trades (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- une proposition ne crée qu'un seul troc : c'est la clé d'idempotence.
    proposal_id    UUID NOT NULL UNIQUE REFERENCES proposals(id),
    proposer_id    UUID NOT NULL REFERENCES users(id),
    recipient_id   UUID NOT NULL REFERENCES users(id),
    status         TEXT NOT NULL DEFAULT 'accepte'
                   CHECK (status IN ('accepte','finalise','annule')),
    delivery_mode  TEXT NOT NULL CHECK (delivery_mode IN ('main_propre','envoi')),
    cash_cents     INTEGER NOT NULL DEFAULT 0,
    cash_direction TEXT NOT NULL DEFAULT 'aucune',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
