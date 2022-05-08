ALTER TABLE airport_elevation ALTER COLUMN elevation TYPE INT;
ALTER TABLE airport_longitude ALTER COLUMN longitude TYPE DOUBLE PRECISION USING longitude::double precision;
ALTER TABLE airport_latitude ALTER COLUMN latitude TYPE DOUBLE PRECISION USING latitude::double precision;
