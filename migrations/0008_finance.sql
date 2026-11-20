-- Finance module. Single-owner (like fun_profile): every row is the owner's; owner-only routes
-- enforce access. Money is NUMERIC(14,2) with a per-row currency, default NGN. Balance and totals
-- are derived from transactions; there is no stored mutable balance and no bank integration.

-- Income and expense records.
CREATE TABLE finance_transactions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
    amount      NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    currency    TEXT NOT NULL DEFAULT 'NGN' CHECK (char_length(currency) = 3),
    category    TEXT NOT NULL DEFAULT 'general',
    description TEXT,
    occurred_on DATE NOT NULL DEFAULT current_date,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX finance_tx_date_idx ON finance_transactions (occurred_on DESC);
CREATE INDEX finance_tx_kind_idx ON finance_transactions (kind);
CREATE INDEX finance_tx_cat_idx  ON finance_transactions (category);

-- Monthly budget limit per category.
CREATE TABLE finance_budgets (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    month        DATE NOT NULL,                    -- first day of the month, e.g. 2026-11-01
    category     TEXT NOT NULL,
    limit_amount NUMERIC(14,2) NOT NULL CHECK (limit_amount >= 0),
    currency     TEXT NOT NULL DEFAULT 'NGN' CHECK (char_length(currency) = 3),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (month, category)
);

-- Debts owed, with running outstanding balance.
CREATE TABLE finance_debts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creditor        TEXT NOT NULL,
    original_amount NUMERIC(14,2) NOT NULL CHECK (original_amount >= 0),
    outstanding     NUMERIC(14,2) NOT NULL CHECK (outstanding >= 0),
    interest_rate   NUMERIC(5,2) CHECK (interest_rate >= 0),   -- annual %, optional
    currency        TEXT NOT NULL DEFAULT 'NGN' CHECK (char_length(currency) = 3),
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'paid', 'defaulted')),
    opened_on       DATE NOT NULL DEFAULT current_date,
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Repayments against a debt. The API decrements finance_debts.outstanding and flips status to
-- 'paid' when it reaches zero.
CREATE TABLE finance_repayments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    debt_id    UUID NOT NULL REFERENCES finance_debts(id) ON DELETE CASCADE,
    amount     NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    paid_on    DATE NOT NULL DEFAULT current_date,
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX finance_repay_debt_idx ON finance_repayments (debt_id);
