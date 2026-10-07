# syntax=docker/dockerfile:1
#
# TalkOps server: Rust control plane + statically built web UI.
# Build context: repository root.

ARG RUST_VERSION=1.97
ARG NODE_VERSION=22
ARG DEBIAN_RELEASE=trixie

FROM node:${NODE_VERSION}-${DEBIAN_RELEASE}-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json web/.npmrc ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p talkops-api --bin talkops \
    && cp target/release/talkops /usr/local/bin/talkops

FROM debian:${DEBIAN_RELEASE}-slim
LABEL org.opencontainers.image.title="talkops-server" \
      org.opencontainers.image.description="TalkOps PBX control plane and web UI" \
      org.opencontainers.image.source="https://github.com/itsh-neumeier/talkops" \
      org.opencontainers.image.licenses="AGPL-3.0-only"

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata postgresql-client \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 talkops \
    && useradd --system --uid 10001 --gid talkops --no-create-home --shell /usr/sbin/nologin talkops \
    && mkdir -p /var/lib/talkops/recordings /var/lib/talkops/voicemail /var/lib/talkops/sounds \
                /var/lib/talkops/provisioning /var/lib/talkops/snapshots /var/lib/talkops/backups \
    && chown -R talkops:talkops /var/lib/talkops

COPY --from=build /usr/local/bin/talkops /usr/local/bin/talkops
COPY --from=web /src/web/build /usr/share/talkops/web
COPY presets/ /usr/share/talkops/presets/

ENV TALKOPS_LISTEN=0.0.0.0:8080 \
    TALKOPS_WEB_DIR=/usr/share/talkops/web \
    TALKOPS_LOG_FORMAT=json

USER talkops
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/talkops"]
CMD ["serve"]
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 CMD ["/usr/local/bin/talkops", "healthcheck"]
