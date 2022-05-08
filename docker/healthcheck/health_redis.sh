#!/bin/sh
set -e

PONG=$(redis-cli -h "${DOCKER_REDIS_HOST}" -p "${DOCKER_REDIS_PORT}" -a "${DOCKER_REDIS_PASSWORD}" --no-auth-warning ping)
if expr "$PONG" : 'PONG' > /dev/null;
then
	exit 0
else
	exit 1
fi