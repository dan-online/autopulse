CREATE INDEX idx_scan_events_process_status_next_retry_at_can_process
ON scan_events (
    process_status,
    next_retry_at,
    can_process
);

CREATE INDEX idx_scan_events_found_status
ON scan_events (found_status);