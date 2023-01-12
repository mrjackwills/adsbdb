FROM ubuntu:22.04

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_TIME_CONT=Europe\
	DOCKER_TIME_CITY=Berlin \
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

# Download latest release from github
# This gets automatically updated via create_release.sh
RUN wget https://github.com/mrjackwills/adsbdb/releases/download/v0.0.19/adsbdb_linux_x86_64.tar.gz \
	&& tar xzvf adsbdb_linux_x86_64.tar.gz adsbdb && rm adsbdb_linux_x86_64.tar.gz \
	&& chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /app/adsbdb /logs \
	&& chmod +x /healthcheck/health_api.sh

# Use an unprivileged user
USER ${DOCKER_APP_USER}

CMD ["/app/adsbdb"]