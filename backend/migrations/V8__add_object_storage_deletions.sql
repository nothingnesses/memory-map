CREATE TABLE object_storage_deletions (
	object_name TEXT PRIMARY KEY,
	created_at timestamptz NOT NULL DEFAULT now(),
	attempts INTEGER NOT NULL DEFAULT 0,
	last_attempt_at timestamptz,
	last_error TEXT
);
