# syntax=docker/dockerfile:1
#
# TalkOps media worker: TTS (Piper) and transcription (whisper.cpp) jobs.
# Piper and its voices are pinned by checksum; whisper.cpp and the CUDA
# variant follow in phase 5. Build context: repository root.

ARG RUST_VERSION=1.97
ARG DEBIAN_RELEASE=trixie

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p talkops-media-worker \
    && cp target/release/talkops-media-worker /usr/local/bin/talkops-media-worker

# Piper 2023.11.14-2 (https://github.com/rhasspy/piper/releases) and voices
# from https://huggingface.co/rhasspy/piper-voices (v1.0.0):
#   de_DE-thorsten-medium – dataset CC0
#   en_US-ljspeech-medium – dataset public domain
FROM debian:${DEBIAN_RELEASE}-slim AS piper
ARG TARGETARCH
ARG PIPER_URL=https://github.com/rhasspy/piper/releases/download/2023.11.14-2
ARG VOICES_URL=https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) arch=x86_64; sum=a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992 ;; \
      arm64) arch=aarch64; sum=fea0fd2d87c54dbc7078d0f878289f404bd4d6eea6e7444a77835d1537ab88eb ;; \
      *) echo "unsupported architecture $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/piper.tgz "$PIPER_URL/piper_linux_$arch.tar.gz"; \
    echo "$sum  /tmp/piper.tgz" | sha256sum -c -; \
    mkdir -p /opt && tar -xzf /tmp/piper.tgz -C /opt; \
    mkdir -p /voices; cd /voices; \
    curl -fsSLo de_DE-thorsten-medium.onnx "$VOICES_URL/de/de_DE/thorsten/medium/de_DE-thorsten-medium.onnx"; \
    curl -fsSLo de_DE-thorsten-medium.onnx.json "$VOICES_URL/de/de_DE/thorsten/medium/de_DE-thorsten-medium.onnx.json"; \
    curl -fsSLo en_US-ljspeech-medium.onnx "$VOICES_URL/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx"; \
    curl -fsSLo en_US-ljspeech-medium.onnx.json "$VOICES_URL/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx.json"; \
    printf '%s\n' \
      "7e64762d8e5118bb578f2eea6207e1a35a8e0c30595010b666f983fc87bb7819  de_DE-thorsten-medium.onnx" \
      "974adee790533adb273a1ac88f49027d2a1b8f0f2cf4905954a4791e79264e85  de_DE-thorsten-medium.onnx.json" \
      "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22  en_US-ljspeech-medium.onnx" \
      "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860  en_US-ljspeech-medium.onnx.json" \
      | sha256sum -c -

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

COPY --from=piper /opt/piper /opt/piper
COPY --from=piper /voices /usr/share/talkops/voices
COPY --from=build /usr/local/bin/talkops-media-worker /usr/local/bin/talkops-media-worker

ENV TALKOPS_LOG_FORMAT=json \
    TALKOPS_WORKER_HEARTBEAT_FILE=/tmp/talkops-worker.alive

USER talkops
ENTRYPOINT ["/usr/local/bin/talkops-media-worker"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/usr/local/bin/talkops-media-worker", "healthcheck"]
