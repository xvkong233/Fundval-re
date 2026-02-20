-- Risk-free rate cache (Postgres flavor)

CREATE TABLE IF NOT EXISTS risk_free_rate_daily (
  id UUID PRIMARY KEY,
  rate_date DATE NOT NULL,
  tenor VARCHAR(10) NOT NULL,
  rate NUMERIC(10, 6) NOT NULL,
  source VARCHAR(50) NOT NULL,
  fetched_at TIMESTAMPTZ NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT risk_free_rate_daily_unique UNIQUE (rate_date, tenor, source)
);

CREATE INDEX IF NOT EXISTS risk_free_rate_daily_date_idx ON risk_free_rate_daily(rate_date);

