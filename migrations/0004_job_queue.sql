-- 'queued' = found by n8n but not yet scored; hidden from the Jobs tab until it scores above the threshold.
ALTER TABLE jobs DROP CONSTRAINT jobs_status_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_status_check
    CHECK (status IN ('queued', 'new', 'saved', 'applied', 'interview', 'offer', 'rejected', 'skipped'));
