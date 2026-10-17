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

# rclone for backup targets (ADR 0031): https://rclone.org/downloads/
# Checksums from https://downloads.rclone.org/v1.75.2/SHA256SUMS.
FROM debian:${DEBIAN_RELEASE}-slim AS rclone
ARG TARGETARCH
ARG TALKOPS_RCLONE_VERSION=v1.75.2
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) sum=349ac8fba6ff65d6247043f1750cdcb518ec5d500ef91463a10d37c0ccdf3702 ;; \
      arm64) sum=7e1e8d69654941b7b7df84ee74c5f7fb09cce7d5496947fb7bf16b55ff427d10 ;; \
      *) echo "unsupported architecture $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    name="rclone-$TALKOPS_RCLONE_VERSION-linux-$TARGETARCH"; \
    curl -fsSLo /tmp/rclone.zip "https://downloads.rclone.org/$TALKOPS_RCLONE_VERSION/$name.zip"; \
    echo "$sum  /tmp/rclone.zip" | sha256sum -c -; \
    unzip -q /tmp/rclone.zip -d /tmp; \
    install -m 0755 "/tmp/$name/rclone" /usr/local/bin/rclone; \
    /usr/local/bin/rclone version

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
                /var/lib/talkops/cdr-spool /var/lib/talkops/targets \
    && chown -R talkops:talkops /var/lib/talkops

COPY --from=build /usr/local/bin/talkops /usr/local/bin/talkops
COPY --from=rclone /usr/local/bin/rclone /usr/local/bin/rclone
COPY --from=web /src/web/build /usr/share/talkops/web
COPY presets/ /usr/share/talkops/presets/
COPY THIRD_PARTY_NOTICES.md /usr/share/doc/talkops/THIRD_PARTY_NOTICES.md
COPY crates/talkops-provisioning/reference/fusionpbx/MPL-1.1.txt /usr/share/doc/talkops/fusionpbx/MPL-1.1.txt
COPY crates/talkops-provisioning/reference/fusionpbx/licenses.php /usr/share/doc/talkops/fusionpbx/licenses.php

ENV TALKOPS_LISTEN=0.0.0.0:8080 \
    TALKOPS_WEB_DIR=/usr/share/talkops/web \
    TALKOPS_LOG_FORMAT=json

USER talkops
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/talkops"]
CMD ["serve"]
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 CMD ["/usr/local/bin/talkops", "healthcheck"]
