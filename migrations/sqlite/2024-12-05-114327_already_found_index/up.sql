CREATE INDEX idx_scan_events_file_path_process_status
ON scan_events (file_path, process_status);