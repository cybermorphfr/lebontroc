-- F1.2 — suppression d'objet : soft delete (la ligne reste pour l'audit et
-- les futures références de trocs), les photos S3 sont purgées.
ALTER TABLE items ADD COLUMN deleted_at TIMESTAMPTZ;
CREATE INDEX items_deleted_idx ON items (deleted_at) WHERE deleted_at IS NOT NULL;
