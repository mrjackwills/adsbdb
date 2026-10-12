#!/bin/sh
set -e

# Check the required env's are present
check_env() {
	missing=false
	for env_var in \
		DOCKER_REDIS_HOST \
		DOCKER_REDIS_PORT \
		REDISCLI_AUTH
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

check_env

cat > /redis.conf <<EOF
bind ${DOCKER_REDIS_HOST}
port ${DOCKER_REDIS_PORT}
requirepass "${REDISCLI_AUTH}"
logfile ""
loglevel notice
save 60 1
dir /redis_data
repl-diskless-sync no
repl-diskless-load disabled
maxmemory 3gb
maxmemory-policy allkeys-lru
maxclients 1500
timeout 300
tcp-keepalive 300
tcp-backlog 511
databases 16
rdbcompression yes
rdbchecksum yes
stop-writes-on-bgsave-error yes
EOF

exec "$@"