#!/bin/sh
set -e

check_env() {
	missing=false
	for env_var in \
		DOCKER_PG_PORT \
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

main() {
	check_env
	PONG=$(pg_isready -U "$DB_NAME" -p "$DOCKER_PG_PORT")
	if expr "$PONG" : "/var/run/postgresql:$DOCKER_PG_PORT - accepting connections" >/dev/null; then
		exit 0
	else
		exit 1
	fi
}

main
