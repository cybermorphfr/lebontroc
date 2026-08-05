DELETE FROM dispute_events WHERE trade_id IS NULL;
ALTER TABLE dispute_events ALTER COLUMN trade_id SET NOT NULL;
ALTER TABLE users DROP COLUMN deleted_at;
ALTER TABLE analytics_events DROP COLUMN exported_at;
ALTER TABLE reports DROP COLUMN status, DROP COLUMN outcome, DROP COLUMN resolved_at;
DROP TABLE admin_audit;
