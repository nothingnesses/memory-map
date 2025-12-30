CREATE TYPE publicity_default AS ENUM ('public', 'private');
CREATE TYPE publicity_override AS ENUM ('default', 'public', 'private');

ALTER TABLE users ADD COLUMN default_publicity publicity_default NOT NULL DEFAULT 'private';
ALTER TABLE objects ADD COLUMN publicity publicity_override NOT NULL DEFAULT 'default';
