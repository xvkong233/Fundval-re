CREATE TABLE IF NOT EXISTS fund_nav_history (
  id TEXT PRIMARY KEY,
  source_name TEXT NOT NULL DEFAULT 'tiantian',
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,
  nav_date DATE NOT NULL,
  unit_nav NUMERIC NOT NULL,
  accumulated_nav NUMERIC NULL,
  daily_growth NUMERIC NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT fund_nav_history_source_fund_date_unique UNIQUE (source_name, fund_id, nav_date)
);

CREATE INDEX IF NOT EXISTS fund_nav_history_source_fund_id_idx
  ON fund_nav_history(source_name, fund_id);
CREATE INDEX IF NOT EXISTS fund_nav_history_source_nav_date_idx
  ON fund_nav_history(source_name, nav_date);

CREATE TABLE IF NOT EXISTS estimate_accuracy (
  id TEXT PRIMARY KEY,
  source_name TEXT NOT NULL,
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,
  estimate_date DATE NOT NULL,
  estimate_nav NUMERIC NOT NULL,
  actual_nav NUMERIC NULL,
  error_rate NUMERIC NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT estimate_accuracy_unique UNIQUE (source_name, fund_id, estimate_date)
);

CREATE INDEX IF NOT EXISTS estimate_accuracy_fund_date_idx ON estimate_accuracy(fund_id, estimate_date);
CREATE INDEX IF NOT EXISTS estimate_accuracy_source_date_idx ON estimate_accuracy(source_name, estimate_date);

