-- Foundation schema. Each later module (jobs, study, finance, ...) adds its own migration.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tasks (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title        TEXT NOT NULL,
    notes        TEXT,
    status       TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done')),
    priority     SMALLINT NOT NULL DEFAULT 3 CHECK (priority BETWEEN 1 AND 4),
    area         TEXT NOT NULL DEFAULT 'daily'
                 CHECK (area IN ('daily', 'health', 'jobs', 'study', 'finance', 'comms', 'fun')),
    due_at       TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    source       TEXT NOT NULL DEFAULT 'app',   -- 'app' or 'n8n'
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX tasks_status_due_idx ON tasks (status, due_at);
CREATE INDEX tasks_area_idx ON tasks (area);

CREATE TABLE checkins (
    day        DATE NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('morning', 'evening')),
    mood       SMALLINT CHECK (mood BETWEEN 1 AND 5),
    energy     SMALLINT CHECK (energy BETWEEN 1 AND 5),
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (day, kind)
);
