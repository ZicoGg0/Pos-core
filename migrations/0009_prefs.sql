-- User preferences (Settings module). Keyed per user so it is self-scoped; the owner uses the
-- full Settings page. Theme is mirrored here so it can sync across devices, but the browser stays
-- the instant source of truth.
CREATE TABLE user_prefs (
    user_id     UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    currency    TEXT NOT NULL DEFAULT 'NGN' CHECK (char_length(currency) = 3),
    date_format TEXT NOT NULL DEFAULT 'DMY' CHECK (date_format IN ('DMY', 'MDY', 'YMD', 'ISO')),
    timezone    TEXT NOT NULL DEFAULT 'Africa/Lagos',
    theme       TEXT CHECK (theme IN ('light', 'dark', 'system')),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
