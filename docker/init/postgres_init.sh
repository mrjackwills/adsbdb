#!/bin/sh -x
set -e

create_adsbdb_user() {
	echo "create_adsbdb_user"
	psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "postgres" -p "$DOCKER_PG_PORT" <<-EOSQL
CREATE ROLE $DB_USER WITH LOGIN PASSWORD '$DB_PASSWORD';
EOSQL
}

create_adsbdb_database() {
	echo "create_adsbdb_database"
	psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "postgres" -p "$DOCKER_PG_PORT" <<-EOSQL
CREATE DATABASE $DB_NAME;
EOSQL
}

bootstrap_from_sql_file() {
	psql -U "$POSTGRES_USER" -d postgres -f /init/init_db.sql
}

restore_pg_dump() {
	echo "restore_pg_dump"
	pg_restore -U "$POSTGRES_USER" -O --exit-on-error --single-transaction -d "$DB_NAME" -p "$DOCKER_PG_PORT" -v /init/pg_dump.tar
	psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$DB_NAME" -p "$DOCKER_PG_PORT" <<-EOSQL
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO $DB_NAME;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO $DB_NAME;
EOSQL
}
# if pg_dump then resotre, elsee from scratch
main () {

	# If creating from scratch, with .sql file & .csv files
	# create_adsbdb_user
	# bootstrap_from_sql_file
	# add_dev_adsbdb

	# If restoring from pg_dump.tar
	create_adsbdb_user
	create_adsbdb_database
	restore_pg_dump
}

main