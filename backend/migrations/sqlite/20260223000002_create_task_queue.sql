-- Task queue + execution logs (SQLite flavor)

CREATE TABLE IF NOT EXISTS task_job (
  id TEXT PRIMARY KEY,
  task_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  not_before TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  status TEXT NOT NULL DEFAULT 'queued', -- queued | running | done | error
  attempt INTEGER NOT NULL DEFAULT 0,
  error TEXT NULL,
  created_by INTEGER NULL REFERENCES auth_user(id) ON DELETE SET NULL,
  started_at TEXT NULL,
  finished_at TEXT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS task_job_due_idx
  ON task_job(status, not_before, priority);

CREATE TABLE IF NOT EXISTS task_run (
  id TEXT PRIMARY KEY,
  queue_type TEXT NOT NULL, -- crawl_job | task_job
  job_id TEXT NOT NULL,
  job_type TEXT NOT NULL,
  fund_code TEXT NULL,
  source_name TEXT NULL,
  status TEXT NOT NULL, -- running | ok | error
  error TEXT NULL,
  started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  finished_at TEXT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS task_run_status_idx ON task_run(status, started_at);
CREATE INDEX IF NOT EXISTS task_run_finished_idx ON task_run(finished_at);

CREATE TABLE IF NOT EXISTS task_run_log (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES task_run(id) ON DELETE CASCADE,
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS task_run_log_run_idx ON task_run_log(run_id, created_at);

CREATE TABLE IF NOT EXISTS fund_signals_batch_item (
  task_id TEXT NOT NULL REFERENCES task_job(id) ON DELETE CASCADE,
  fund_code TEXT NOT NULL,
  source TEXT NOT NULL,
  as_of_date TEXT NULL,
  best_peer_json TEXT NULL,
  computed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (task_id, fund_code)
);

CREATE INDEX IF NOT EXISTS fund_signals_batch_item_task_idx
  ON fund_signals_batch_item(task_id, fund_code);

