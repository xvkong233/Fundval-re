CREATE TABLE IF NOT EXISTS sim_run (
  id UUID PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES auth_user(id) ON DELETE CASCADE,
  mode TEXT NOT NULL,
  name TEXT NOT NULL DEFAULT '',
  source_name TEXT NOT NULL DEFAULT 'tiantian',
  fund_codes_json TEXT NOT NULL,
  start_date DATE NOT NULL,
  end_date DATE NOT NULL,
  "current_date" DATE NULL,
  calendar_json TEXT NOT NULL,

  initial_cash NUMERIC NOT NULL,
  cash_available NUMERIC NOT NULL,
  cash_frozen NUMERIC NOT NULL DEFAULT 0,

  buy_fee_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
  sell_fee_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
  settlement_days INTEGER NOT NULL DEFAULT 2,

  status TEXT NOT NULL DEFAULT 'created',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sim_run_user_id_idx ON sim_run(user_id);
CREATE INDEX IF NOT EXISTS sim_run_status_idx ON sim_run(status);

CREATE TABLE IF NOT EXISTS sim_position (
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  fund_code TEXT NOT NULL,
  shares_available NUMERIC NOT NULL DEFAULT 0,
  shares_frozen NUMERIC NOT NULL DEFAULT 0,
  avg_cost NUMERIC NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  PRIMARY KEY (run_id, fund_code)
);

CREATE TABLE IF NOT EXISTS sim_cash_receivable (
  id UUID PRIMARY KEY,
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  settle_date DATE NOT NULL,
  amount NUMERIC NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sim_cash_receivable_run_settle_idx
  ON sim_cash_receivable(run_id, settle_date);

CREATE TABLE IF NOT EXISTS sim_order (
  id UUID PRIMARY KEY,
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  trade_date DATE NOT NULL,
  exec_date DATE NOT NULL,
  side TEXT NOT NULL,
  fund_code TEXT NOT NULL,

  amount NUMERIC NULL,
  shares NUMERIC NULL,

  status TEXT NOT NULL DEFAULT 'pending',
  exec_nav NUMERIC NULL,
  fee NUMERIC NULL,
  executed_shares NUMERIC NULL,
  cash_delta NUMERIC NULL,
  settle_date DATE NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sim_order_run_trade_idx ON sim_order(run_id, trade_date);
CREATE INDEX IF NOT EXISTS sim_order_run_exec_idx ON sim_order(run_id, exec_date);
CREATE INDEX IF NOT EXISTS sim_order_run_status_idx ON sim_order(run_id, status);

CREATE TABLE IF NOT EXISTS sim_trade (
  id UUID PRIMARY KEY,
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  order_id UUID NULL REFERENCES sim_order(id) ON DELETE SET NULL,
  exec_date DATE NOT NULL,
  side TEXT NOT NULL,
  fund_code TEXT NOT NULL,
  nav NUMERIC NOT NULL,
  shares NUMERIC NOT NULL,
  gross_amount NUMERIC NOT NULL,
  fee NUMERIC NOT NULL,
  net_amount NUMERIC NOT NULL,
  settle_date DATE NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sim_trade_run_exec_idx ON sim_trade(run_id, exec_date);
CREATE INDEX IF NOT EXISTS sim_trade_run_fund_idx ON sim_trade(run_id, fund_code);

CREATE TABLE IF NOT EXISTS sim_daily_equity (
  run_id UUID NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  date DATE NOT NULL,
  total_equity DOUBLE PRECISION NOT NULL,
  cash_available DOUBLE PRECISION NOT NULL,
  cash_frozen DOUBLE PRECISION NOT NULL,
  cash_receivable DOUBLE PRECISION NOT NULL,
  positions_value DOUBLE PRECISION NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  PRIMARY KEY (run_id, date)
);
