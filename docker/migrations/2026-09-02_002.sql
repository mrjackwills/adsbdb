\echo "Drop index_incoming_request_comp"
DROP INDEX IF EXISTS index_incoming_request_comp;

\echo "Drop index_incoming_request_url"
DROP INDEX IF EXISTS index_incoming_request_url;

\echo "Drop index index_incoming_request_unique_method_url"
DROP INDEX IF EXISTS index_incoming_request_unique_method_url;

\echo "Drop incoming_request_id"
ALTER TABLE incoming_request DROP COLUMN IF EXISTS incoming_request_id, DROP COLUMN IF EXISTS timestamp;

\echo "add contstraint incoming_request_pkey PRIMARY KEY (incoming_request_url_id, request_method)"
DO $$
BEGIN
	IF EXISTS (
		SELECT 1 FROM information_schema.columns
		WHERE table_name = 'incoming_request'
		  AND column_name = 'incoming_request_url_id'
	) AND NOT EXISTS (
		SELECT 1
		FROM pg_constraint
		WHERE conname = 'incoming_request_pkey'
		  AND conrelid = to_regclass('public.incoming_request')
	) THEN
		ALTER TABLE incoming_request ADD CONSTRAINT incoming_request_pkey PRIMARY KEY (incoming_request_url_id, request_method);
	END IF;
END
$$;

\echo "Set incoming_request fillfactor"
ALTER TABLE incoming_request SET (fillfactor = 70);

\echo "Remove timestamps from incoming_request_url & parts tables"
ALTER TABLE incoming_request_url DROP COLUMN IF EXISTS timestamp;
ALTER TABLE incoming_request_url_version DROP COLUMN IF EXISTS timestamp;
ALTER TABLE incoming_request_url_path DROP COLUMN IF EXISTS timestamp;
ALTER TABLE incoming_request_url_query DROP COLUMN IF EXISTS timestamp;

DROP INDEX IF EXISTS index_incoming_request_url_verion_trgm;
DROP INDEX IF EXISTS index_incoming_request_url_path_trgm;
DROP INDEX IF EXISTS index_incoming_request_url_query_trgm;

BEGIN;

\echo "Create request counter"
CREATE TABLE IF NOT EXISTS request_total (
	id SMALLINT PRIMARY KEY DEFAULT 1,
	total BIGINT NOT NULL DEFAULT 0,
	CONSTRAINT request_total_single_row CHECK (id = 1)
);
GRANT ALL ON request_total TO adsbdb;

\echo "Set work mem"
SET LOCAL work_mem = '1GB';
SET LOCAL maintenance_work_mem = '1GB';

\echo "Convert incoming urls"
DO $$
BEGIN
	IF EXISTS (
		SELECT 1 FROM information_schema.columns
		WHERE table_name = 'incoming_request'
		  AND column_name = 'incoming_request_url_id'
	) THEN
		ALTER TABLE incoming_request
			ADD COLUMN IF NOT EXISTS incoming_request_url_version_id BIGINT,
			ADD COLUMN IF NOT EXISTS incoming_request_url_path_id BIGINT,
			ADD COLUMN IF NOT EXISTS incoming_request_url_query_id BIGINT;

		UPDATE incoming_request ir
		SET
			incoming_request_url_version_id = iru.incoming_request_url_version_id,
			incoming_request_url_path_id    = iru.incoming_request_url_path_id,
			incoming_request_url_query_id   = iru.incoming_request_url_query_id
		FROM incoming_request_url iru
		WHERE iru.incoming_request_url_id = ir.incoming_request_url_id;

		DROP TABLE IF EXISTS incoming_request_new;
		CREATE TABLE incoming_request_new AS
		SELECT
			incoming_request_url_version_id,
			incoming_request_url_path_id,
			incoming_request_url_query_id,
			request_method,
			SUM(count)::BIGINT AS count
		FROM incoming_request
		GROUP BY
			incoming_request_url_version_id,
			incoming_request_url_path_id,
			incoming_request_url_query_id,
			request_method;

		ALTER TABLE incoming_request RENAME TO incoming_request_old;
		ALTER TABLE incoming_request_new RENAME TO incoming_request;

		DROP TABLE IF EXISTS incoming_request_old;
	END IF;
END
$$;

GRANT ALL ON incoming_request TO adsbdb;
ALTER TABLE incoming_request SET (fillfactor = 70);

ALTER TABLE incoming_request
	ADD COLUMN IF NOT EXISTS ir_version_nk BIGINT GENERATED ALWAYS AS
		(COALESCE(incoming_request_url_version_id, 0)) STORED,
	ADD COLUMN IF NOT EXISTS ir_path_nk BIGINT GENERATED ALWAYS AS
		(COALESCE(incoming_request_url_path_id, 0)) STORED,
	ADD COLUMN IF NOT EXISTS ir_query_nk BIGINT GENERATED ALWAYS AS
		(COALESCE(incoming_request_url_query_id, 0)) STORED;

DO $$
BEGIN
	IF NOT EXISTS (
		SELECT 1 FROM pg_constraint WHERE conname = 'incoming_request_pkey'
	) THEN
		ALTER TABLE incoming_request
			ADD CONSTRAINT incoming_request_pkey
			PRIMARY KEY (request_method, ir_version_nk, ir_path_nk, ir_query_nk);
	END IF;
END
$$;

\echo "Add index"
CREATE INDEX IF NOT EXISTS index_incoming_request_path_id ON incoming_request (incoming_request_url_path_id);

\echo "Fix counter"

DO $$
BEGIN
	IF NOT EXISTS (SELECT 1 FROM request_total WHERE id = 1 AND total != 0) THEN
		INSERT INTO request_total (id, total)
		VALUES (1, (SELECT COALESCE(SUM(count), 0) FROM incoming_request))
		ON CONFLICT (id) DO UPDATE SET total = EXCLUDED.total;
	END IF;
END $$;

\echo "Drop old table"
DROP TABLE IF EXISTS incoming_request_url;

\echo "aircraft_photo substring fix"
UPDATE aircraft_photo SET url_photo = substring(url_photo, 9) WHERE url_photo LIKE 'https://%';

\echo "Denormalize airport table"

DO $$
BEGIN
	IF NOT EXISTS (
		SELECT 1
		FROM information_schema.columns
		WHERE table_schema = 'public'
			AND table_name = 'airport'
			AND column_name = 'elevation'
	) THEN
		
		ALTER TABLE airport
			ADD COLUMN elevation INT,
			ADD COLUMN iata_code TEXT,
			ADD COLUMN icao_code TEXT,
			ADD COLUMN latitude DOUBLE PRECISION,
			ADD COLUMN longitude DOUBLE PRECISION,
			ADD COLUMN airport_name TEXT,
			ADD COLUMN terminated TIMESTAMPTZ;

		UPDATE airport AS a
		SET
			elevation = (SELECT elevation FROM airport_elevation WHERE airport_elevation_id = a.airport_elevation_id),
			latitude = (SELECT latitude FROM airport_latitude WHERE airport_latitude_id = a.airport_latitude_id),
			longitude = (SELECT longitude FROM airport_longitude WHERE airport_longitude_id = a.airport_longitude_id),
			airport_name = (SELECT name FROM airport_name WHERE airport_name_id = a.airport_name_id),
			iata_code = (SELECT iata_code FROM airport_iata_code WHERE airport_iata_code_id = a.airport_iata_code_id),
			icao_code = (SELECT icao_code FROM airport_icao_code WHERE airport_icao_code_id = a.airport_icao_code_id);

		ALTER TABLE airport
			ALTER COLUMN airport_name SET NOT NULL,
			-- todo
			-- ALTER COLUMN airport_name SET NOT NULL,
			-- This can, and will, fail if the elevation's haven't been updated
			-- ALTER COLUMN elevation SET NOT NULL,
			ALTER COLUMN latitude SET NOT NULL,
			ALTER COLUMN longitude SET NOT NULL,
			ADD CONSTRAINT airport_iata_code_check CHECK (iata_code ~ '^[A-Z0-9]{3}$'),
			ADD CONSTRAINT airport_icao_code_check CHECK (icao_code ~ '^[A-Z0-9]{3,4}$');

		DROP INDEX IF EXISTS index_airport_country_id;
		DROP INDEX IF EXISTS index_airport_elevation_id;
		DROP INDEX IF EXISTS index_airport_iata_code_id;
		DROP INDEX IF EXISTS index_airport_icao_code_id;
		DROP INDEX IF EXISTS index_airport_latitude_id;
		DROP INDEX IF EXISTS index_airport_longitude_id;
		DROP INDEX IF EXISTS index_airport_name_id;

		CREATE INDEX IF NOT EXISTS index_airport_iata_code ON airport (iata_code);
		CREATE INDEX IF NOT EXISTS index_airport_name ON airport (airport_name);

		ALTER TABLE airport
			DROP COLUMN IF EXISTS airport_elevation_id,
			DROP COLUMN IF EXISTS airport_iata_code_id,
			DROP COLUMN IF EXISTS airport_icao_code_id,
			DROP COLUMN IF EXISTS airport_latitude_id,
			DROP COLUMN IF EXISTS airport_longitude_id,
			DROP COLUMN IF EXISTS airport_name_id;

		DROP TABLE IF EXISTS airport_elevation;
		DROP TABLE IF EXISTS airport_iata_code;
		DROP TABLE IF EXISTS airport_icao_code;
		DROP TABLE IF EXISTS airport_latitude;
		DROP TABLE IF EXISTS airport_longitude;
		DROP TABLE IF EXISTS airport_name;
		
	END IF;
END
$$;

-- TODO set all long/lats to numeric 5?

COMMIT;