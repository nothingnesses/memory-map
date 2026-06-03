pub const MARK_OBJECTS_DELETE_PENDING_QUERY: &str = "UPDATE objects
SET storage_state = 'delete_pending', storage_state_updated_at = now()
WHERE id = ANY($1) AND storage_state = 'available'
RETURNING id, name, storage_key, content_type, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const INSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, storage_key, content_type, storage_state, made_on, location, user_id, publicity)
VALUES ($1, $2, $3, 'pending_upload', $4::timestamptz, ST_GeomFromEWKT($5), $6, $7)
RETURNING id, name, storage_key, content_type, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const INSERT_OBJECT_UPLOAD_SESSION_QUERY: &str = "INSERT INTO object_upload_sessions (
	object_id,
	storage_key,
	upload_id,
	content_type,
	file_size,
	part_size_bytes,
	expires_at,
	cleanup_next_attempt_at
)
VALUES ($1, $2, $3, $4, $5, $6, now() + ($7::BIGINT * interval '1 second'), now() + ($7::BIGINT * interval '1 second'))
RETURNING
	object_id,
	storage_key,
	upload_id,
	content_type,
	file_size,
	part_size_bytes,
	expires_at,
	cleanup_attempts,
	cleanup_last_attempt_at,
	cleanup_next_attempt_at,
	cleanup_last_error,
	created_at";

pub const SELECT_ACTIVE_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY: &str = "SELECT
	session.object_id,
	session.storage_key,
	session.upload_id,
	session.content_type,
	session.file_size,
	session.part_size_bytes,
	session.expires_at,
	session.cleanup_attempts,
	session.cleanup_last_attempt_at,
	session.cleanup_next_attempt_at,
	session.cleanup_last_error,
	session.created_at
FROM object_upload_sessions session
JOIN objects object ON object.id = session.object_id
WHERE session.object_id = $1
	AND object.user_id = $2
	AND object.storage_state = 'pending_upload'
	AND session.expires_at > now()";

pub const SELECT_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY: &str = "SELECT
	session.object_id,
	session.storage_key,
	session.upload_id,
	session.content_type,
	session.file_size,
	session.part_size_bytes,
	session.expires_at,
	session.cleanup_attempts,
	session.cleanup_last_attempt_at,
	session.cleanup_next_attempt_at,
	session.cleanup_last_error,
	session.created_at
FROM object_upload_sessions session
JOIN objects object ON object.id = session.object_id
WHERE session.object_id = $1
	AND object.user_id = $2
	AND object.storage_state = 'pending_upload'";

/// Claims up to `$1` expired upload sessions whose retry/lease time has arrived
/// and which still have retry budget left. Rows past `$3::INTEGER` attempts are
/// parked with their last error for operator triage.
pub const CLAIM_EXPIRED_OBJECT_UPLOAD_SESSIONS_QUERY: &str = "WITH claimed AS MATERIALIZED (
	SELECT session.object_id
	FROM object_upload_sessions session
	JOIN objects object ON object.id = session.object_id
	WHERE session.expires_at <= now()
		AND session.cleanup_next_attempt_at <= now()
		AND session.cleanup_attempts < $3::INTEGER
		AND object.storage_state = 'pending_upload'
	ORDER BY session.expires_at, session.cleanup_next_attempt_at, session.created_at
	LIMIT $1
	FOR UPDATE OF session SKIP LOCKED
)
UPDATE object_upload_sessions session
SET cleanup_attempts = cleanup_attempts + 1,
	cleanup_last_attempt_at = now(),
	cleanup_next_attempt_at = now() + ($2::BIGINT * interval '1 second'),
	cleanup_last_error = NULL
FROM claimed
WHERE session.object_id = claimed.object_id
RETURNING
	session.object_id,
	session.storage_key,
	session.upload_id,
	session.content_type,
	session.file_size,
	session.part_size_bytes,
	session.expires_at,
	session.cleanup_attempts,
	session.cleanup_last_attempt_at,
	session.cleanup_next_attempt_at,
	session.cleanup_last_error,
	session.created_at";

pub const DELETE_OBJECT_UPLOAD_SESSION_QUERY: &str =
	"DELETE FROM object_upload_sessions WHERE object_id = $1";

pub const FINALIZE_OBJECT_UPLOAD_QUERY: &str = "WITH finalized AS (
	UPDATE objects
	SET storage_state = 'available', storage_state_updated_at = now()
	WHERE id = $1 AND storage_key = $2 AND storage_state = 'pending_upload'
	RETURNING id, name, storage_key, content_type, made_on, location, user_id, publicity
)
SELECT
	finalized.id,
	finalized.name,
	finalized.storage_key,
	finalized.content_type,
	finalized.made_on,
	ST_Y(finalized.location::geometry) AS latitude,
	ST_X(finalized.location::geometry) AS longitude,
	finalized.user_id,
	finalized.publicity,
	COALESCE((
		SELECT array_agg(users.email)
		FROM object_allowed_users allowed
		JOIN users ON allowed.user_id = users.id
		WHERE allowed.object_id = finalized.id
	), '{}') AS allowed_users
FROM finalized;";

pub const SELECT_AVAILABLE_OBJECT_FOR_USER_QUERY: &str =
	"SELECT * FROM available_objects_with_users WHERE id = $1 AND user_id = $2;";

pub const DELETE_PENDING_OBJECT_UPLOAD_QUERY: &str = "DELETE FROM objects
WHERE id = $1
	AND storage_key = $2
	AND user_id = $3
	AND storage_state = 'pending_upload'";

pub const DELETE_PENDING_OBJECT_UPLOAD_BY_SESSION_QUERY: &str = "DELETE FROM objects
WHERE id = $1
	AND storage_key = $2
	AND storage_state = 'pending_upload'";

pub const MARK_OBJECT_UPLOAD_SESSION_CLEANUP_FAILED_QUERY: &str = "UPDATE object_upload_sessions
SET cleanup_next_attempt_at = now() + ($3::BIGINT * interval '1 second'),
	cleanup_last_error = $2
WHERE object_id = $1";

pub const MARK_UPLOAD_DELETE_PENDING_QUERY: &str = "UPDATE objects
SET storage_state = 'delete_pending', storage_state_updated_at = now()
WHERE id = $1 AND storage_key = $2 AND storage_state IN ('pending_upload', 'available')
RETURNING id, name, storage_key, content_type, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const MARK_STALE_UPLOADS_DELETE_PENDING_QUERY: &str = "UPDATE objects
SET storage_state = 'delete_pending', storage_state_updated_at = now()
WHERE storage_state = 'pending_upload'
	AND storage_state_updated_at < now() - ($1::BIGINT * interval '1 second')
	AND NOT EXISTS (
		SELECT 1 FROM object_upload_sessions session
		WHERE session.object_id = objects.id
	)
RETURNING id, name, storage_key, content_type, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

/// Query to update an existing object in the database.
/// It updates the name, made_on timestamp, and location based on the provided ID.
pub const UPDATE_OBJECT_QUERY: &str = "UPDATE objects
SET name = $2, made_on = $3::timestamptz, location = ST_GeomFromEWKT($4), publicity = $5
WHERE id = $1 AND storage_state = 'available'
RETURNING id, name, storage_key, content_type, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const SELECT_ALL_OBJECTS_QUERY: &str = "SELECT * FROM available_objects_with_users;";

pub const SELECT_OBJECT_BY_ID_QUERY: &str =
	"SELECT * FROM available_objects_with_users WHERE id = $1;";

pub const SELECT_OBJECT_BY_NAME_QUERY: &str =
	"SELECT * FROM available_objects_with_users WHERE name = $1;";

pub const SELECT_OBJECTS_BY_IDS_QUERY: &str =
	"SELECT * FROM available_objects_with_users WHERE id = ANY($1);";

pub const SELECT_OBJECTS_BY_USER_ID_QUERY: &str =
	"SELECT * FROM available_objects_with_users WHERE user_id = $1;";

pub const SELECT_VISIBLE_OBJECTS_QUERY: &str = "SELECT v.*
FROM available_objects_with_users v
JOIN users owner ON owner.id = v.user_id
WHERE
	($1::BIGINT IS NOT NULL AND v.user_id = $1)
	OR v.publicity = 'public'
	OR (v.publicity = 'default' AND owner.default_publicity = 'public')
	OR (
		v.publicity = 'selected_users'
		AND $1::BIGINT IS NOT NULL
		AND EXISTS (
			SELECT 1
			FROM object_allowed_users allowed
			WHERE allowed.object_id = v.id
				AND allowed.user_id = $1
		)
	);";

pub const DELETE_OBJECT_ALLOWED_USERS_QUERY: &str =
	"DELETE FROM object_allowed_users WHERE object_id = $1";

pub const REPLACE_OBJECT_ALLOWED_USERS_QUERY: &str = "WITH valid AS (
	SELECT id, email
	FROM users
	WHERE email = ANY($2)
),
inserted AS (
	INSERT INTO object_allowed_users (object_id, user_id)
	SELECT $1, id
	FROM valid
	RETURNING user_id
)
SELECT valid.email
FROM valid
JOIN inserted ON inserted.user_id = valid.id";

pub const SELECT_ALL_USERS_QUERY: &str =
	"SELECT id, email, role, created_at, updated_at, default_publicity FROM users";

pub const SELECT_USER_BY_ID_QUERY: &str =
	"SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE id = $1";

pub const SELECT_USER_BY_EMAIL_QUERY: &str =
	"SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE email = $1";

pub const SELECT_USER_ID_BY_EMAIL_FOR_UPDATE_QUERY: &str =
	"SELECT id FROM users WHERE email = $1 FOR UPDATE";

pub const INSERT_OBJECT_STORAGE_DELETIONS_QUERY: &str =
	"INSERT INTO object_storage_deletions (storage_key, object_id)
SELECT UNNEST($1::TEXT[]), UNNEST($2::BIGINT[])
ON CONFLICT (storage_key) DO NOTHING";

/// Claims up to `$1` deletion rows whose scheduled retry/lease time has arrived
/// and which still have retry budget left. Rows past `$3::INTEGER` attempts are parked: they remain
/// in the table with `last_error` populated for operator triage, but are never
/// reclaimed by the worker.
pub const CLAIM_OBJECT_STORAGE_DELETIONS_QUERY: &str = "WITH claimed AS MATERIALIZED (
	SELECT storage_key
	FROM object_storage_deletions
	WHERE attempts < $3::INTEGER
		AND next_attempt_at <= now()
	ORDER BY next_attempt_at, created_at
	LIMIT $1
	FOR UPDATE SKIP LOCKED
)
UPDATE object_storage_deletions deletion
SET attempts = attempts + 1,
	last_attempt_at = now(),
	next_attempt_at = now() + ($2::BIGINT * interval '1 second'),
	last_error = NULL
FROM claimed
WHERE deletion.storage_key = claimed.storage_key
RETURNING deletion.storage_key";

pub const DELETE_OBJECT_STORAGE_DELETIONS_QUERY: &str =
	"DELETE FROM object_storage_deletions WHERE storage_key = ANY($1)";

pub const DELETE_OBJECTS_BY_STORAGE_KEYS_QUERY: &str =
	"DELETE FROM objects WHERE storage_key = ANY($1) AND storage_state = 'delete_pending'";

pub const MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY: &str = "UPDATE object_storage_deletions
SET next_attempt_at = now() + ($3::BIGINT * interval '1 second'),
	last_error = $2
WHERE storage_key = ANY($1)";

pub const SELECT_USER_COUNT_BY_EMAIL_QUERY: &str = "SELECT COUNT(*) FROM users WHERE email = $1";

pub const SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY: &str =
	"SELECT COUNT(*) FROM users WHERE email = $1 AND id != $2";

pub const SELECT_USER_PASSWORD_HASH_BY_ID_QUERY: &str =
	"SELECT password_hash FROM users WHERE id = $1";

pub const SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY: &str = "SELECT id, email, password_hash, role, created_at, updated_at, default_publicity FROM users WHERE email = $1";

pub const INSERT_USER_QUERY: &str = "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const UPDATE_USER_PUBLICITY_QUERY: &str = "UPDATE users SET default_publicity = $1, updated_at = now() WHERE id = $2 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const UPDATE_USER_PASSWORD_QUERY: &str =
	"UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2";

pub const UPDATE_USER_EMAIL_QUERY: &str = "UPDATE users SET email = $1, updated_at = now() WHERE id = $2 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const ADMIN_UPDATE_USER_QUERY: &str = "UPDATE users SET role = $1, email = $2, updated_at = now() WHERE id = $3 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const INSERT_PASSWORD_RESET_TOKEN_QUERY: &str = "INSERT INTO password_reset_tokens (token, user_id, expires_at) VALUES ($1, $2, now() + interval '10 minutes')";

pub const SELECT_PASSWORD_RESET_TOKEN_QUERY: &str =
	"SELECT user_id FROM password_reset_tokens WHERE token = $1 AND expires_at > now()";

/// Returns whether the user has any unconsumed token issued within the rate-limit window.
///
/// Used by `request_password_reset` to throttle issuance per user; the window is bound
/// by the `$2::BIGINT` seconds parameter so callers control the policy.
pub const RECENT_PASSWORD_RESET_TOKEN_EXISTS_QUERY: &str = "SELECT EXISTS (
		SELECT 1 FROM password_reset_tokens
		WHERE user_id = $1
			AND created_at > now() - ($2::BIGINT * interval '1 second')
	)";

/// Invalidates all unconsumed reset tokens for a user.
///
/// Used at issuance time (replace siblings with the new token) and at consumption time
/// (after a successful reset, kill any other outstanding tokens so the user has none).
pub const DELETE_PASSWORD_RESET_TOKENS_BY_USER_QUERY: &str =
	"DELETE FROM password_reset_tokens WHERE user_id = $1";

pub const INSERT_EMAIL_OUTBOX_QUERY: &str =
	"INSERT INTO email_outbox (kind, payload) VALUES ($1, $2::TEXT::jsonb)";

/// Claims up to `$1` email rows whose scheduled retry/lease time has arrived
/// and which still have retry budget left. Rows past `$3::INTEGER` attempts are
/// parked with their last error for operator triage.
pub const CLAIM_EMAIL_OUTBOX_QUERY: &str = "WITH claimed AS MATERIALIZED (
	SELECT id
	FROM email_outbox
	WHERE attempts < $3::INTEGER
		AND next_attempt_at <= now()
	ORDER BY next_attempt_at, created_at
	LIMIT $1
	FOR UPDATE SKIP LOCKED
)
UPDATE email_outbox outbox
SET attempts = attempts + 1,
	last_attempt_at = now(),
	next_attempt_at = now() + ($2::BIGINT * interval '1 second'),
	last_error = NULL
FROM claimed
WHERE outbox.id = claimed.id
RETURNING outbox.id, outbox.kind, outbox.payload::TEXT AS payload";

pub const DELETE_EMAIL_OUTBOX_QUERY: &str = "DELETE FROM email_outbox WHERE id = ANY($1)";

pub const MARK_EMAIL_OUTBOX_FAILED_QUERY: &str = "UPDATE email_outbox
SET next_attempt_at = now() + ($3::BIGINT * interval '1 second'),
	last_error = $2
WHERE id = ANY($1)";
