CREATE TABLE email_outbox (
	id BIGSERIAL PRIMARY KEY,
	kind TEXT NOT NULL CHECK (kind <> ''),
	payload JSONB NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
	last_attempt_at timestamptz,
	next_attempt_at timestamptz NOT NULL DEFAULT now(),
	processing_expires_at timestamptz,
	last_error TEXT
);

CREATE INDEX email_outbox_claim_idx
	ON email_outbox (next_attempt_at, created_at);
