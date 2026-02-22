CREATE INDEX idx_scan_events_process_found
ON scan_events (process_status(255), found_status(255));
