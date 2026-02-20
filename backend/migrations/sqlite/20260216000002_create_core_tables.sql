-- Core tables aligned with Django models (SQLite flavor)

CREATE TABLE IF NOT EXISTS fund (
  id TEXT PRIMARY KEY,
  fund_code TEXT NOT NULL UNIQUE,
  fund_name TEXT NOT NULL,
  fund_type TEXT NULL,

  latest_nav NUMERIC NULL,
  latest_nav_date DATE NULL,

  estimate_nav NUMERIC NULL,
  estimate_growth NUMERIC NULL,
  estimate_time DATETIME NULL,

  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS account (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES auth_user(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  parent_id TEXT NULL REFERENCES account(id) ON DELETE CASCADE,
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT account_user_name_unique UNIQUE (user_id, name),
  CONSTRAINT default_account_must_be_parent CHECK ((is_default = 0) OR (parent_id IS NULL))
);

CREATE INDEX IF NOT EXISTS account_user_id_idx ON account(user_id);
CREATE INDEX IF NOT EXISTS account_parent_id_idx ON account(parent_id);

CREATE TABLE IF NOT EXISTS position (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  holding_share NUMERIC NOT NULL DEFAULT 0,
  holding_cost NUMERIC NOT NULL DEFAULT 0,
  holding_nav NUMERIC NOT NULL DEFAULT 0,

  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT position_account_fund_unique UNIQUE (account_id, fund_id)
);

CREATE INDEX IF NOT EXISTS position_account_id_idx ON position(account_id);
CREATE INDEX IF NOT EXISTS position_fund_id_idx ON position(fund_id);

