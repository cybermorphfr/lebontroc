-- F4.1 — remise en main propre : codes de confirmation croisés (QR + 6
-- chiffres), double confirmation → finalisé, relance J+7, annulation J+14
-- ou d'un commun accord.
ALTER TABLE trades
    ADD COLUMN proposer_code  TEXT,
    ADD COLUMN recipient_code TEXT,
    ADD COLUMN proposer_confirmed_at  TIMESTAMPTZ,
    ADD COLUMN recipient_confirmed_at TIMESTAMPTZ,
    ADD COLUMN finalized_at TIMESTAMPTZ,
    ADD COLUMN cancelled_at TIMESTAMPTZ,
    ADD COLUMN reminded_at  TIMESTAMPTZ,
    ADD COLUMN cancel_requested_by UUID REFERENCES users(id);

CREATE INDEX trades_active_idx ON trades (created_at) WHERE status = 'accepte';
