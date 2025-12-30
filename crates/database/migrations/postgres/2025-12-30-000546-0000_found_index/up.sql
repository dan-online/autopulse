CREATE INDEX IF NOT EXISTS idx_scan_events_process_found
ON scan_events (process_status, found_status);