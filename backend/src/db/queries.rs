pub const DELETE_OBJECTS_QUERY: &str = "DELETE FROM objects WHERE id = ANY($1) RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

/// Query to update an existing object in the database.
/// It updates the name, made_on timestamp, and location based on the provided ID.
pub const UPDATE_OBJECT_QUERY: &str = "UPDATE objects
SET name = $2, made_on = $3::timestamptz, location = ST_GeomFromEWKT($4), publicity = $5
WHERE id = $1
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const UPSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, made_on, location, user_id, publicity)
VALUES ($1, $2::timestamptz, ST_GeomFromEWKT($3), $4, $5)
ON CONFLICT (name) DO UPDATE
SET made_on = EXCLUDED.made_on, location = EXCLUDED.location, publicity = EXCLUDED.publicity
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

pub const SELECT_ALL_OBJECTS_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
GROUP BY o.id;";

pub const SELECT_OBJECT_BY_ID_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
WHERE o.id = $1
GROUP BY o.id;";

pub const SELECT_OBJECT_BY_NAME_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
WHERE o.name = $1
GROUP BY o.id;";

pub const SELECT_OBJECTS_BY_IDS_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
WHERE o.id = ANY($1)
GROUP BY o.id;";

pub const SELECT_OBJECTS_BY_USER_ID_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
WHERE o.user_id = $1
GROUP BY o.id;";

pub const SELECT_VISIBLE_OBJECTS_QUERY: &str = "SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
COALESCE(array_agg(u_allowed.email) FILTER (WHERE u_allowed.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
JOIN users u ON o.user_id = u.id
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u_allowed ON oau.user_id = u_allowed.id
WHERE
	($1::BIGINT IS NOT NULL AND o.user_id = $1)
	OR o.publicity = 'public'
	OR (o.publicity = 'default' AND u.default_publicity = 'public')
	OR (o.publicity = 'selected_users' AND $1::BIGINT IS NOT NULL AND $1 IN (SELECT user_id FROM object_allowed_users WHERE object_id = o.id))
GROUP BY o.id;";

pub const DELETE_OBJECT_ALLOWED_USERS_QUERY: &str = "DELETE FROM object_allowed_users WHERE object_id = $1";

pub const INSERT_OBJECT_ALLOWED_USER_QUERY: &str = "INSERT INTO object_allowed_users (object_id, user_id) VALUES ($1, $2)";

pub const SELECT_ALL_USERS_QUERY: &str = "SELECT id, email, role, created_at, updated_at, default_publicity FROM users";

pub const SELECT_USER_BY_ID_QUERY: &str = "SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE id = $1";

pub const SELECT_USER_BY_EMAIL_QUERY: &str = "SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE email = $1";

pub const SELECT_USER_EXISTS_QUERY: &str = "SELECT 1 FROM users WHERE id = $1";

pub const SELECT_USERS_BY_EMAILS_QUERY: &str = "SELECT id, email FROM users WHERE email = ANY($1)";

pub const SELECT_USER_COUNT_BY_EMAIL_QUERY: &str = "SELECT COUNT(*) FROM users WHERE email = $1";

pub const SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY: &str = "SELECT COUNT(*) FROM users WHERE email = $1 AND id != $2";

pub const SELECT_USER_PASSWORD_HASH_BY_ID_QUERY: &str = "SELECT password_hash FROM users WHERE id = $1";

pub const SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY: &str = "SELECT id, email, password_hash, role, created_at, updated_at, default_publicity FROM users WHERE email = $1";

pub const INSERT_USER_QUERY: &str = "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const UPDATE_USER_PUBLICITY_QUERY: &str = "UPDATE users SET default_publicity = $1, updated_at = now() WHERE id = $2 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const UPDATE_USER_PASSWORD_QUERY: &str = "UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2";

pub const UPDATE_USER_EMAIL_QUERY: &str = "UPDATE users SET email = $1, updated_at = now() WHERE id = $2 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const ADMIN_UPDATE_USER_QUERY: &str = "UPDATE users SET role = $1, email = $2, updated_at = now() WHERE id = $3 RETURNING id, email, role, created_at, updated_at, default_publicity";

pub const INSERT_PASSWORD_RESET_TOKEN_QUERY: &str = "INSERT INTO password_reset_tokens (token, user_id, expires_at) VALUES ($1, $2, now() + interval '10 minutes')";

pub const SELECT_PASSWORD_RESET_TOKEN_QUERY: &str = "SELECT user_id FROM password_reset_tokens WHERE token = $1 AND expires_at > now()";

pub const DELETE_PASSWORD_RESET_TOKEN_QUERY: &str = "DELETE FROM password_reset_tokens WHERE token = $1";
