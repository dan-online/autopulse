-- Old installs may already have duplicate queued rows. Merge the values we
-- care about before deleting extras so upgrades don't shorten waits or drop
-- hashes.
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

-- Keep one queued row per path while leaving complete/failed history alone.
CREATE UNIQUE INDEX idx_scan_events_dedupe_pending_retry
  ON scan_events (file_path)
  WHERE process_status IN ('pending', 'retry');
