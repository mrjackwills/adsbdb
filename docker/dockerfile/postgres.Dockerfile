FROM postgres:18-alpine3.24

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

RUN addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /pg_data /healthcheck /init /logs \
	&& chown -R ${DOCKER_APP_USER}:postgres /pg_data \
	&& chown -R ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /healthcheck /init /logs

# From dump OR scratch
COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./docker/init/init_postgres.sh /docker-entrypoint-initdb.d/

# This is a bit of a hack, for pg_dump.tar, this can fail if pg_dump.tar doesn't exist!
COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./docker/init/init_db.sql ./docker/data/pg_dump.tar* /init/

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./docker/migrations /migrations

# Create .psqlrc file
RUN printf '\\timing\n\\x auto\n\\pset null [NULL]\n' > "/home/${DOCKER_APP_USER}/.psqlrc"
RUN chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} "/home/${DOCKER_APP_USER}/.psqlrc"

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./docker/healthcheck/health_postgres.sh /healthcheck/

RUN chmod +x /healthcheck/health_postgres.sh /docker-entrypoint-initdb.d/init_postgres.sh


USER $DOCKER_APP_USER
