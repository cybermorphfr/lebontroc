-- F3.2 — messagerie : une conversation par proposition, accusés de lecture,
-- masquage des coordonnées mémorisé, relance 24 h.
CREATE TABLE messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    sender_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body        TEXT NOT NULL CHECK (char_length(body) <= 2000),
    photo_key   TEXT,
    redacted    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at     TIMESTAMPTZ,
    reminded_at TIMESTAMPTZ
);
CREATE INDEX messages_proposal_idx ON messages (proposal_id, created_at);
CREATE INDEX messages_unread_idx ON messages (proposal_id, sender_id) WHERE read_at IS NULL;
