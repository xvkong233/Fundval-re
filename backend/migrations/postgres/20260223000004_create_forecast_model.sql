-- Global forecast model storage (Postgres flavor)

CREATE TABLE IF NOT EXISTS forecast_model (
  id UUID PRIMARY KEY,
  model_name TEXT NOT NULL,
  source TEXT NOT NULL,
  horizon INTEGER NOT NULL,
  lag_k INTEGER NOT NULL,
  weights_json TEXT NOT NULL,
  bias DOUBLE PRECISION NOT NULL,
  mean_json TEXT NOT NULL,
  std_json TEXT NOT NULL,
  residual_sigma DOUBLE PRECISION NOT NULL,
  sample_count BIGINT NOT NULL DEFAULT 0,
  trained_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT forecast_model_unique UNIQUE (model_name, source, horizon, lag_k)
);

CREATE INDEX IF NOT EXISTS forecast_model_lookup_idx
  ON forecast_model(model_name, source, horizon, lag_k);

