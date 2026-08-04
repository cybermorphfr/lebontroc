-- F2.2 — recherche plein texte (FTS français) + tolérance aux fautes (pg_trgm)
-- + « accepte une soulte » au niveau de l'objet (filtre de recherche ; le
-- curseur de soulte réel arrive en F3.1).
CREATE EXTENSION IF NOT EXISTS pg_trgm;

ALTER TABLE items
    ADD COLUMN accepts_soulte BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN search_tsv tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('french', title), 'A')
        || setweight(to_tsvector('french', description), 'B')
    ) STORED;

CREATE INDEX items_search_tsv_idx ON items USING GIN (search_tsv);
CREATE INDEX items_title_trgm_idx ON items USING GIN (title gin_trgm_ops);
