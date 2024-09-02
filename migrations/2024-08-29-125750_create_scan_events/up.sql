CREATE TYPE ProcessStatus AS ENUM ('complete', 'pending', 'retry', 'failed');
CREATE TYPE FoundStatus AS ENUM ('not_found', 'found', 'hash_mismatch');

-- Your SQL goes here
CREATE TABLE scan_events (
    id SERIAL PRIMARY KEY,

    event_source TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ DEFAULT NOW() NOT NULL,

    file_path TEXT NOT NULL,
    -- optional file hash
    file_hash TEXT,

    -- sending to the media servers
    process_status ProcessStatus NOT NULL DEFAULT 'pending',
    -- found as a file and optionally checked for correct hash
    found_status FoundStatus NOT NULL DEFAULT 'not_found',

    failed_times INT DEFAULT 0 NOT NULL,
    next_retry_at TIMESTAMPTZ,

    found_at TIMESTAMPTZ,
    processed_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);