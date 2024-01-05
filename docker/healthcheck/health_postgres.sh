#!/bin/sh
set -e

PONG=$(pg_isready -U "$DB_NAME" -p "${DOCKER_PG_PORT}")
if expr "$PONG" : "/var/run/postgresql:${DOCKER_PG_PORT} - accepting connections" >/dev/null; then
	exit 0
else
	exit 1
fi
