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
