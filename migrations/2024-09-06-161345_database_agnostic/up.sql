DROP TABLE scan_events;
CREATE TABLE IF NOT EXISTS scan_events (
    id TEXT PRIMARY KEY NOT NULL,

    event_source TEXT NOT NULL,
    event_timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,

    file_path TEXT NOT NULL,
    file_hash TEXT,

    process_status TEXT NOT NULL DEFAULT 'pending',
    found_status TEXT NOT NULL DEFAULT 'not_found',

    failed_times INTEGER DEFAULT 0 NOT NULL,
    next_retry_at TIMESTAMP,

    targets_hit TEXT DEFAULT '' NOT NULL,

    found_at TIMESTAMP,
    processed_at TIMESTAMP,

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);