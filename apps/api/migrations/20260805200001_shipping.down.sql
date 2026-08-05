DROP TABLE dispute_events;
DROP TABLE shipments;
UPDATE trades SET status = 'annule' WHERE status = 'litige_gele';
ALTER TABLE trades DROP CONSTRAINT trades_status_check;
ALTER TABLE trades ADD CONSTRAINT trades_status_check
    CHECK (status IN ('attente_paiement','accepte','finalise','annule'));
ALTER TABLE payments DROP COLUMN shipping_cents, DROP COLUMN service_cents;
ALTER TABLE payments DROP CONSTRAINT payments_trade_payer_key;
ALTER TABLE payments ADD CONSTRAINT payments_trade_id_key UNIQUE (trade_id);
