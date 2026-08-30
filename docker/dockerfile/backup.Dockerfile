FROM alpine:3.24

ARG DOCKER_GUID=1000 \
DOCKER_UID=1000 \
DOCKER_APP_USER=app_user \
DOCKER_APP_GROUP=app_group

RUN apk add --update --no-cache age postgresql18-client redis \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /backups /logs /temp_backup

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} docker/backup/create_backup.sh /create_backup.sh

RUN chown -R ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /backups /logs /temp_backup

RUN chmod +x /create_backup.sh

USER $DOCKER_APP_USER

CMD [ "/create_backup.sh" ]
