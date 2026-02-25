-- Sim: add strategy metadata + training rounds (for auto trading / RL experiments)

ALTER TABLE sim_run
  ADD COLUMN IF NOT EXISTS strategy TEXT NOT NULL DEFAULT 'buy_and_hold_equal';

ALTER TABLE sim_run
  ADD COLUMN IF NOT EXISTS strategy_params_json TEXT NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS sim_run_strategy_idx ON sim_run(strategy);

CREATE TABLE IF NOT EXISTS sim_train_round (
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  best_total_return DOUBLE PRECISION NOT NULL,
  best_final_equity DOUBLE PRECISION NOT NULL,
  best_weights_json TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (run_id, round)
);

CREATE INDEX IF NOT EXISTS sim_train_round_run_idx ON sim_train_round(run_id);
