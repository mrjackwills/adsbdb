FROM alpine:3.17

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_TIME_CONT=Europe\
	DOCKER_TIME_CITY=Berlin \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

ENV TZ=${DOCKER_TIME_CONT}/${DOCKER_TIME_CITY}

RUN apk add --update --no-cache gnupg tzdata postgresql-client \
	&& cp /usr/share/zoneinfo/${TZ} /etc/localtime \
	&& echo ${TZ} > /etc/timezone \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /backups /redis_data /logs

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./backup/postgres_backup.sh /postgres_backup.sh
RUN chmod +x /postgres_backup.sh

USER ${DOCKER_APP_USER}

CMD [ "/postgres_backup.sh" ]