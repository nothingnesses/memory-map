CREATE TABLE object_upload_sessions (
	object_id BIGINT PRIMARY KEY REFERENCES objects(id) ON DELETE CASCADE,
	storage_key TEXT NOT NULL UNIQUE CHECK (storage_key <> ''),
	upload_id TEXT NOT NULL CHECK (upload_id <> ''),
	content_type TEXT NOT NULL CHECK (content_type <> ''),
	file_size BIGINT NOT NULL CHECK (file_size > 0),
	part_size_bytes BIGINT NOT NULL CHECK (part_size_bytes >= 5 * 1024 * 1024),
	expires_at timestamptz NOT NULL,
	cleanup_attempts INTEGER NOT NULL DEFAULT 0 CHECK (cleanup_attempts >= 0),
	cleanup_last_attempt_at timestamptz,
	cleanup_next_attempt_at timestamptz NOT NULL DEFAULT now(),
	cleanup_last_error TEXT,
	created_at timestamptz NOT NULL DEFAULT now(),
	CHECK (expires_at > created_at)
);

CREATE INDEX object_upload_sessions_created_at_idx
	ON object_upload_sessions (created_at);

CREATE INDEX object_upload_sessions_cleanup_idx
	ON object_upload_sessions (expires_at, cleanup_next_attempt_at, created_at);
