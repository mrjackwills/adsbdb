FROM postgres:14-alpine3.16

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_TIME_CONT=America \
	DOCKER_TIME_CITY=New_York \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

ENV TZ=${DOCKER_TIME_CONT}/${DOCKER_TIME_CITY}

RUN apk add --update --no-cache tzdata \
	&& cp /usr/share/zoneinfo/${TZ} /etc/localtime \
	&& echo ${TZ} > /etc/timezone \
	&& addgroup -g ${DOCKER_GUID} -S ${DOCKER_APP_GROUP} \
	&& adduser -u ${DOCKER_UID} -S -G ${DOCKER_APP_GROUP} ${DOCKER_APP_USER} \
	&& mkdir /pg_data /healthcheck /init \
	&& chown -R ${DOCKER_APP_USER}:postgres /pg_data \
	&& chown -R ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /healthcheck /init

# From pg_dump
COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./data/pg_dump.tar /init/

# From scratch with .sql & .csv files
# COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./init/init_db.sql ./data/aircraft.csv ./data/airport.csv ./data/country.csv ./data/flightroute_iata.csv ./data/flightroute_icao.csv /init/

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./init/dev.postgres_init.sh /docker-entrypoint-initdb.d/
# RUN chmod +x /docker-entrypoint-initdb.d/dev.postgres_init.sh

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./confs/.psqlrc /home/app_user/

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./healthcheck/health_postgres.sh /healthcheck/
RUN chmod +x /healthcheck/health_postgres.sh  /docker-entrypoint-initdb.d/dev.postgres_init.sh

USER ${DOCKER_APP_USER}