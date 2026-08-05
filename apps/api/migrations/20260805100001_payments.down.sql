DROP TABLE payments;
DELETE FROM trades WHERE status = 'attente_paiement';
ALTER TABLE trades DROP CONSTRAINT trades_status_check;
ALTER TABLE trades ADD CONSTRAINT trades_status_check
    CHECK (status IN ('accepte','finalise','annule'));
