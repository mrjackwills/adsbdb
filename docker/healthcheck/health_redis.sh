#!/bin/sh
set -e

check_env() {
	missing=false
	for env_var in \
		DOCKER_REDIS_HOST \
		DOCKER_REDIS_PORT
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
	PONG=$(redis-cli -h "$DOCKER_REDIS_HOST" -p "$DOCKER_REDIS_PORT" ping)
	if expr "$PONG" : 'PONG' >/dev/null; then
		exit 0
	else
		exit 1
	fi
}

main
