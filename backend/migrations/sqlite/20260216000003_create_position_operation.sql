CREATE TABLE IF NOT EXISTS position_operation (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  fund_id TEXT NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  operation_type TEXT NOT NULL,
  operation_date DATE NOT NULL,
  before_15 INTEGER NOT NULL DEFAULT 1,

  amount NUMERIC NOT NULL,
  share NUMERIC NOT NULL,
  nav NUMERIC NOT NULL,

  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS position_operation_account_id_idx ON position_operation(account_id);
CREATE INDEX IF NOT EXISTS position_operation_fund_id_idx ON position_operation(fund_id);
CREATE INDEX IF NOT EXISTS position_operation_operation_date_idx ON position_operation(operation_date);

