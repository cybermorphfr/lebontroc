DROP INDEX items_title_trgm_idx;
DROP INDEX items_search_tsv_idx;
ALTER TABLE items DROP COLUMN search_tsv, DROP COLUMN accepts_soulte;
