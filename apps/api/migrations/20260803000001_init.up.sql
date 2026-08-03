-- F0.1 — initialisation du schéma.
-- Table témoin du walking skeleton : prouve que les migrations s'appliquent
-- et donne une cible triviale aux tests d'intégration.
CREATE TABLE app_bootstrap (
    id          SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO app_bootstrap (id) VALUES (1);
