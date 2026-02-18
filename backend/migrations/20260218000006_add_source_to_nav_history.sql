-- 多数据源支持：历史净值增加 source_name 维度

ALTER TABLE fund_nav_history
  ADD COLUMN IF NOT EXISTS source_name VARCHAR(50) NOT NULL DEFAULT 'tiantian';

-- 旧约束：同一基金同一日期唯一
ALTER TABLE fund_nav_history
  DROP CONSTRAINT IF EXISTS fund_nav_history_fund_date_unique;

-- 新约束：同一数据源 + 基金 + 日期唯一
ALTER TABLE fund_nav_history
  ADD CONSTRAINT fund_nav_history_source_fund_date_unique UNIQUE (source_name, fund_id, nav_date);

CREATE INDEX IF NOT EXISTS fund_nav_history_source_fund_id_idx
  ON fund_nav_history(source_name, fund_id);

CREATE INDEX IF NOT EXISTS fund_nav_history_source_nav_date_idx
  ON fund_nav_history(source_name, nav_date);

