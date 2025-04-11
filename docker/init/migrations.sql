\echo "Migrations"

\echo "airport_elevation.elevation to INT"
ALTER TABLE
    airport_elevation
ALTER COLUMN
    elevation TYPE INT;

\echo "airport_longitude.longitude to DOUBLE PRECISION"
ALTER TABLE
    airport_longitude
ALTER COLUMN
    longitude TYPE DOUBLE PRECISION USING longitude :: double precision;

\echo "airport_latitude.latitude to DOUBLE PRECISION"
ALTER TABLE
    airport_latitude
ALTER COLUMN
    latitude TYPE DOUBLE PRECISION USING latitude :: double precision;

-- Fix typo in Aircraft table where prefix is ' I', should just be 'I'
\echo "aircraft ' I' prefix to 'I'"
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
\echo "aircraft 'F-0' prefix to 'F'"
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

\echo "aircraft.aircraft_icao_type_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_icao_type_id SET NOT NULL;

\echo "aircraft.aircraft_manufacturer_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_manufacturer_id SET NOT NULL;

\echo "aircraft.aircraft_mode_s_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_mode_s_id SET NOT NULL;

\echo "aircraft.aircraft_registered_owner_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_registered_owner_id SET NOT NULL;

\echo "aircraft.aircraft_registration_country_prefix_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_registration_country_prefix_id SET NOT NULL;

\echo "aircraft.aircraft_registration_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_registration_id SET NOT NULL;

\echo "aircraft.aircraft_type_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN aircraft_type_id SET NOT NULL;

\echo "aircraft.country_id NOT NULL"
ALTER TABLE aircraft ALTER COLUMN country_id SET NOT NULL;

