CREATE TABLE IF NOT EXISTS fund_relate_theme (
  fund_code TEXT NOT NULL,
  sec_code TEXT NOT NULL,
  sec_name TEXT NOT NULL,
  corr_1y NUMERIC NULL,
  ol2top NUMERIC NULL,
  source TEXT NOT NULL,
  fetched_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (fund_code, sec_code, source)
);

CREATE INDEX IF NOT EXISTS fund_relate_theme_fund_code_idx ON fund_relate_theme(fund_code);
CREATE INDEX IF NOT EXISTS fund_relate_theme_sec_code_idx ON fund_relate_theme(sec_code);

