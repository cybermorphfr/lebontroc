DROP INDEX trades_active_idx;
ALTER TABLE trades
    DROP COLUMN proposer_code, DROP COLUMN recipient_code,
    DROP COLUMN proposer_confirmed_at, DROP COLUMN recipient_confirmed_at,
    DROP COLUMN finalized_at, DROP COLUMN cancelled_at,
    DROP COLUMN reminded_at, DROP COLUMN cancel_requested_by;
