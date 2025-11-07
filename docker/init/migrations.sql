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

\echo "Create index for flightroute table"
CREATE INDEX IF NOT EXISTS index_flightroute_airport_destination_id ON flightroute (airport_destination_id);
CREATE INDEX IF NOT EXISTS index_flightroute_airport_midpoint_id ON flightroute (airport_midpoint_id);
CREATE INDEX IF NOT EXISTS index_flightroute_airport_origin_id ON flightroute (airport_origin_id);
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_id ON flightroute (flightroute_callsign_id);

\echo "Create index for flightroute_callsign table"
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_airline_id ON flightroute_callsign (airline_id);
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_airline_id_iata_prefix_id ON flightroute_callsign (airline_id, iata_prefix_id);
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_airline_id_icao_prefix_id ON flightroute_callsign (airline_id, icao_prefix_id);
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_callsign_id ON flightroute_callsign (callsign_id);
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_icao_prefix_id ON flightroute_callsign (icao_prefix_id);

\echo "Create indexes for aircraft table"
CREATE INDEX IF NOT EXISTS index_aircraft_country_id ON aircraft (country_id);
CREATE INDEX IF NOT EXISTS index_aircraft_icao_type_id ON aircraft (aircraft_icao_type_id);
CREATE INDEX IF NOT EXISTS index_aircraft_manufacturer_id ON aircraft (aircraft_manufacturer_id);
CREATE INDEX IF NOT EXISTS index_aircraft_mode_s_id ON aircraft (aircraft_mode_s_id);
CREATE INDEX IF NOT EXISTS index_aircraft_operator_flag_id ON aircraft (aircraft_operator_flag_code_id);
CREATE INDEX IF NOT EXISTS index_aircraft_photo_id ON aircraft (aircraft_photo_id);
CREATE INDEX IF NOT EXISTS index_aircraft_registered_owner_id ON aircraft (aircraft_registered_owner_id);
CREATE INDEX IF NOT EXISTS index_aircraft_registration_id ON aircraft (aircraft_registration_id);
CREATE INDEX IF NOT EXISTS index_aircraft_type_id ON aircraft (aircraft_type_id);

\echo "Create indexes for aircraft_registration table"
CREATE INDEX IF NOT EXISTS index_aircraft_registration_registration ON aircraft_registration (registration);

\echo "Create indexes for airline table"
CREATE INDEX IF NOT EXISTS index_airline_country_id ON airline (country_id);
CREATE INDEX IF NOT EXISTS index_airline_iata_prefix ON airline (iata_prefix);
CREATE INDEX IF NOT EXISTS index_airline_icao_prefix ON airline (icao_prefix);

\echo "Create indexes for flightroute_callsign_inner table"
CREATE INDEX IF NOT EXISTS index_flightroute_callsign_inner_callsign ON flightroute_callsign_inner (callsign);

\echo "Create indexes for airport table"
CREATE INDEX IF NOT EXISTS index_airport_country_id ON airport (country_id);
CREATE INDEX IF NOT EXISTS index_airport_elevation_id ON airport (airport_elevation_id);
CREATE INDEX IF NOT EXISTS index_airport_iata_code_id ON airport (airport_iata_code_id);
CREATE INDEX IF NOT EXISTS index_airport_icao_code_id ON airport (airport_icao_code_id);
CREATE INDEX IF NOT EXISTS index_airport_latitude_id ON airport (airport_latitude_id);
CREATE INDEX IF NOT EXISTS index_airport_longitude_id ON airport (airport_longitude_id);
CREATE INDEX IF NOT EXISTS index_airport_municipality_id ON airport (airport_municipality_id);
CREATE INDEX IF NOT EXISTS index_airport_name_id ON airport (airport_name_id);


\echo "update work mem"
ALTER DATABASE adsbdb SET work_mem = '32MB';

\echo "Add extension pg_trgm"
CREATE EXTENSION IF NOT EXISTS pg_trgm;

\echo "Add method enum, use a DO as can't use IF NOT EXISTS for this"
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 
        FROM pg_type 
        WHERE typname = 'request_method'
    ) THEN
        CREATE TYPE request_method AS ENUM (
            'CONNECT',
            'DELETE',
            'GET',
            'HEAD',
            'OPTIONS',
            'PATCH',
            'POST',
            'PUT',
            'TRACE'
        );
    END IF;
END
$$;

\echo "Create incoming request url table"
CREATE TABLE incoming_request_url (
    incoming_request_url_id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    request_url TEXT NOT NULL UNIQUE
);

GRANT ALL ON incoming_request_url TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE incoming_request_url_incoming_request_url_id_seq TO adsbdb;

\echo "Create incoming request url indexes"
CREATE INDEX IF NOT EXISTS index_incoming_request_url_trgm ON incoming_request_url USING gin (request_url gin_trgm_ops);
CREATE INDEX IF NOT EXISTS index_incoming_request_url_request_url ON incoming_request_url (request_url);

\echo "Create temp incoming request table"
CREATE TABLE temp_incoming_request (
    temp_incoming_request_id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    incoming_request_url_id BIGINT REFERENCES incoming_request_url(incoming_request_url_id) NOT NULL,
    request_method request_method NOT NULL,
    count INTEGER NOT NULL DEFAULT 1
);

GRANT ALL ON temp_incoming_request TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE temp_incoming_request_temp_incoming_request_id_seq TO adsbdb;

\echo "Create temp incoming request indexes"
CREATE UNIQUE INDEX IF NOT EXISTS index_temp_incoming_request_unique_method_minute_url ON temp_incoming_request (request_method, incoming_request_url_id);
CREATE INDEX IF NOT EXISTS index_temp_incoming_request_url ON temp_incoming_request (incoming_request_url_id);
CREATE INDEX IF NOT EXISTS index_temp_incoming_request_timestamp ON temp_incoming_request (timestamp);
CREATE INDEX IF NOT EXISTS index_temp_incoming_request_comp ON temp_incoming_request (incoming_request_url_id, count);


\echo "Create incoming request table"
CREATE TABLE incoming_request (
    incoming_request_id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    -- minute_ts TIMESTAMPTZ NOT NULL,
    incoming_request_url_id BIGINT REFERENCES incoming_request_url(incoming_request_url_id) NOT NULL,
    request_method request_method NOT NULL,
    count INTEGER NOT NULL DEFAULT 1
);


GRANT ALL ON incoming_request TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE incoming_request_incoming_request_id_seq TO adsbdb;

\echo "Create incoming request indexes"
CREATE UNIQUE INDEX IF NOT EXISTS index_incoming_request_unique_method_minute_url ON incoming_request ( request_method, incoming_request_url_id);
CREATE INDEX IF NOT EXISTS index_incoming_request_url ON incoming_request (incoming_request_url_id);
CREATE INDEX IF NOT EXISTS index_incoming_request_comp ON incoming_request (incoming_request_url_id, count);

\echo "Rename index_incoming_request_unique_method_minute_url"
ALTER INDEX "index_incoming_request_unique_method_minute_url" RENAME TO "index_incoming_request_unique_method_url";

\echo "Refactor incoming_request_url table"

\echo "Create incoming_request_url_version table"
CREATE TABLE incoming_request_url_version (
    incoming_request_url_version_id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    url_version TEXT NOT NULL UNIQUE
);

GRANT ALL ON incoming_request_url_version TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE incoming_request_url_version_incoming_request_url_version_i_seq TO adsbdb;

CREATE INDEX IF NOT EXISTS index_incoming_request_url_verion_trgm ON incoming_request_url_version USING gin (url_version gin_trgm_ops);
CREATE INDEX IF NOT EXISTS index_incoming_request_url_version ON incoming_request_url_version (url_version);

\echo "Create incoming_request_url_path table"
CREATE TABLE incoming_request_url_path (
    incoming_request_url_path_id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    url_path TEXT NOT NULL UNIQUE
);

GRANT ALL ON incoming_request_url_path TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE incoming_request_url_path_incoming_request_url_path_id_seq TO adsbdb;

CREATE INDEX IF NOT EXISTS index_incoming_request_url_path_trgm ON incoming_request_url_path USING gin (url_path gin_trgm_ops);
CREATE INDEX IF NOT EXISTS index_incoming_request_url_path ON incoming_request_url_path (url_path);

\echo "Create incoming_request_url_query table"
CREATE TABLE incoming_request_url_query (
    incoming_request_url_query_id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    url_query TEXT NOT NULL UNIQUE
);

GRANT ALL ON incoming_request_url_query TO adsbdb;
GRANT USAGE, SELECT ON SEQUENCE incoming_request_url_query_incoming_request_url_query_id_seq TO adsbdb;

CREATE INDEX IF NOT EXISTS index_incoming_request_url_query_trgm ON incoming_request_url_query USING gin (url_query gin_trgm_ops);
CREATE INDEX IF NOT EXISTS index_incoming_request_url_query ON incoming_request_url_query (url_query);

\echo "Alter incoming_request_url table add new ids"
ALTER TABLE incoming_request_url ADD COLUMN incoming_request_url_version_id BIGINT REFERENCES incoming_request_url_version(incoming_request_url_version_id);
ALTER TABLE incoming_request_url ADD COLUMN incoming_request_url_path_id BIGINT REFERENCES incoming_request_url_path(incoming_request_url_path_id);
ALTER TABLE incoming_request_url ADD COLUMN incoming_request_url_query_id BIGINT REFERENCES incoming_request_url_query(incoming_request_url_query_id);

\echo "Insert url_version"
WITH extracted_segments AS (
    SELECT 
        TRIM(split_part(ltrim(request_url, '/'), '/', 1)) AS url_segment
    FROM incoming_request_url
    WHERE request_url IS NOT NULL
)
INSERT INTO incoming_request_url_version (url_version)
SELECT DISTINCT url_segment
FROM extracted_segments
WHERE url_segment != '' 
ON CONFLICT DO NOTHING;

\echo "Update url_version"
UPDATE incoming_request_url AS iru
SET incoming_request_url_version_id = iruv.incoming_request_url_version_id
FROM incoming_request_url_version AS iruv
WHERE iruv.url_version = split_part(ltrim(iru.request_url, '/'), '/', 1);

\echo "Insert url_path"
WITH extracted_segments AS (
    SELECT 
        TRIM(split_part(ltrim(request_url, '/'), '/', 2)) AS url_segment
    FROM incoming_request_url
    WHERE request_url IS NOT NULL
)
INSERT INTO incoming_request_url_path (url_path)
SELECT DISTINCT url_segment
FROM extracted_segments
WHERE url_segment != ''
ON CONFLICT DO NOTHING;

\echo "Update url_path"
UPDATE incoming_request_url AS iru
SET incoming_request_url_path_id = irup.incoming_request_url_path_id
FROM incoming_request_url_path AS irup
WHERE irup.url_path = split_part(ltrim(iru.request_url, '/'), '/', 2);

\echo "Insert url_query"
WITH extracted_segments AS (
    SELECT 
        TRIM(split_part(ltrim(request_url, '/'), '/', 3)) AS url_segment
    FROM incoming_request_url
    WHERE request_url IS NOT NULL
)
INSERT INTO incoming_request_url_query (url_query)
SELECT DISTINCT url_segment
FROM extracted_segments
WHERE url_segment != ''
ON CONFLICT DO NOTHING;

\echo "Update url_query"
UPDATE incoming_request_url AS iru
SET incoming_request_url_query_id = iruq.incoming_request_url_query_id
FROM incoming_request_url_query AS iruq
WHERE iruq.url_query = split_part(ltrim(iru.request_url, '/'), '/', 3);


\echo "De-duplicate incoming_request_urls"
BEGIN;

CREATE TEMP TABLE tmp_incoming_request_url_map AS
WITH duplicates AS (
  SELECT 
      incoming_request_url_version_id,
      incoming_request_url_path_id,
      incoming_request_url_query_id,
      MIN(incoming_request_url_id) AS keep_id,
      ARRAY_AGG(incoming_request_url_id) AS all_ids
  FROM incoming_request_url
  GROUP BY incoming_request_url_version_id,
           incoming_request_url_path_id,
           incoming_request_url_query_id
  HAVING COUNT(*) > 1
)
SELECT 
  d.keep_id, 
  dup_id AS duplicate_id
FROM duplicates d
CROSS JOIN LATERAL unnest(d.all_ids) AS dup_id
WHERE dup_id <> d.keep_id;

INSERT INTO incoming_request (timestamp, request_method, incoming_request_url_id, count)
SELECT 
  NOW(),
  r.request_method,
  m.keep_id,
  SUM(r.count)
FROM incoming_request r
JOIN tmp_incoming_request_url_map m 
  ON r.incoming_request_url_id = m.duplicate_id
GROUP BY r.request_method, m.keep_id
ON CONFLICT (request_method, incoming_request_url_id) DO UPDATE
  SET count = incoming_request.count + EXCLUDED.count;

INSERT INTO temp_incoming_request (timestamp, request_method, incoming_request_url_id, count)
SELECT 
  NOW(),
  r.request_method,
  m.keep_id,
  SUM(r.count)
FROM temp_incoming_request r
JOIN tmp_incoming_request_url_map m 
  ON r.incoming_request_url_id = m.duplicate_id
GROUP BY r.request_method, m.keep_id
ON CONFLICT (request_method, incoming_request_url_id) DO UPDATE
  SET count = temp_incoming_request.count + EXCLUDED.count;

DELETE FROM incoming_request r
USING tmp_incoming_request_url_map m
WHERE r.incoming_request_url_id = m.duplicate_id;

DELETE FROM temp_incoming_request r
USING tmp_incoming_request_url_map m
WHERE r.incoming_request_url_id = m.duplicate_id;

DELETE FROM incoming_request_url u
USING tmp_incoming_request_url_map m
WHERE u.incoming_request_url_id = m.duplicate_id;

ALTER TABLE incoming_request_url
ADD CONSTRAINT uunique_incoming_request_url_parts
UNIQUE (incoming_request_url_version_id, incoming_request_url_path_id, incoming_request_url_query_id);

COMMIT;

ALTER TABLE incoming_request_url DROP COLUMN request_url;

-- v0.5.0