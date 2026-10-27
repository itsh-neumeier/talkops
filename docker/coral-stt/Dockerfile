# syntax=docker/dockerfile:1
#
# TalkOps coral-stt: German and English speech recognition on Google Coral
# Edge TPUs (ADR 0046). The TPU libraries and the models stay out of the
# media worker image. Models are not part of the image: put the compiled
# models into the models volume (see docs/en/recording.md and tools/coral/).
# Build context: repository root.

ARG RUST_VERSION=1.97
ARG DEBIAN_RELEASE=trixie

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS build
ARG TALKOPS_BUILD_REVISION=unknown
ENV TALKOPS_BUILD_REVISION=${TALKOPS_BUILD_REVISION}
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p talkops-coral-stt \
    && cp target/release/talkops-coral-stt /usr/local/bin/talkops-coral-stt

# TensorFlow Lite C library v2.19.1 (Apache-2.0,
# https://github.com/tensorflow/tensorflow), the version that the
# libedgetpu build below uses. XNNPACK is off: its CMake build fails in
# this version, and the Edge TPU does the model work.
FROM debian:${DEBIAN_RELEASE}-slim AS tflite
ARG TF_TAG=v2.19.1
ARG TF_COMMIT=f4247ebb6f9e7421f38c3f01a9a5d5cd54bd24fd
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates git cmake ninja-build g++ python3 \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    git clone --depth 1 --branch "$TF_TAG" https://github.com/tensorflow/tensorflow /tf; \
    test "$(git -C /tf rev-parse HEAD)" = "$TF_COMMIT"; \
    cmake -S /tf/tensorflow/lite/c -B /build -G Ninja -DCMAKE_BUILD_TYPE=Release \
      -DTFLITE_ENABLE_XNNPACK=OFF; \
    cmake --build /build -j"$(nproc)"; \
    mkdir -p /out; \
    cp /build/libtensorflowlite_c.so /out/; \
    cp /tf/LICENSE /out/LICENSE.tensorflow; \
    rm -rf /tf /build

# libedgetpu 16.0 for TensorFlow 2.19.1 (Apache-2.0), community build for
# Debian trixie: https://github.com/feranick/libedgetpu (release
# 16.0TF2.19.1-1). The "std" variant runs the TPU at the reduced clock.
FROM debian:${DEBIAN_RELEASE}-slim AS edgetpu
ARG TARGETARCH
ARG EDGETPU_URL=https://github.com/feranick/libedgetpu/releases/download/16.0TF2.19.1-1
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) sum=764d1bc680f65440ca1c8aca1d33ce986d7ca652b984ae0fc744d04b2f896e53 ;; \
      arm64) sum=df9e9b16c802f4e4ad6fd8d9d0866d25eff8ddef989813f398794546d267bd11 ;; \
      *) echo "unsupported architecture $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/edgetpu.deb "$EDGETPU_URL/libedgetpu1-std_16.0tf2.19.1-1.trixie_${TARGETARCH}.deb"; \
    echo "$sum  /tmp/edgetpu.deb" | sha256sum -c -; \
    dpkg-deb -x /tmp/edgetpu.deb /edgetpu; \
    mkdir -p /out; \
    cp /edgetpu/usr/lib/*/libedgetpu.so.1.0 /out/; \
    cp /edgetpu/usr/share/doc/libedgetpu1-std/copyright /out/copyright.libedgetpu; \
    cp /edgetpu/usr/lib/udev/rules.d/60-libedgetpu1-std.rules /out/

FROM debian:${DEBIAN_RELEASE}-slim
LABEL org.opencontainers.image.title="talkops-coral-stt" \
      org.opencontainers.image.description="TalkOps speech recognition on Coral Edge TPUs" \
      org.opencontainers.image.source="https://github.com/itsh-neumeier/talkops" \
      org.opencontainers.image.licenses="AGPL-3.0-only"

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata libusb-1.0-0 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 talkops \
    && useradd --system --uid 10001 --gid talkops --no-create-home --shell /usr/sbin/nologin talkops \
    && mkdir -p /var/lib/talkops/models/coral \
    && chown -R talkops:talkops /var/lib/talkops

COPY --from=tflite /out/libtensorflowlite_c.so /usr/local/lib/libtensorflowlite_c.so
COPY --from=tflite /out/LICENSE.tensorflow /usr/share/doc/talkops-coral-stt/LICENSE.tensorflow
COPY --from=edgetpu /out/libedgetpu.so.1.0 /usr/local/lib/libedgetpu.so.1.0
COPY --from=edgetpu /out/copyright.libedgetpu /usr/share/doc/talkops-coral-stt/copyright.libedgetpu
# For the host: the udev rule that gives the plugdev group access to USB sticks.
COPY --from=edgetpu /out/60-libedgetpu1-std.rules /usr/share/talkops/coral/60-libedgetpu1-std.rules
COPY --from=build /usr/local/bin/talkops-coral-stt /usr/local/bin/talkops-coral-stt
RUN ln -s libedgetpu.so.1.0 /usr/local/lib/libedgetpu.so.1 && ldconfig

ENV TALKOPS_LOG_FORMAT=json \
    TALKOPS_CORAL_TFLITE_LIB=/usr/local/lib/libtensorflowlite_c.so \
    TALKOPS_CORAL_EDGETPU_LIB=/usr/local/lib/libedgetpu.so.1 \
    TALKOPS_CORAL_MODELS_DIR=/var/lib/talkops/models/coral

USER talkops
ENTRYPOINT ["/usr/local/bin/talkops-coral-stt"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/usr/local/bin/talkops-coral-stt", "healthcheck"]
