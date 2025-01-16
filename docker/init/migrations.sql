ALTER TABLE
	airport_elevation
ALTER COLUMN
	elevation TYPE INT;

ALTER TABLE
	airport_longitude
ALTER COLUMN
	longitude TYPE DOUBLE PRECISION USING longitude :: double precision;

ALTER TABLE
	airport_latitude
ALTER COLUMN
	latitude TYPE DOUBLE PRECISION USING latitude :: double precision;


-- Fix typo in Aircraft table where prefix is ' I', should just be 'I'
UPDATE aircraft
SET aircraft_registration_country_prefix_id = (
    SELECT aircraft_registration_country_prefix_id
    FROM aircraft_registration_country_prefix
    WHERE registration_country_prefix = 'I'
)
WHERE aircraft_registration_country_prefix_id = (
    SELECT aircraft_registration_country_prefix_id
    FROM aircraft_registration_country_prefix
    WHERE registration_country_prefix = ' I'
);

DELETE FROM aircraft_registration_country_prefix WHERE registration_country_prefix = ' I'; 

-- Fix type in Aircraft table where prefix is 'F-0', should just be 'F'
UPDATE aircraft
SET aircraft_registration_country_prefix_id = (
    SELECT aircraft_registration_country_prefix_id
    FROM aircraft_registration_country_prefix
    WHERE registration_country_prefix = 'F'
)
WHERE aircraft_registration_country_prefix_id = (
    SELECT aircraft_registration_country_prefix_id
    FROM aircraft_registration_country_prefix
    WHERE registration_country_prefix = 'F-O'
);

DELETE FROM aircraft_registration_country_prefix WHERE registration_country_prefix = 'F-O'; 