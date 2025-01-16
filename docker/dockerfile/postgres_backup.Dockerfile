FROM alpine:3.21

ARG DOCKER_GUID=1000 \
DOCKER_UID=1000 \
DOCKER_APP_USER=app_user \
DOCKER_APP_GROUP=app_group

RUN apk add --update --no-cache gnupg age \
	&& apk add --update --no-cache --repository=http://dl-cdn.alpinelinux.org/alpine/edge/main postgresql17-client \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /backups /redis_data /logs

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/backup/postgres_backup.sh /postgres_backup.sh

RUN chmod +x /postgres_backup.sh

USER ${DOCKER_APP_USER}

CMD [ "/postgres_backup.sh" ]