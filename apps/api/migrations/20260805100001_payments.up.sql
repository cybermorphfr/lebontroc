-- F4.2 — soulte séquestrée : le paiement par préautorisation carte. Un troc
-- avec soulte naît en `attente_paiement` ; il ne devient `accepte` (codes de
-- remise actifs) qu'une fois la préautorisation posée. La capture a lieu à la
-- double confirmation, la libération sur annulation.
ALTER TABLE trades DROP CONSTRAINT trades_status_check;
ALTER TABLE trades ADD CONSTRAINT trades_status_check
    CHECK (status IN ('attente_paiement','accepte','finalise','annule'));

CREATE TABLE payments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- un troc porte au plus un paiement : clé d'idempotence.
    trade_id        UUID NOT NULL UNIQUE REFERENCES trades(id),
    payer_id        UUID NOT NULL REFERENCES users(id),
    beneficiary_id  UUID NOT NULL REFERENCES users(id),
    amount_cents    INTEGER NOT NULL CHECK (amount_cents > 0),
    -- commission plateforme, prélevée sur le montant capturé (0 en bêta).
    fees_cents      INTEGER NOT NULL DEFAULT 0 CHECK (fees_cents >= 0),
    status          TEXT NOT NULL DEFAULT 'en_attente'
                    CHECK (status IN ('en_attente','echoue','sequestre',
                                      'capture','annule','expire')),
    provider        TEXT NOT NULL,
    provider_ref    TEXT,
    failure_reason  TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    -- date limite de préautorisation : passée sans séquestre, le troc
    -- est annulé et les objets libérés.
    deadline        TIMESTAMPTZ NOT NULL,
    escrowed_at     TIMESTAMPTZ,
    captured_at     TIMESTAMPTZ,
    cancelled_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX payments_overdue_idx ON payments (deadline)
    WHERE status IN ('en_attente','echoue');
CREATE INDEX payments_escrowed_idx ON payments (updated_at)
    WHERE status = 'sequestre';
