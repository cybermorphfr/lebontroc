DROP TABLE trades;
ALTER TABLE proposals DROP COLUMN counter_of;
ALTER TABLE proposals DROP CONSTRAINT proposals_status_check;
ALTER TABLE proposals ADD CONSTRAINT proposals_status_check
    CHECK (status IN ('envoyee','vue','acceptee','refusee','contre_proposee','expiree'));
