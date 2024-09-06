-- Create the table if it does not exist
CREATE TABLE IF NOT EXISTS scan_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,

    event_source TEXT NOT NULL,
    event_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL,

    file_path TEXT NOT NULL,
    file_hash TEXT,

    process_status TEXT CHECK (process_status IN ('complete', 'pending', 'retry', 'failed')) NOT NULL DEFAULT 'pending',
    found_status TEXT CHECK (found_status IN ('not_found', 'found', 'hash_mismatch')) NOT NULL DEFAULT 'not_found',

    failed_times INTEGER DEFAULT 0 NOT NULL,
    next_retry_at DATETIME,

    targets_hit TEXT DEFAULT '' NOT NULL,  -- Emulate array by storing JSON or comma-separated values

    found_at DATETIME,
    processed_at DATETIME,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);