-- Core tables aligned with Django models in backend/api/models.py

CREATE TABLE IF NOT EXISTS fund (
  id UUID PRIMARY KEY,
  fund_code VARCHAR(10) NOT NULL UNIQUE,
  fund_name VARCHAR(100) NOT NULL,
  fund_type VARCHAR(50) NULL,

  latest_nav NUMERIC(10, 4) NULL,
  latest_nav_date DATE NULL,

  estimate_nav NUMERIC(10, 4) NULL,
  estimate_growth NUMERIC(10, 4) NULL,
  estimate_time TIMESTAMPTZ NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS account (
  id UUID PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES auth_user(id) ON DELETE CASCADE,
  name VARCHAR(100) NOT NULL,
  parent_id UUID NULL REFERENCES account(id) ON DELETE CASCADE,
  is_default BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT account_user_name_unique UNIQUE (user_id, name),
  CONSTRAINT default_account_must_be_parent CHECK ((is_default = FALSE) OR (parent_id IS NULL))
);

CREATE INDEX IF NOT EXISTS account_user_id_idx ON account(user_id);
CREATE INDEX IF NOT EXISTS account_parent_id_idx ON account(parent_id);

CREATE TABLE IF NOT EXISTS position (
  id UUID PRIMARY KEY,
  account_id UUID NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  fund_id UUID NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  holding_share NUMERIC(20, 4) NOT NULL DEFAULT 0,
  holding_cost NUMERIC(20, 2) NOT NULL DEFAULT 0,
  holding_nav NUMERIC(10, 4) NOT NULL DEFAULT 0,

  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT position_account_fund_unique UNIQUE (account_id, fund_id)
);

CREATE INDEX IF NOT EXISTS position_account_id_idx ON position(account_id);
CREATE INDEX IF NOT EXISTS position_fund_id_idx ON position(fund_id);

