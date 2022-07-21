#!/bin/sh
set -e

sed -i "s|# requirepass foobared|requirepass ${DOCKER_REDIS_PASSWORD}|" /init/redis.conf
sed -i "s=bind 127.0.0.1 -::1=bind ${DOCKER_REDIS_HOST}=" /init/redis.conf
sed -i "s=port 6379=port ${DOCKER_REDIS_PORT}=" /init/redis.conf
sed -i "s=/var/run/redis_6379.pid=/var/run/redis_${DOCKER_REDIS_PORT}.pid=" /init/redis.conf
sed -i "s=logfile \"\"=logfile /redis_logs/redis-server.log=" /init/redis.conf
sed -i "s=# save 3600 1 300 100 60 10000=save 60 1=" /init/redis.conf
sed -i "s=dir ./=dir /redis_data=" /init/redis.conf
sed -i "s=repl-diskless-sync yes=repl-diskless-sync no=" /init/redis.conf

exec "$@"