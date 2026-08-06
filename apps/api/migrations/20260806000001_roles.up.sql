-- Rôles d'administration : utilisateur → admin → super_admin.
ALTER TABLE users
    ADD COLUMN role TEXT NOT NULL DEFAULT 'utilisateur'
        CHECK (role IN ('utilisateur', 'admin', 'super_admin')),
    -- Le compte maître : intouchable par les autres administrateurs.
    ADD COLUMN is_master BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX users_role_idx ON users (role) WHERE role <> 'utilisateur';
-- Un seul compte maître, quoi qu'il arrive.
CREATE UNIQUE INDEX users_master_unique ON users (is_master) WHERE is_master;

-- Le journal d'audit nomme désormais l'auteur de chaque action.
ALTER TABLE admin_audit ADD COLUMN actor_id UUID REFERENCES users(id);
