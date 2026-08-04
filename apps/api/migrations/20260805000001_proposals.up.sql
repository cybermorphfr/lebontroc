-- F3.1 — propositions de troc : « ça contre ça », soulte plafonnée (règle
-- domaine 50 % du meilleur objet), expiration à 7 jours par tâche de fond.
CREATE TABLE proposals (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposer_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status         TEXT NOT NULL DEFAULT 'envoyee'
                   CHECK (status IN ('envoyee','vue','acceptee','refusee','contre_proposee','expiree')),
    cash_cents     INTEGER NOT NULL DEFAULT 0 CHECK (cash_cents >= 0 AND cash_cents <= 100000),
    cash_direction TEXT NOT NULL DEFAULT 'aucune'
                   CHECK (cash_direction IN ('aucune','du_proposant','du_destinataire')),
    message        TEXT CHECK (char_length(message) <= 500),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    viewed_at      TIMESTAMPTZ,
    expires_at     TIMESTAMPTZ NOT NULL,
    CHECK (proposer_id <> recipient_id),
    CHECK ((cash_cents = 0) = (cash_direction = 'aucune'))
);
CREATE INDEX proposals_recipient_idx ON proposals (recipient_id, created_at DESC);
CREATE INDEX proposals_proposer_idx ON proposals (proposer_id, created_at DESC);
CREATE INDEX proposals_expiry_idx ON proposals (expires_at) WHERE status IN ('envoyee','vue');

-- Les objets de la proposition, avec la valeur figée au moment de l'envoi
-- (les valeurs des objets peuvent changer ensuite).
CREATE TABLE proposal_items (
    proposal_id         UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    item_id             UUID NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    side                TEXT NOT NULL CHECK (side IN ('offert','demande')),
    value_cents_snapshot INTEGER NOT NULL,
    PRIMARY KEY (proposal_id, item_id)
);
CREATE INDEX proposal_items_item_idx ON proposal_items (item_id);
