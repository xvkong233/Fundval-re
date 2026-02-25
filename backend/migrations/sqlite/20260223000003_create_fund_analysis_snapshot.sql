-- Fund analysis v2 snapshots (SQLite flavor)

CREATE TABLE IF NOT EXISTS fund_analysis_snapshot (
  id TEXT PRIMARY KEY,
  fund_code TEXT NOT NULL,
  source TEXT NOT NULL,
  profile TEXT NOT NULL,
  as_of_date TEXT NULL,
  result_json TEXT NOT NULL,
  last_task_id TEXT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT fund_analysis_snapshot_unique UNIQUE (fund_code, source, profile)
);

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_fund_code_idx
  ON fund_analysis_snapshot(fund_code);

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_updated_at_idx
  ON fund_analysis_snapshot(updated_at);

