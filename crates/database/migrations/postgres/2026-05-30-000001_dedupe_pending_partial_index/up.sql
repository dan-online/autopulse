-- Collapse pre-existing duplicates so the partial unique index can be created.
-- For each file_path with multiple pending/retry rows, keep the most recently
-- updated row and drop the rest. Merge the longest can_process and any existing
-- hash into the survivor so upgrading doesn't shorten settle timers or discard
-- hash validation. This is the historical state #369 created before the dedupe
-- logic landed; without it, the CREATE INDEX below would fail on any
-- installation that already accumulated duplicates.
UPDATE scan_events AS survivor
SET
  can_process = (
    SELECT MAX(duplicate.can_process)
    FROM scan_events AS duplicate
    WHERE duplicate.file_path = survivor.file_path
      AND duplicate.process_status IN ('pending', 'retry')
  ),
  file_hash = COALESCE(
    survivor.file_hash,
    (
      SELECT duplicate.file_hash
      FROM scan_events AS duplicate
      WHERE duplicate.file_path = survivor.file_path
        AND duplicate.process_status IN ('pending', 'retry')
        AND duplicate.file_hash IS NOT NULL
      ORDER BY duplicate.updated_at DESC, duplicate.id DESC
      LIMIT 1
    )
  )
WHERE survivor.process_status IN ('pending', 'retry')
  AND 1 < (
    SELECT COUNT(*)
    FROM scan_events AS duplicate
    WHERE duplicate.file_path = survivor.file_path
      AND duplicate.process_status IN ('pending', 'retry')
  );

DELETE FROM scan_events
WHERE process_status IN ('pending', 'retry')
  AND id NOT IN (
    SELECT id FROM (
      SELECT id, ROW_NUMBER() OVER (
        PARTITION BY file_path ORDER BY updated_at DESC, id DESC
      ) AS rn
      FROM scan_events
      WHERE process_status IN ('pending', 'retry')
    ) ranked
    WHERE rn = 1
  );

-- Partial unique index: enforces "at most one non-terminal row per file_path"
-- at the DB level and gives the upsert path a deterministic conflict target.
-- Terminal rows (complete, failed) remain unconstrained so processing
-- history is preserved across cleanups.
CREATE UNIQUE INDEX idx_scan_events_dedupe_pending_retry
  ON scan_events (file_path)
  WHERE process_status IN ('pending', 'retry');
