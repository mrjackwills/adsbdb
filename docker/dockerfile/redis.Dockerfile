FROM redis:alpine3.23

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

RUN deluser redis \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /redis_data /init /healthcheck \
	&& chown -R ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /redis_data /init /healthcheck

WORKDIR /

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/healthcheck/health_redis.sh /healthcheck/
RUN chmod +x /healthcheck/health_redis.sh

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/init/init_redis.sh /init/
RUN chmod +x /init/init_redis.sh

# Writable at runtime so the entrypoint can (re)generate it from the env vars
RUN touch /redis.conf && chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /redis.conf

ENTRYPOINT ["/init/init_redis.sh"]

USER $DOCKER_APP_USER
WORKDIR /redis_data

CMD ["redis-server", "/redis.conf"]
