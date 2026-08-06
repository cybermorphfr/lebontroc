DROP TABLE totp_recovery_codes;
ALTER TABLE sessions DROP COLUMN totp_verified_at;
ALTER TABLE users DROP COLUMN totp_secret, DROP COLUMN totp_enabled_at;
