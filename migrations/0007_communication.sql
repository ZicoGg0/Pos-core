-- Communication: sign-up, pairing codes, 1:1 conversations, messages with media.
ALTER TABLE users ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'owner' CHECK (role IN ('owner', 'member'));

CREATE TABLE pair_codes (
    code       TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX pair_codes_user_idx ON pair_codes (user_id);

CREATE TABLE conversations (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_a     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_b     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (user_a < user_b),
    UNIQUE (user_a, user_b)
);
CREATE INDEX conversations_b_idx ON conversations (user_b);

-- Media is stored in the database (small personal scale; 10 MB cap per file) so the single
-- Postgres backup covers everything.
CREATE TABLE media (
    id              UUID PRIMARY KEY,                  -- client-generated, makes uploads idempotent
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    uploader        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mime            TEXT NOT NULL,
    size            INTEGER NOT NULL,
    data            BYTEA NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE messages (
    seq             BIGSERIAL NOT NULL UNIQUE,          -- server order; clients poll with after_seq
    id              UUID PRIMARY KEY,                   -- client-generated, makes sends idempotent
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    sender          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('text', 'image', 'audio', 'file')),
    body            TEXT,
    media_id        UUID REFERENCES media(id) ON DELETE SET NULL,
    duration_ms     INTEGER,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX messages_conv_seq_idx ON messages (conversation_id, seq);
