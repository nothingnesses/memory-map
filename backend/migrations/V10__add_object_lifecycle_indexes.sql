ALTER TABLE object_storage_deletions
	ADD COLUMN next_attempt_at timestamptz;

UPDATE object_storage_deletions
SET next_attempt_at = CASE
	WHEN processing_expires_at IS NOT NULL THEN processing_expires_at
	WHEN last_attempt_at IS NULL THEN created_at
	ELSE now()
END;

ALTER TABLE object_storage_deletions
	ALTER COLUMN next_attempt_at SET DEFAULT now(),
	ALTER COLUMN next_attempt_at SET NOT NULL;

CREATE INDEX object_storage_deletions_claim_idx
	ON object_storage_deletions (next_attempt_at, created_at);

CREATE INDEX objects_pending_upload_age_idx
	ON objects (storage_state_updated_at)
	WHERE storage_state = 'pending_upload';

CREATE INDEX objects_user_state_idx
	ON objects (user_id, storage_state);
