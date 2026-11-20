CREATE TABLE fun_profile (
    id                INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),   -- single owner
    age               SMALLINT CHECK (age BETWEEN 1 AND 120),
    about             TEXT,
    social            TEXT CHECK (social IN ('outgoing', 'balanced', 'homebody')),
    hobbies           TEXT[] NOT NULL DEFAULT '{}',
    consumes          TEXT[] NOT NULL DEFAULT '{}',
    has_partner       BOOLEAN NOT NULL DEFAULT false,
    partner_interests TEXT,
    completed_at      TIMESTAMPTZ,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE fun_picks (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       TEXT NOT NULL CHECK (kind IN ('media', 'game', 'activity', 'group')),
    title      TEXT NOT NULL,
    why        TEXT,
    url        TEXT,
    status     TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested', 'liked', 'done', 'rejected')),
    feedback   TEXT,
    source     TEXT NOT NULL DEFAULT 'app',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE weights (
    day  DATE PRIMARY KEY,
    kg   NUMERIC(5,1) NOT NULL CHECK (kg BETWEEN 20 AND 400),
    note TEXT
);
CREATE TABLE workouts (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    day        DATE NOT NULL DEFAULT current_date,
    name       TEXT NOT NULL,
    minutes    INTEGER CHECK (minutes BETWEEN 1 AND 1440),
    notes      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE meals (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    day         DATE NOT NULL DEFAULT current_date,
    slot        TEXT NOT NULL CHECK (slot IN ('breakfast', 'lunch', 'dinner', 'snack')),
    description TEXT NOT NULL,
    source      TEXT NOT NULL DEFAULT 'me',        -- 'me' or 'n8n' (recommended)
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX workouts_day_idx ON workouts (day);
CREATE INDEX meals_day_idx ON meals (day);
