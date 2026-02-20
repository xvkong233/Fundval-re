-- Crawl job queue (Postgres flavor)

CREATE TABLE IF NOT EXISTS crawl_job (
  id UUID PRIMARY KEY,
  job_type VARCHAR(50) NOT NULL,
  fund_code VARCHAR(10) NULL,
  source_name VARCHAR(50) NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  not_before TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  status VARCHAR(20) NOT NULL DEFAULT 'queued',
  attempt INTEGER NOT NULL DEFAULT 0,
  last_error TEXT NULL,
  last_ok_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  CONSTRAINT crawl_job_unique UNIQUE (job_type, fund_code, source_name)
);

CREATE INDEX IF NOT EXISTS crawl_job_due_idx
  ON crawl_job(status, not_before, priority);

CREATE TABLE IF NOT EXISTS crawl_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

