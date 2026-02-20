-- Crawl job queue (SQLite flavor)

CREATE TABLE IF NOT EXISTS crawl_job (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  fund_code TEXT NULL,
  source_name TEXT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  not_before DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  status TEXT NOT NULL DEFAULT 'queued',
  attempt INTEGER NOT NULL DEFAULT 0,
  last_error TEXT NULL,
  last_ok_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT crawl_job_unique UNIQUE (job_type, fund_code, source_name)
);

CREATE INDEX IF NOT EXISTS crawl_job_due_idx
  ON crawl_job(status, not_before, priority);

CREATE TABLE IF NOT EXISTS crawl_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

