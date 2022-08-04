FROM alpine:3.16

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_TIME_CONT=America \
	DOCKER_TIME_CITY=New_York \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

ENV VIRT=".build_packages"
ENV TZ=${DOCKER_TIME_CONT}/${DOCKER_TIME_CITY}

RUN addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& apk --no-cache add --virtual ${VIRT} tzdata \
	&& cp /usr/share/zoneinfo/${TZ} /etc/localtime \
	&& echo ${TZ} > /etc/timezone \
	&& apk del ${VIRT}

WORKDIR /app

RUN mkdir /healthcheck /logs
COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/healthcheck/health_api.sh /healthcheck

# Download latest release from github
RUN wget https://github.com/mrjackwills/adsbdb/releases/download/v0.0.13/adsbdb_linux_x86_64_musl.tar.gz \
	&& tar xzvf adsbdb_linux_x86_64_musl.tar.gz adsbdb && rm adsbdb_linux_x86_64_musl.tar.gz \
	&& chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /app/adsbdb /logs \
	&& chmod +x /healthcheck/health_api.sh

# Use an unprivileged user
USER ${DOCKER_APP_USER}

CMD ["/app/adsbdb"]