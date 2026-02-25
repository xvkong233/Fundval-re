-- Add refer_index_code to fund_analysis_snapshot and make snapshots unique per reference index (Postgres flavor)

ALTER TABLE fund_analysis_snapshot
  ADD COLUMN IF NOT EXISTS refer_index_code TEXT NOT NULL DEFAULT '1.000001';

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fund_analysis_snapshot_unique') THEN
    ALTER TABLE fund_analysis_snapshot DROP CONSTRAINT fund_analysis_snapshot_unique;
  END IF;
END $$;

ALTER TABLE fund_analysis_snapshot
  ADD CONSTRAINT fund_analysis_snapshot_unique UNIQUE (fund_code, source, profile, refer_index_code);

CREATE INDEX IF NOT EXISTS fund_analysis_snapshot_refer_index_code_idx
  ON fund_analysis_snapshot(refer_index_code);

