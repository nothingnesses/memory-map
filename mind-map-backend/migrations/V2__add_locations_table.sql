CREATE TABLE locations (
	id SERIAL PRIMARY KEY,
	-- '4326' is the Spatial Reference System Identifier (SRID) for WGS 84,
	-- the standard for GPS and global coordinate systems.
	location GEOGRAPHY(Point, 4326) NOT NULL
);