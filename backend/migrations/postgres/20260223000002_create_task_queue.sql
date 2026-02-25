-- Task queue + execution logs (Postgres flavor)

CREATE TABLE IF NOT EXISTS task_job (
  id UUID PRIMARY KEY,
  task_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  not_before TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  status TEXT NOT NULL DEFAULT 'queued', -- queued | running | done | error
  attempt INTEGER NOT NULL DEFAULT 0,
  error TEXT NULL,
  created_by BIGINT NULL REFERENCES auth_user(id) ON DELETE SET NULL,
  started_at TIMESTAMPTZ NULL,
  finished_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS task_job_due_idx
  ON task_job(status, not_before, priority);

CREATE TABLE IF NOT EXISTS task_run (
  id UUID PRIMARY KEY,
  queue_type TEXT NOT NULL, -- crawl_job | task_job
  job_id UUID NOT NULL,
  job_type TEXT NOT NULL,
  fund_code TEXT NULL,
  source_name TEXT NULL,
  status TEXT NOT NULL, -- running | ok | error
  error TEXT NULL,
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  finished_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS task_run_status_idx ON task_run(status, started_at DESC);
CREATE INDEX IF NOT EXISTS task_run_finished_idx ON task_run(finished_at DESC);

CREATE TABLE IF NOT EXISTS task_run_log (
  id UUID PRIMARY KEY,
  run_id UUID NOT NULL REFERENCES task_run(id) ON DELETE CASCADE,
  level TEXT NOT NULL, -- INFO | WARN | ERROR | DEBUG
  message TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS task_run_log_run_idx ON task_run_log(run_id, created_at ASC);

-- Async fund signals batch results (lite)
CREATE TABLE IF NOT EXISTS fund_signals_batch_item (
  task_id UUID NOT NULL REFERENCES task_job(id) ON DELETE CASCADE,
  fund_code TEXT NOT NULL,
  source TEXT NOT NULL,
  as_of_date TEXT NULL,
  best_peer_json JSONB NULL,
  computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (task_id, fund_code)
);

CREATE INDEX IF NOT EXISTS fund_signals_batch_item_task_idx
  ON fund_signals_batch_item(task_id, fund_code);

