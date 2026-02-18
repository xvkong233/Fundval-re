CREATE TABLE IF NOT EXISTS fund_nav_history (
  id UUID PRIMARY KEY,
  fund_id UUID NOT NULL REFERENCES fund(id) ON DELETE CASCADE,
  nav_date DATE NOT NULL,
  unit_nav NUMERIC(10, 4) NOT NULL,
  accumulated_nav NUMERIC(10, 4) NULL,
  daily_growth NUMERIC(10, 4) NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT fund_nav_history_fund_date_unique UNIQUE (fund_id, nav_date)
);

CREATE INDEX IF NOT EXISTS fund_nav_history_fund_id_idx ON fund_nav_history(fund_id);
CREATE INDEX IF NOT EXISTS fund_nav_history_nav_date_idx ON fund_nav_history(nav_date);

CREATE TABLE IF NOT EXISTS estimate_accuracy (
  id UUID PRIMARY KEY,
  source_name VARCHAR(50) NOT NULL,
  fund_id UUID NOT NULL REFERENCES fund(id) ON DELETE CASCADE,
  estimate_date DATE NOT NULL,
  estimate_nav NUMERIC(10, 4) NOT NULL,
  actual_nav NUMERIC(10, 4) NULL,
  error_rate NUMERIC(10, 6) NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT estimate_accuracy_unique UNIQUE (source_name, fund_id, estimate_date)
);

CREATE INDEX IF NOT EXISTS estimate_accuracy_fund_date_idx ON estimate_accuracy(fund_id, estimate_date);
CREATE INDEX IF NOT EXISTS estimate_accuracy_source_date_idx ON estimate_accuracy(source_name, estimate_date);
