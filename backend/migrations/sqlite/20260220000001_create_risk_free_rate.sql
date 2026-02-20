-- Risk-free rate cache (SQLite flavor)

CREATE TABLE IF NOT EXISTS risk_free_rate_daily (
  id TEXT PRIMARY KEY,
  rate_date DATE NOT NULL,
  tenor TEXT NOT NULL,
  rate NUMERIC NOT NULL,
  source TEXT NOT NULL,
  fetched_at DATETIME NOT NULL,

  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT risk_free_rate_daily_unique UNIQUE (rate_date, tenor, source)
);

CREATE INDEX IF NOT EXISTS risk_free_rate_daily_date_idx ON risk_free_rate_daily(rate_date);

