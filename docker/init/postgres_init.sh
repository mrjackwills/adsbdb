#!/bin/sh -x
set -e

create_adsbdb_user() {
	echo "create_adsbdb_user"
	psql -v ON_ERROR_STOP=1 -U "${POSTGRES_USER}" -d "${POSTGRES_USER}" <<-EOSQL
		CREATE ROLE ${DB_USER} WITH LOGIN PASSWORD '${DB_PASSWORD}';
	EOSQL
}

create_adsbdb_database() {
	echo "create_adsbdb_database"
	psql -v ON_ERROR_STOP=1 -U "${POSTGRES_USER}" -d "${POSTGRES_USER}" <<-EOSQL
		CREATE DATABASE ${DB_NAME};
	EOSQL
}

# Create db from .sql file, requires other data (*.csv etc) to read to build
bootstrap_from_sql_file() {
	psql -U "${POSTGRES_USER}" -d "${POSTGRES_USER}" -f /init/init_db.sql
}

# ! psql -v ON_ERROR_STOP=1 -U "adsbdb" -d "${POSTGRES_USER}" -f "/init/migrations.sql"

# Run any & all migrations
run_migrations() {
	if ! psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "${DB_NAME}" -f "/init/migrations.sql"; then
		echo "Error: Failed to run migrations.sql" >&2
		exit 1
		fi
}
	

# restore a db from a pg_dump file
restore_pg_dump() {
	echo "restore_pg_dump"
	pg_restore -U "${POSTGRES_USER}" -O --exit-on-error --single-transaction -d "${DB_NAME}" -v /init/pg_dump.tar
	psql -v ON_ERROR_STOP=1 -U "${POSTGRES_USER}" <<-EOSQL
		GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO ${DB_NAME};
		GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO ${DB_NAME};
	EOSQL
}

from_pg_dump() {
	create_adsbdb_user
	create_adsbdb_database
	restore_pg_dump
}

from_scratch() {
	create_adsbdb_user
	bootstrap_from_sql_file
}

main() {
	if [ -f "/init/pg_dump.tar" ]; then
		from_pg_dump
	else
		from_scratch
	fi
		run_migrations
}

main
