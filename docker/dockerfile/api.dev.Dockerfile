FROM debian:bullseye-slim

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_TIME_CONT=America \
	DOCKER_TIME_CITY=New_York \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

ENV TZ=${DOCKER_TIME_CONT}/${DOCKER_TIME_CITY}

RUN apt-get update \
	&& apt-get install -y ca-certificates wget \
	&& update-ca-certificates \
	&& groupadd --gid ${DOCKER_GUID} ${DOCKER_APP_GROUP} \
	&& useradd --no-create-home --no-log-init --uid ${DOCKER_UID} --gid ${DOCKER_GUID} ${DOCKER_APP_USER} \
	&& mkdir /healthcheck /logs \
	&& chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /logs

WORKDIR /app

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/healthcheck/health_api.sh /healthcheck

# Copy from local release destination
COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} target/release/adsbdb /app/

RUN chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /app/adsbdb \
	&& chmod +x /healthcheck/health_api.sh

# Use an unprivileged user
USER ${DOCKER_APP_USER}

CMD ["/app/adsbdb"]