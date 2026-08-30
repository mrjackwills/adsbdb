#!/bin/bash
set -e

check_env() {
	missing=false
	for env_var in \
		POSTGRES_USER \
		DB_USER \
		DB_NAME
	do
		eval "value=\${$env_var}"
		if [ -z "$value" ]; then
			echo "ERROR: environment variable $env_var is not set" >&2
			missing=true
		fi
	done

	if [ "$missing" = true ]; then
		exit 1
	fi
}

create_database() {
	echo "create ${DB_NAME} database"
	psql -v ON_ERROR_STOP=0 -U "$POSTGRES_USER" -d "$POSTGRES_USER" <<-EOSQL
		CREATE DATABASE ${DB_NAME};
	EOSQL
}

create_user() {
	echo "create  ${DB_USER} user"
	psql -v ON_ERROR_STOP=0 -U "$POSTGRES_USER" -d "$POSTGRES_USER" <<-EOSQL
		CREATE ROLE ${DB_USER} WITH LOGIN PASSWORD '$DB_PASSWORD';
	EOSQL
}

# Create db from .sql file, requires other data (*.csv etc) to read to build
bootstrap_from_sql_file() {
	psql -U "${POSTGRES_USER}" -d "${POSTGRES_USER}" -f /init/init_db.sql
}

# Run any & all migrations
run_migrations() {
	# need env port here!
	if ! psql -v ON_ERROR_STOP=1 -U "${POSTGRES_USER}" -d "${DB_NAME}" -f "/init/migrations.sql"; then
		echo "Error: Failed to run migrations.sql" >&2
		exit 1
	fi
}

# # restore a db from a pg_dump file
restore_pg_dump() {
	echo "restore_pg_dump"
	pg_restore -U "${POSTGRES_USER}" -O --exit-on-error --single-transaction -d "${DB_NAME}" -v /init/pg_dump.tar
	psql -v ON_ERROR_STOP=0 -U "${POSTGRES_USER}" <<-EOSQL
		GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO ${DB_NAME};
		GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO ${DB_NAME};
	EOSQL
}

from_pg_dump() {
	create_user
	create_database
	restore_pg_dump
}

from_scratch() {
	create_user
	bootstrap_from_sql_file
}

create_tables() {
	if [ -f "/init/pg_dump.tar" ]; then
		from_pg_dump
	else
		from_scratch
	fi
}

main() {
	check_env

	if [ "$1" == "migrations" ]; then
		run_migrations
	else
		create_tables
	fi
}

main "$1"