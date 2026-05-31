ALTER TABLE password_reset_tokens
	ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();

CREATE INDEX password_reset_tokens_user_id_idx ON password_reset_tokens (user_id);
