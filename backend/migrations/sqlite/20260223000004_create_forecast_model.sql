-- Global forecast model storage (SQLite flavor)

CREATE TABLE IF NOT EXISTS forecast_model (
  id TEXT PRIMARY KEY,
  model_name TEXT NOT NULL,
  source TEXT NOT NULL,
  horizon INTEGER NOT NULL,
  lag_k INTEGER NOT NULL,
  weights_json TEXT NOT NULL,
  bias REAL NOT NULL,
  mean_json TEXT NOT NULL,
  std_json TEXT NOT NULL,
  residual_sigma REAL NOT NULL,
  sample_count INTEGER NOT NULL DEFAULT 0,
  trained_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT forecast_model_unique UNIQUE (model_name, source, horizon, lag_k)
);

CREATE INDEX IF NOT EXISTS forecast_model_lookup_idx
  ON forecast_model(model_name, source, horizon, lag_k);

