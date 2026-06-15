# syntax=docker/dockerfile:1.6

ARG APT_MIRROR=""
ARG CARGO_RS_PROXY=""
ARG HTTP_PROXY=""
ARG HTTPS_PROXY=""
ARG NO_PROXY=""

FROM rust:1.93-slim-bookworm AS builder
ARG APT_MIRROR
ARG CARGO_RS_PROXY
ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
ENV CARGO_TARGET_DIR=/app/target
ENV HTTP_PROXY=${HTTP_PROXY} HTTPS_PROXY=${HTTPS_PROXY} NO_PROXY=${NO_PROXY}

RUN if [ -n "${APT_MIRROR}" ]; then \
    find /etc/apt -type f \( -name '*.sources' -o -name 'sources.list' \) -exec \
      sed -i "s|http://deb.debian.org|http://${APT_MIRROR}|g; s|http://security.debian.org|http://${APT_MIRROR}|g" {} +; \
  fi \
  && if [ -n "${CARGO_RS_PROXY}" ]; then \
    mkdir -p /usr/local/cargo && printf '%s\n' \
      '[source.crates-io]' \
      'replace-with = "rsproxy-sparse"' \
      '' \
      '[source.rsproxy-sparse]' \
      'registry = "sparse+https://rsproxy.cn/index/"' \
      > /usr/local/cargo/config.toml; \
  fi \
  && apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY rust-toolchain.toml ./
RUN channel="$(sed -n 's/channel = "\(.*\)"/\1/p' rust-toolchain.toml)" \
  && rustup toolchain install "${channel}" \
  && rustup default "${channel}" \
  && cargo --version >/dev/null
COPY Cargo.toml Cargo.lock ./
# Copy workspace member manifests so Cargo can resolve workspace-shared deps
# without invalidating the build on every unrelated source change.
COPY crates/server/Cargo.toml crates/server/Cargo.toml
COPY crates/trusted-key-auth/Cargo.toml crates/trusted-key-auth/Cargo.toml
COPY crates/mcp/Cargo.toml crates/mcp/Cargo.toml
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/executors/Cargo.toml crates/executors/Cargo.toml
COPY crates/services/Cargo.toml crates/services/Cargo.toml
COPY crates/worktree-manager/Cargo.toml crates/worktree-manager/Cargo.toml
COPY crates/workspace-manager/Cargo.toml crates/workspace-manager/Cargo.toml
COPY crates/relay-control/Cargo.toml crates/relay-control/Cargo.toml
COPY crates/relay-hosts/Cargo.toml crates/relay-hosts/Cargo.toml
COPY crates/relay-protocol/Cargo.toml crates/relay-protocol/Cargo.toml
COPY crates/relay-webrtc/Cargo.toml crates/relay-webrtc/Cargo.toml
COPY crates/relay-ws/Cargo.toml crates/relay-ws/Cargo.toml
COPY crates/server-info/Cargo.toml crates/server-info/Cargo.toml
COPY crates/utils/Cargo.toml crates/utils/Cargo.toml
COPY crates/git/Cargo.toml crates/git/Cargo.toml
COPY crates/git-host/Cargo.toml crates/git-host/Cargo.toml
COPY crates/local-deployment/Cargo.toml crates/local-deployment/Cargo.toml
COPY crates/deployment/Cargo.toml crates/deployment/Cargo.toml
COPY crates/review/Cargo.toml crates/review/Cargo.toml
COPY crates/api-types crates/api-types
COPY crates/relay-types crates/relay-types
COPY crates/relay-control crates/relay-control
COPY crates/relay-protocol crates/relay-protocol
COPY crates/relay-tunnel-core crates/relay-tunnel-core
COPY crates/relay-tunnel crates/relay-tunnel
COPY crates/relay-ws crates/relay-ws
COPY crates/ws-bridge crates/ws-bridge

RUN mkdir -p /app/bin

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=relay-target,target=/app/target \
    cargo build --locked --release --manifest-path crates/relay-tunnel/Cargo.toml \
 && cp /app/target/release/relay-server /app/bin/relay-server

FROM debian:bookworm-slim
ARG APT_MIRROR

RUN if [ -n "${APT_MIRROR}" ]; then \
    find /etc/apt -type f \( -name '*.sources' -o -name 'sources.list' \) -exec \
      sed -i "s|http://deb.debian.org|http://${APT_MIRROR}|g; s|http://security.debian.org|http://${APT_MIRROR}|g" {} +; \
  fi \
  && apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 wget \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --uid 10001 appuser

COPY --from=builder /app/bin/relay-server /usr/local/bin/relay-server

USER appuser

ENV RELAY_LISTEN_ADDR=0.0.0.0:8082 \
    RUST_LOG=info

EXPOSE 8082

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["wget","--spider","-q","http://127.0.0.1:8082/health"]

ENTRYPOINT ["/usr/local/bin/relay-server"]
