-- Add refer_index_code to fund_analysis_snapshot and make snapshots unique per reference index (SQLite flavor)

PRAGMA foreign_keys=off;

ALTER TABLE fund_analysis_snapshot RENAME TO fund_analysis_snapshot_old;

CREATE TABLE fund_analysis_snapshot (
  id TEXT PRIMARY KEY,
  fund_code TEXT NOT NULL,
  source TEXT NOT NULL,
  profile TEXT NOT NULL,
  refer_index_code TEXT NOT NULL DEFAULT '1.000001',
  as_of_date TEXT NULL,
  result_json TEXT NOT NULL,
  last_task_id TEXT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT fund_analysis_snapshot_unique UNIQUE (fund_code, source, profile, refer_index_code)
);

INSERT INTO fund_analysis_snapshot (
  id, fund_code, source, profile, refer_index_code, as_of_date, result_json, last_task_id, created_at, updated_at
)
SELECT
  id, fund_code, source, profile, '1.000001' as refer_index_code, as_of_date, result_json, last_task_id, created_at, updated_at
FROM fund_analysis_snapshot_old;

DROP TABLE fund_analysis_snapshot_old;

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_fund_code_idx
  ON fund_analysis_snapshot(fund_code);

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_refer_index_code_idx
  ON fund_analysis_snapshot(refer_index_code);

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_updated_at_idx
  ON fund_analysis_snapshot(updated_at);

PRAGMA foreign_keys=on;
