CREATE TABLE IF NOT EXISTS sim_run (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES auth_user(id) ON DELETE CASCADE,
  mode TEXT NOT NULL,
  name TEXT NOT NULL DEFAULT '',
  source_name TEXT NOT NULL DEFAULT 'tiantian',
  fund_codes_json TEXT NOT NULL,
  start_date DATE NOT NULL,
  end_date DATE NOT NULL,
  current_date DATE NULL,
  calendar_json TEXT NOT NULL,

  initial_cash TEXT NOT NULL,
  cash_available TEXT NOT NULL,
  cash_frozen TEXT NOT NULL DEFAULT '0',

  buy_fee_rate REAL NOT NULL DEFAULT 0.0,
  sell_fee_rate REAL NOT NULL DEFAULT 0.0,
  settlement_days INTEGER NOT NULL DEFAULT 2,

  status TEXT NOT NULL DEFAULT 'created',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS sim_run_user_id_idx ON sim_run(user_id);
CREATE INDEX IF NOT EXISTS sim_run_status_idx ON sim_run(status);

CREATE TABLE IF NOT EXISTS sim_position (
  run_id TEXT NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  fund_code TEXT NOT NULL,
  shares_available TEXT NOT NULL DEFAULT '0',
  shares_frozen TEXT NOT NULL DEFAULT '0',
  avg_cost TEXT NOT NULL DEFAULT '0',
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (run_id, fund_code)
);

CREATE TABLE IF NOT EXISTS sim_cash_receivable (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  settle_date DATE NOT NULL,
  amount TEXT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS sim_cash_receivable_run_settle_idx
  ON sim_cash_receivable(run_id, settle_date);

CREATE TABLE IF NOT EXISTS sim_order (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  trade_date DATE NOT NULL,
  exec_date DATE NOT NULL,
  side TEXT NOT NULL,
  fund_code TEXT NOT NULL,

  amount TEXT NULL,
  shares TEXT NULL,

  status TEXT NOT NULL DEFAULT 'pending',
  exec_nav TEXT NULL,
  fee TEXT NULL,
  executed_shares TEXT NULL,
  cash_delta TEXT NULL,
  settle_date DATE NULL,

  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS sim_order_run_trade_idx ON sim_order(run_id, trade_date);
CREATE INDEX IF NOT EXISTS sim_order_run_exec_idx ON sim_order(run_id, exec_date);
CREATE INDEX IF NOT EXISTS sim_order_run_status_idx ON sim_order(run_id, status);

CREATE TABLE IF NOT EXISTS sim_trade (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  order_id TEXT NULL REFERENCES sim_order(id) ON DELETE SET NULL,
  exec_date DATE NOT NULL,
  side TEXT NOT NULL,
  fund_code TEXT NOT NULL,
  nav TEXT NOT NULL,
  shares TEXT NOT NULL,
  gross_amount TEXT NOT NULL,
  fee TEXT NOT NULL,
  net_amount TEXT NOT NULL,
  settle_date DATE NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS sim_trade_run_exec_idx ON sim_trade(run_id, exec_date);
CREATE INDEX IF NOT EXISTS sim_trade_run_fund_idx ON sim_trade(run_id, fund_code);

CREATE TABLE IF NOT EXISTS sim_daily_equity (
  run_id TEXT NOT NULL REFERENCES sim_run(id) ON DELETE CASCADE,
  date DATE NOT NULL,
  total_equity REAL NOT NULL,
  cash_available REAL NOT NULL,
  cash_frozen REAL NOT NULL,
  cash_receivable REAL NOT NULL,
  positions_value REAL NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (run_id, date)
);

