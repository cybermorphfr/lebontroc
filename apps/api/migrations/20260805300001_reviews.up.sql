-- F5.1 — évaluations : chaque partie note l'autre à la finalisation.
-- Publication simultanée anti-représailles : une note reste invisible tant
-- que l'autre partie n'a pas noté — ou jusqu'à J+14 après la finalisation.
CREATE TABLE reviews (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id     UUID NOT NULL REFERENCES trades(id),
    reviewer_id  UUID NOT NULL REFERENCES users(id),
    reviewee_id  UUID NOT NULL REFERENCES users(id),
    rating       SMALLINT NOT NULL CHECK (rating BETWEEN 1 AND 5),
    comment      TEXT CHECK (char_length(comment) <= 500),
    -- NULL = sous embargo anti-représailles.
    published_at TIMESTAMPTZ,
    -- Réponse publique unique du noté.
    reply        TEXT CHECK (char_length(reply) <= 500),
    reply_at     TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (trade_id, reviewer_id)
);
CREATE INDEX reviews_reviewee_idx ON reviews (reviewee_id) WHERE published_at IS NOT NULL;
CREATE INDEX reviews_embargo_idx ON reviews (trade_id) WHERE published_at IS NULL;
