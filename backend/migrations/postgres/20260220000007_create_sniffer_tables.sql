-- Sniffer (DeepQ star) snapshots and runs

CREATE TABLE IF NOT EXISTS sniffer_run (
  id UUID PRIMARY KEY,
  source_url TEXT NOT NULL,
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  finished_at TIMESTAMPTZ NULL,
  ok BOOLEAN NOT NULL DEFAULT FALSE,
  item_count INTEGER NOT NULL DEFAULT 0,
  error TEXT NULL,
  snapshot_id UUID NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sniffer_run_started_at_idx ON sniffer_run(started_at DESC);
CREATE INDEX IF NOT EXISTS sniffer_run_ok_started_at_idx ON sniffer_run(ok, started_at DESC);

CREATE TABLE IF NOT EXISTS sniffer_snapshot (
  id UUID PRIMARY KEY,
  source_url TEXT NOT NULL,
  fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  item_count INTEGER NOT NULL,
  run_id UUID NULL REFERENCES sniffer_run(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS sniffer_snapshot_fetched_at_idx ON sniffer_snapshot(fetched_at DESC);

CREATE TABLE IF NOT EXISTS sniffer_item (
  id UUID PRIMARY KEY,
  snapshot_id UUID NOT NULL REFERENCES sniffer_snapshot(id) ON DELETE CASCADE,
  fund_id UUID NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  sector VARCHAR(100) NOT NULL,
  tags TEXT[] NOT NULL DEFAULT '{}',
  star_count INTEGER NULL,
  week_growth NUMERIC(10, 4) NULL,
  year_growth NUMERIC(10, 4) NULL,
  max_drawdown NUMERIC(10, 4) NULL,
  fund_size_text TEXT NULL,

  raw JSONB NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT sniffer_item_snapshot_fund_unique UNIQUE (snapshot_id, fund_id)
);

CREATE INDEX IF NOT EXISTS sniffer_item_snapshot_id_idx ON sniffer_item(snapshot_id);
CREATE INDEX IF NOT EXISTS sniffer_item_fund_id_idx ON sniffer_item(fund_id);
CREATE INDEX IF NOT EXISTS sniffer_item_sector_idx ON sniffer_item(sector);
CREATE INDEX IF NOT EXISTS sniffer_item_tags_gin_idx ON sniffer_item USING GIN(tags);
