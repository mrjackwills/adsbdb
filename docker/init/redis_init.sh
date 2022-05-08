#!/bin/sh
set -e

sed -i "s|requirepass replace_me|requirepass ${DOCKER_REDIS_PASSWORD}|" /init/redis.conf
sed -i "s|bind redis |bind ${DOCKER_REDIS_HOST}|" /init/redis.conf

exec "$@"