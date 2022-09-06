ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/base:0-${VARIANT}

ENV PATH="/home/vscode/.cargo/bin:${PATH}"

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
	&& apt-get -y install --no-install-recommends build-essential pkg-config libssl-dev

USER vscode
RUN curl --proto '=https' --tlsv1.2 -sSf curl https://sh.rustup.rs | sh -s -- -y
