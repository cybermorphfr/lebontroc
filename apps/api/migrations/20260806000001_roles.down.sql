ALTER TABLE admin_audit DROP COLUMN actor_id;
DROP INDEX users_master_unique;
DROP INDEX users_role_idx;
ALTER TABLE users DROP COLUMN is_master, DROP COLUMN role;
