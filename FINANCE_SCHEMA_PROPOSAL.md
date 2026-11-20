# Finance Module — Schema Proposal (migration 0008)

**Status:** PROPOSED — awaiting owner approval before anything is written.
Per constraint 15, no schema change happens until this is approved.

## Design principles
- **Single owner**, like `fun_profile` — every row is the owner's; no per-user column needed
  (owner-only routes already enforce access).
- **Additive only** — new tables, no change to existing ones. `tasks.area` already allows
  `'finance'`, so finance tasks already fit.
- **Money** stored as `NUMERIC(14,2)` (exact, no float errors), with a per-row `currency`
  defaulting to **`NGN`**. Other currencies may be recorded.
- **Balance/totals are derived** from transactions (not a stored mutable balance) — no bank
  integration, no invented data.
- **Honest limitation:** totals mixing currencies aren't summed across currencies; insights
  group by currency, and NGN is the default/primary. (Stated in UI.)

## Proposed tables

```sql
-- migration 0008_finance.sql  (PROPOSED)

-- Income and expense records.
CREATE TABLE finance_transactions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
    amount      NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    currency    TEXT NOT NULL DEFAULT 'NGN' CHECK (char_length(currency) = 3),
    category    TEXT NOT NULL DEFAULT 'general',   -- free text (e.g. salary, food, transport)
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

-- Repayments against a debt. API decrements debts.outstanding and flips status to 'paid' at 0.
CREATE TABLE finance_repayments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    debt_id    UUID NOT NULL REFERENCES finance_debts(id) ON DELETE CASCADE,
    amount     NUMERIC(14,2) NOT NULL CHECK (amount > 0),
    paid_on    DATE NOT NULL DEFAULT current_date,
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX finance_repay_debt_idx ON finance_repayments (debt_id);
```

## API surface (new routes, owner-only via `Authed`, same patterns as existing modules)
- `GET  /finance/overview` → derived: balance, total/monthly income & expenses, savings
  (income−expense this month), outstanding debt total (per currency).
- `GET/POST /finance/transactions`, `PATCH/DELETE /finance/transactions/:id`
- `GET/POST /finance/budgets` (upsert on month+category), `DELETE /finance/budgets/:id`,
  and budgets returned with computed `spent` + progress.
- `GET/POST /finance/debts`, `PATCH /finance/debts/:id`
- `POST /finance/debts/:id/repayments` (records repayment, decrements outstanding)
- `GET /finance/insights` → monthly trends, income-vs-expense, category breakdown.

## Assistant tools (Phase 5, after module works; added to the n8n Assistant workflow)
Read: `finance_overview`, `list_transactions`, `list_debts`, `summarize_spending`.
Write (with confirmation for consequential actions): `record_income`, `record_expense`,
`record_repayment`. System prompt updated to connect Finance and to **require confirmation before
financially consequential or destructive actions** (and never invent records).

## Frontend (vanilla, new "Finance" view + nav item)
Overview cards, transactions list + add/edit, budgets with progress bars, debts + repayments,
simple insight charts (inline SVG, no paid libs). NGN default; currency selectable per entry.

---
**If you approve**, I write `migrations/0008_finance.sql` exactly as above, implement the API +
frontend, test locally, then extend the assistant. **Tell me any changes** (e.g. you want
multiple accounts/wallets, a savings-goal table, or recurring transactions) and I'll revise before
writing.
