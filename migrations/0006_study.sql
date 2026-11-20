CREATE TABLE subjects (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name           TEXT NOT NULL UNIQUE,
    goal           TEXT,
    weekly_minutes INTEGER CHECK (weekly_minutes >= 0),
    archived       BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE study_sessions (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    day        DATE NOT NULL DEFAULT current_date,
    minutes    INTEGER NOT NULL CHECK (minutes BETWEEN 1 AND 1440),
    notes      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX study_sessions_day_idx ON study_sessions (day);
-- Spaced review (Leitner boxes 0 to 5). The server decides the next due date from the box.
CREATE TABLE reviews (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id    UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    prompt        TEXT NOT NULL,
    answer        TEXT,
    box           SMALLINT NOT NULL DEFAULT 0 CHECK (box BETWEEN 0 AND 5),
    due_on        DATE NOT NULL DEFAULT current_date,
    last_reviewed DATE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX reviews_due_idx ON reviews (due_on);
