CREATE VIEW available_objects_with_users AS
SELECT
	o.id,
	o.name,
	o.storage_key,
	o.content_type,
	o.made_on,
	ST_Y(o.location::geometry) AS latitude,
	ST_X(o.location::geometry) AS longitude,
	o.user_id,
	o.publicity,
	COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
FROM objects o
LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
LEFT JOIN users u ON oau.user_id = u.id
WHERE o.storage_state = 'available'
GROUP BY o.id;
