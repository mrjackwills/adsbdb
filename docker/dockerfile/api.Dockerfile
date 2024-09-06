#############
## Builder ##
#############

FROM rust:slim AS builder

WORKDIR /usr/src

# Create blank project
RUN cargo new adsbdb

# We want dependencies cached, so copy those first
COPY Cargo.* /usr/src/adsbdb/

# Set the working directory
WORKDIR /usr/src/adsbdb

# Prepared statements required to build for sqlx macros
COPY .sqlx /usr/src/adsbdb/.sqlx

# This is a dummy build to get the dependencies cached - probably not needed - as run via a github action
RUN cargo build --release

# Now copy in the rest of the sources
COPY src /usr/src/adsbdb/src/

## Touch main.rs to prevent cached release build
RUN touch /usr/src/adsbdb/src/main.rs

# This is the actual application build
RUN cargo build --release

####################
## Runtime Ubuntu ##
####################

FROM ubuntu:22.04

ARG DOCKER_GUID=1000 \
	DOCKER_UID=1000 \
	DOCKER_APP_USER=app_user \
	DOCKER_APP_GROUP=app_group

RUN apt-get update \
	&& apt-get install -y ca-certificates wget \
	&& update-ca-certificates \
	&& groupadd --gid ${DOCKER_GUID} ${DOCKER_APP_GROUP} \
	&& useradd --no-create-home --no-log-init --uid ${DOCKER_UID} --gid ${DOCKER_GUID} ${DOCKER_APP_USER} \
	&& mkdir /healthcheck /logs \
	&& chown ${DOCKER_APP_USER}:${DOCKER_APP_GROUP} /logs

WORKDIR /app

COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} ./docker/healthcheck/health_api.sh /healthcheck

RUN chmod +x /healthcheck/health_api.sh

# Copy from host filesystem - used when debugging
# COPY --chown=${DOCKER_APP_USER}:${DOCKER_APP_GROUP} target/release/adsbdb /app

COPY --from=builder /usr/src/adsbdb/target/release/adsbdb /app/

# Use an unprivileged user
USER ${DOCKER_APP_USER}

CMD ["/app/adsbdb"]