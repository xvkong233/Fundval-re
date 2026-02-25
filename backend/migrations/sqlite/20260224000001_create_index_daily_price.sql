-- Reference index daily close storage (SQLite flavor)

CREATE TABLE IF NOT EXISTS index_daily_price (
  id TEXT PRIMARY KEY,
  index_code TEXT NOT NULL,
  source_name TEXT NOT NULL,
  trade_date TEXT NOT NULL,
  close TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT index_daily_price_unique UNIQUE (index_code, source_name, trade_date)
);

CREATE INDEX IF NOT EXISTS index_daily_price_lookup_idx
  ON index_daily_price(index_code, source_name, trade_date);

