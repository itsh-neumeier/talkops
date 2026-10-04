# syntax=docker/dockerfile:1
#
# TalkOps media worker: TTS (Piper) and transcription (whisper.cpp) jobs.
# Piper, whisper.cpp and the CUDA variant are added in phases 3 and 5.
# Build context: repository root.

ARG RUST_VERSION=1.97
ARG DEBIAN_RELEASE=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p talkops-media-worker \
    && cp target/release/talkops-media-worker /usr/local/bin/talkops-media-worker

FROM debian:${DEBIAN_RELEASE}-slim
LABEL org.opencontainers.image.title="talkops-media-worker" \
      org.opencontainers.image.description="TalkOps TTS and transcription worker" \
      org.opencontainers.image.source="https://github.com/itsh-neumeier/talkops" \
      org.opencontainers.image.licenses="AGPL-3.0-only"

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 talkops \
    && useradd --system --uid 10001 --gid talkops --no-create-home --shell /usr/sbin/nologin talkops \
    && mkdir -p /var/lib/talkops/recordings /var/lib/talkops/voicemail /var/lib/talkops/sounds \
                /var/lib/talkops/models \
    && chown -R talkops:talkops /var/lib/talkops

COPY --from=build /usr/local/bin/talkops-media-worker /usr/local/bin/talkops-media-worker

ENV TALKOPS_LOG_FORMAT=json \
    TALKOPS_WORKER_HEARTBEAT_FILE=/tmp/talkops-worker.alive

USER talkops
ENTRYPOINT ["/usr/local/bin/talkops-media-worker"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/usr/local/bin/talkops-media-worker", "healthcheck"]
