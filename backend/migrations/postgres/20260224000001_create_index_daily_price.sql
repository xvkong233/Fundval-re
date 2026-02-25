-- Reference index daily close storage (Postgres flavor)

CREATE TABLE IF NOT EXISTS index_daily_price (
  id UUID PRIMARY KEY,
  index_code TEXT NOT NULL,
  source_name TEXT NOT NULL,
  trade_date DATE NOT NULL,
  close NUMERIC NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT index_daily_price_unique UNIQUE (index_code, source_name, trade_date)
);

CREATE INDEX IF NOT EXISTS index_daily_price_lookup_idx
  ON index_daily_price(index_code, source_name, trade_date);

