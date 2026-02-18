CREATE TABLE IF NOT EXISTS position_operation (
  id UUID PRIMARY KEY,
  account_id UUID NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  fund_id UUID NOT NULL REFERENCES fund(id) ON DELETE CASCADE,

  operation_type VARCHAR(10) NOT NULL,
  operation_date DATE NOT NULL,
  before_15 BOOLEAN NOT NULL DEFAULT TRUE,

  amount NUMERIC(20, 2) NOT NULL,
  share NUMERIC(20, 4) NOT NULL,
  nav NUMERIC(10, 4) NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS position_operation_account_id_idx ON position_operation(account_id);
CREATE INDEX IF NOT EXISTS position_operation_fund_id_idx ON position_operation(fund_id);
CREATE INDEX IF NOT EXISTS position_operation_operation_date_idx ON position_operation(operation_date);

