CREATE TYPE object_storage_state AS ENUM ('pending_upload', 'available', 'delete_pending');

ALTER TABLE objects ADD COLUMN storage_key TEXT;
UPDATE objects SET storage_key = name;
ALTER TABLE objects ALTER COLUMN storage_key SET NOT NULL;
ALTER TABLE objects ADD CONSTRAINT objects_storage_key_key UNIQUE (storage_key);

ALTER TABLE objects ADD COLUMN content_type TEXT;
UPDATE objects
SET content_type = CASE
	WHEN lower(name) LIKE '%.png' THEN 'image/png'
	WHEN lower(name) LIKE '%.jpg' OR lower(name) LIKE '%.jpeg' THEN 'image/jpeg'
	WHEN lower(name) LIKE '%.gif' THEN 'image/gif'
	WHEN lower(name) LIKE '%.webp' THEN 'image/webp'
	WHEN lower(name) LIKE '%.svg' THEN 'image/svg+xml'
	WHEN lower(name) LIKE '%.avif' THEN 'image/avif'
	WHEN lower(name) LIKE '%.apng' THEN 'image/apng'
	WHEN lower(name) LIKE '%.mp4' THEN 'video/mp4'
	WHEN lower(name) LIKE '%.webm' THEN 'video/webm'
	WHEN lower(name) LIKE '%.ogv' OR lower(name) LIKE '%.ogg' THEN 'video/ogg'
	WHEN lower(name) LIKE '%.mp3' THEN 'audio/mpeg'
	WHEN lower(name) LIKE '%.wav' THEN 'audio/wav'
	WHEN lower(name) LIKE '%.oga' THEN 'audio/ogg'
	WHEN lower(name) LIKE '%.flac' THEN 'audio/flac'
	WHEN lower(name) LIKE '%.aac' THEN 'audio/aac'
	WHEN lower(name) LIKE '%.m4a' THEN 'audio/mp4'
	ELSE 'application/octet-stream'
END;
ALTER TABLE objects ALTER COLUMN content_type SET NOT NULL;

ALTER TABLE objects
	ADD COLUMN storage_state object_storage_state NOT NULL DEFAULT 'available',
	ADD COLUMN storage_state_updated_at timestamptz NOT NULL DEFAULT now();

ALTER TABLE objects DROP CONSTRAINT objects_name_key;
CREATE UNIQUE INDEX objects_active_name_key
	ON objects (name)
	WHERE storage_state <> 'delete_pending';

CREATE TABLE object_storage_deletions (
	storage_key TEXT PRIMARY KEY,
	object_id BIGINT,
	created_at timestamptz NOT NULL DEFAULT now(),
	attempts INTEGER NOT NULL DEFAULT 0,
	last_attempt_at timestamptz,
	last_error TEXT
);
