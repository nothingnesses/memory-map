ALTER TYPE publicity_override
ADD VALUE 'selected_users';
CREATE TABLE object_allowed_users (
	object_id BIGINT REFERENCES objects(id) ON DELETE CASCADE,
	user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
	PRIMARY KEY (object_id, user_id)
);