FROM redis:alpine3.23

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

RUN deluser redis \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /redis_logs /redis_data /init /healthcheck \
	&& touch /redis_logs/redis-server.log \
	&& chown -R ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /redis_logs /redis_data /init /healthcheck

WORKDIR /

USER ${DOCKER_APP_USER}

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/healthcheck/health_redis.sh /healthcheck/

CMD sh -c 'exec redis-server \
	--bind "${DOCKER_REDIS_HOST}" \
	--port "${DOCKER_REDIS_PORT}" \
	--requirepass "${DOCKER_REDIS_PASSWORD}" \
	--logfile /redis_logs/redis-server.log \
	--loglevel notice \
	--save "60 1" \
	--dir /redis_data \
	--repl-diskless-sync no \
	--repl-diskless-load disabled \
	--maxmemory 3gb \
	--maxmemory-policy allkeys-lru \
	--maxclients 100 \
	--timeout 300 \
	--tcp-keepalive 300 \
	--tcp-backlog 511 \
	--databases 16 \
	--appendonly yes \
	--appendfilename appendonly.aof \
	--appendfsync everysec \
	--auto-aof-rewrite-percentage 100 \
	--auto-aof-rewrite-min-size 64mb \
	--stop-writes-on-bgsave-error yes \
	--rdbcompression yes \
	--rdbchecksum yes'
