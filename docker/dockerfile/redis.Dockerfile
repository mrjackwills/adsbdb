FROM redis:alpine3.21

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

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/init/init_redis.sh docker/confs/redis.conf /init/

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/healthcheck/health_redis.sh /healthcheck/

RUN chmod +x /healthcheck/health_redis.sh /init/init_redis.sh

ENTRYPOINT [ "/init/init_redis.sh" ]

CMD ["redis-server", "/init/redis.conf"]
