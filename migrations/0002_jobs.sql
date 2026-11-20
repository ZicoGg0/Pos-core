CREATE TABLE jobs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source       TEXT NOT NULL,                 -- remoteok, weworkremotely, remotive, manual, ...
    title        TEXT NOT NULL,
    company      TEXT,
    url          TEXT NOT NULL UNIQUE,          -- dedup key
    description  TEXT,
    category     TEXT,                          -- web-dev, web-design, ai-automation, other
    job_type     TEXT,                          -- freelance, full-time, contract, other
    location     TEXT,
    pay_min      INTEGER,
    pay_max      INTEGER,
    pay_period   TEXT CHECK (pay_period IN ('hour', 'month', 'year')),
    pay_text     TEXT,                          -- raw salary text as published
    score        SMALLINT CHECK (score BETWEEN 0 AND 100),
    score_reason TEXT,
    status       TEXT NOT NULL DEFAULT 'new'
                 CHECK (status IN ('new', 'saved', 'applied', 'interview', 'offer', 'rejected', 'skipped')),
    skip_reason  TEXT,
    notes        TEXT,
    posted_at    TIMESTAMPTZ,
    found_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_at   TIMESTAMPTZ
);
CREATE INDEX jobs_status_score_idx ON jobs (status, score DESC NULLS LAST, found_at DESC);
