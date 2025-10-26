CREATE TABLE objects (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	name TEXT NOT NULL UNIQUE,
	made_on timestamptz,
	-- '4326' is the Spatial Reference System Identifier (SRID) for WGS 84,
	-- the standard for GPS and global coordinate systems.
	location GEOGRAPHY(Point, 4326)
);