-- F4.3 — envoi croisé : deux colis par troc (un par direction), formats
-- forfaitaires S/M/L, point relais de réception choisi par le destinataire.
-- Un troc en mode envoi fait payer CHAQUE partie (transport + service
-- + soulte éventuelle) : la contrainte « un paiement par troc » devient
-- « un paiement par troc et par payeur ».
ALTER TABLE payments DROP CONSTRAINT payments_trade_id_key;
ALTER TABLE payments ADD CONSTRAINT payments_trade_payer_key UNIQUE (trade_id, payer_id);
ALTER TABLE payments
    ADD COLUMN shipping_cents INTEGER NOT NULL DEFAULT 0 CHECK (shipping_cents >= 0),
    ADD COLUMN service_cents  INTEGER NOT NULL DEFAULT 0 CHECK (service_cents >= 0);

-- Le gel de litige (résolution manuelle en attendant F5.2).
ALTER TABLE trades DROP CONSTRAINT trades_status_check;
ALTER TABLE trades ADD CONSTRAINT trades_status_check
    CHECK (status IN ('attente_paiement','accepte','finalise','annule','litige_gele'));

CREATE TABLE shipments (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id          UUID NOT NULL REFERENCES trades(id),
    sender_id         UUID NOT NULL REFERENCES users(id),
    recipient_id      UUID NOT NULL REFERENCES users(id),
    status            TEXT NOT NULL DEFAULT 'preparation'
                      CHECK (status IN ('preparation','etiquette','depose','transit',
                                        'arrive','retire','confirme','incident','annule')),
    -- format choisi par l'expéditeur, relais choisi par le DESTINATAIRE.
    format            TEXT CHECK (format IN ('s','m','l')),
    relay_code        TEXT,
    relay_name        TEXT,
    relay_address     TEXT,
    provider          TEXT,
    provider_ref      TEXT,
    drop_code         TEXT,
    label_generated_at TIMESTAMPTZ,
    dropped_at        TIMESTAMPTZ,
    in_transit_at     TIMESTAMPTZ,
    arrived_at        TIMESTAMPTZ,
    picked_up_at      TIMESTAMPTZ,
    confirmed_at      TIMESTAMPTZ,
    issue_reason      TEXT,
    issue_reported_at TIMESTAMPTZ,
    -- 0 = aucun rappel de dépôt envoyé, puis 1 (J+2), 2 (J+4).
    drop_reminders    INTEGER NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (trade_id, sender_id)
);
CREATE INDEX shipments_open_idx ON shipments (trade_id)
    WHERE status NOT IN ('confirme','incident','annule');

-- Journal des défaillances : rien n'est sanctionné avant F5.2, mais rien
-- n'est perdu — F5.2 le consommera rétroactivement.
CREATE TABLE dispute_events (
    id         BIGSERIAL PRIMARY KEY,
    trade_id   UUID NOT NULL REFERENCES trades(id),
    event_type TEXT NOT NULL,
    culprit_id UUID REFERENCES users(id),
    details    TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
