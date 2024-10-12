ALTER TABLE scan_events ADD COLUMN can_process TIMESTAMP NOT NULL;
UPDATE scan_events SET can_process = created_at;