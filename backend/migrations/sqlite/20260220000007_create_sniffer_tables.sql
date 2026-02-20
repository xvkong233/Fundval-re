-- Sniffer (DeepQ star) snapshots and runs (SQLite flavor)

CREATE TABLE IF NOT EXISTS sniffer_run (
  id TEXT PRIMARY KEY,
  source_url TEXT NOT NULL,
  started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  finished_at DATETIME NULL,
  ok INTEGER NOT NULL DEFAULT 0,
  item_count INTEGER NOT NULL DEFAULT 0,
  error TEXT NULL,
  snapshot_id TEXT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS sniffer_run_started_at_idx ON sniffer_run(started_at DESC);
CREATE INDEX IF NOT EXISTS sniffer_run_ok_started_at_idx ON sniffer_run(ok, started_at DESC);

CREATE TABLE IF NOT EXISTS sniffer_snapshot (
  id TEXT PRIMARY KEY,
  source_url TEXT NOT NULL,
  fetched_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  item_count INTEGER NOT NULL,
  run_id TEXT NULL REFERENCES sniffer_run(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS sniffer_snapshot_fetched_at_idx ON sniffer_snapshot(fetched_at DESC);

CREATE TABLE IF NOT EXISTS sniffer_item (
  id TEXT PRIMARY KEY,
  snapshot_id TEXT NOT NULL REFERENCES sniffer_snapshot(id) ON DELETE CASCADE,
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  sector TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '{}',
  star_count INTEGER NULL,
  week_growth NUMERIC NULL,
  year_growth NUMERIC NULL,
  max_drawdown NUMERIC NULL,
  fund_size_text TEXT NULL,

  raw TEXT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT sniffer_item_snapshot_fund_unique UNIQUE (snapshot_id, fund_id)
);

CREATE INDEX IF NOT EXISTS sniffer_item_snapshot_id_idx ON sniffer_item(snapshot_id);
CREATE INDEX IF NOT EXISTS sniffer_item_fund_id_idx ON sniffer_item(fund_id);
CREATE INDEX IF NOT EXISTS sniffer_item_sector_idx ON sniffer_item(sector);
