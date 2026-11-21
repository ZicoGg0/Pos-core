-- Make the core features per-user. Existing rows are assigned to the owner.
-- On a fresh database these tables are empty (the owner is created after migrations run),
-- so the backfill is a no-op there; on an existing database it moves current data to the owner.
DO $$
DECLARE oid UUID;
BEGIN
  SELECT id INTO oid FROM users WHERE role = 'owner' ORDER BY created_at LIMIT 1;

  -- tasks
  ALTER TABLE tasks ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE tasks SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE tasks ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX tasks_user_idx ON tasks (user_id);

  -- checkins: PK (day, kind) -> (user_id, day, kind)
  ALTER TABLE checkins ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE checkins SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE checkins ALTER COLUMN user_id SET NOT NULL;
  ALTER TABLE checkins DROP CONSTRAINT checkins_pkey;
  ALTER TABLE checkins ADD PRIMARY KEY (user_id, day, kind);

  -- fun_profile: single row (id = 1) -> one row per user
  ALTER TABLE fun_profile ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE fun_profile SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE fun_profile DROP CONSTRAINT fun_profile_pkey;
  ALTER TABLE fun_profile DROP CONSTRAINT fun_profile_id_check;
  ALTER TABLE fun_profile ALTER COLUMN id DROP DEFAULT;
  ALTER TABLE fun_profile ALTER COLUMN user_id SET NOT NULL;
  ALTER TABLE fun_profile ADD PRIMARY KEY (user_id);
  ALTER TABLE fun_profile DROP COLUMN id;

  -- fun_picks
  ALTER TABLE fun_picks ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE fun_picks SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE fun_picks ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX fun_picks_user_idx ON fun_picks (user_id);

  -- weights: PK (day) -> (user_id, day)
  ALTER TABLE weights ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE weights SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE weights ALTER COLUMN user_id SET NOT NULL;
  ALTER TABLE weights DROP CONSTRAINT weights_pkey;
  ALTER TABLE weights ADD PRIMARY KEY (user_id, day);

  -- workouts, meals
  ALTER TABLE workouts ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE workouts SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE workouts ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX workouts_user_idx ON workouts (user_id);
  ALTER TABLE meals ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE meals SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE meals ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX meals_user_idx ON meals (user_id);

  -- subjects: name UNIQUE -> (user_id, name)
  ALTER TABLE subjects ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE subjects SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE subjects ALTER COLUMN user_id SET NOT NULL;
  ALTER TABLE subjects DROP CONSTRAINT subjects_name_key;
  ALTER TABLE subjects ADD CONSTRAINT subjects_user_name_key UNIQUE (user_id, name);

  -- study_sessions, reviews
  ALTER TABLE study_sessions ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE study_sessions SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE study_sessions ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX study_sessions_user_idx ON study_sessions (user_id);
  ALTER TABLE reviews ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE reviews SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE reviews ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX reviews_user_idx ON reviews (user_id);

  -- finance
  ALTER TABLE finance_transactions ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE finance_transactions SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE finance_transactions ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX finance_tx_user_idx ON finance_transactions (user_id);

  ALTER TABLE finance_budgets ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE finance_budgets SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE finance_budgets ALTER COLUMN user_id SET NOT NULL;
  ALTER TABLE finance_budgets DROP CONSTRAINT finance_budgets_month_category_key;
  ALTER TABLE finance_budgets ADD CONSTRAINT finance_budgets_user_month_cat_key UNIQUE (user_id, month, category);

  ALTER TABLE finance_debts ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE finance_debts SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE finance_debts ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX finance_debts_user_idx ON finance_debts (user_id);

  ALTER TABLE finance_repayments ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
  UPDATE finance_repayments SET user_id = oid WHERE user_id IS NULL;
  ALTER TABLE finance_repayments ALTER COLUMN user_id SET NOT NULL;
  CREATE INDEX finance_repay_user_idx ON finance_repayments (user_id);
END $$;
