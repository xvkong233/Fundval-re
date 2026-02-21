CREATE TABLE IF NOT EXISTS ml_sector_model (
  peer_code TEXT NOT NULL,
  task TEXT NOT NULL,
  horizon_days INTEGER NOT NULL,
  feature_names_json TEXT NOT NULL,
  model_json TEXT NOT NULL,
  metrics_json TEXT NOT NULL,
  trained_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (peer_code, task, horizon_days)
);

CREATE TABLE IF NOT EXISTS fund_signal_snapshot (
  fund_code TEXT NOT NULL,
  peer_code TEXT NOT NULL,
  as_of_date DATE NOT NULL,

  position_percentile_0_100 REAL NULL,
  position_bucket TEXT NULL,

  dip_buy_proba_5t REAL NULL,
  dip_buy_proba_20t REAL NULL,
  magic_rebound_proba_5t REAL NULL,
  magic_rebound_proba_20t REAL NULL,

  computed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (fund_code, peer_code, as_of_date)
);

CREATE INDEX IF NOT EXISTS fund_signal_snapshot_fund_code_idx ON fund_signal_snapshot(fund_code);
CREATE INDEX IF NOT EXISTS fund_signal_snapshot_peer_code_idx ON fund_signal_snapshot(peer_code);
CREATE INDEX IF NOT EXISTS fund_signal_snapshot_as_of_date_idx ON fund_signal_snapshot(as_of_date);

