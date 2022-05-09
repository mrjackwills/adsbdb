###########
# Builder #
###########

FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev
RUN update-ca-certificates

ENV DOCKER_APP_USER=adsbdb
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${DOCKER_APP_USER}"


WORKDIR /adsbdb

COPY Cargo.* ./
COPY ./src ./src

RUN cargo build --target x86_64-unknown-linux-musl --release

############
# App only #
############

FROM alpine

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /adsbdb

RUN mkdir /healthcheck

COPY --chown=${DOCKER_APP_USER} docker/healthcheck/health_api.sh /healthcheck/
RUN chmod +x /healthcheck/health_api.sh

# Copy our build
COPY --from=builder /adsbdb/target/x86_64-unknown-linux-musl/release/adsbdb ./

# Use an unprivileged user.
USER ${DOCKER_APP_USER}:${DOCKER_APP_USER}

CMD ["/adsbdb/adsbdb"]