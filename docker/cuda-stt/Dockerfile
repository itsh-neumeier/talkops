# syntax=docker/dockerfile:1
#
# TalkOps cuda-stt: German and English speech recognition on an NVIDIA GPU
# (ADR 0053). sherpa-onnx runs the models with the ONNX Runtime CUDA
# execution provider. The CUDA libraries and the models stay out of the
# media worker image. Models are not part of the image: administrators
# install them in the TalkOps UI (docs/en/recording.md).
# Build context: repository root. amd64 only.
#
# Host requirements: NVIDIA driver 580 or later (CUDA 13) and the NVIDIA
# Container Toolkit.

ARG RUST_VERSION=1.97
ARG DEBIAN_RELEASE=trixie

# sherpa-onnx v1.13.8 (Apache-2.0, https://github.com/k2-fsa/sherpa-onnx)
# with ONNX Runtime 1.28.2 for CUDA 13 and cuDNN 9. The version must be the
# version of the `sherpa-onnx` crate in crates/talkops-cuda-stt/Cargo.toml.
FROM debian:${DEBIAN_RELEASE}-slim AS sherpa
ARG SHERPA_VERSION=1.13.8
ARG SHERPA_SHA256=7eddaee14a42d143ec995ed68e9ec3623dd6ca9d22076139d4c0879c3121c819
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl bzip2 \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    name="sherpa-onnx-v${SHERPA_VERSION}-cuda-13.x-cudnn-9.x-onnxruntime1.28.2-linux-x64-gpu"; \
    curl -fsSLo /tmp/sherpa.tar.bz2 "https://github.com/k2-fsa/sherpa-onnx/releases/download/v${SHERPA_VERSION}/${name}.tar.bz2"; \
    echo "$SHERPA_SHA256  /tmp/sherpa.tar.bz2" | sha256sum -c -; \
    mkdir -p /opt/sherpa-onnx/lib; \
    tar xjf /tmp/sherpa.tar.bz2 -C /tmp; \
    for lib in libsherpa-onnx-c-api.so libonnxruntime.so libonnxruntime_providers_shared.so libonnxruntime_providers_cuda.so; do \
      cp "/tmp/${name}/lib/${lib}" /opt/sherpa-onnx/lib/; \
    done; \
    curl -fsSLo /opt/sherpa-onnx/LICENSE.sherpa-onnx "https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/v${SHERPA_VERSION}/LICENSE"; \
    rm -rf /tmp/sherpa.tar.bz2 "/tmp/${name}"

FROM rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS build
ARG TALKOPS_BUILD_REVISION=unknown
ENV TALKOPS_BUILD_REVISION=${TALKOPS_BUILD_REVISION} \
    SHERPA_ONNX_LIB_DIR=/opt/sherpa-onnx/lib
COPY --from=sherpa /opt/sherpa-onnx /opt/sherpa-onnx
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p talkops-cuda-stt \
    && cp target/release/talkops-cuda-stt /usr/local/bin/talkops-cuda-stt

FROM debian:${DEBIAN_RELEASE}-slim
LABEL org.opencontainers.image.title="talkops-cuda-stt" \
      org.opencontainers.image.description="TalkOps speech recognition on NVIDIA GPUs" \
      org.opencontainers.image.source="https://github.com/itsh-neumeier/talkops" \
      org.opencontainers.image.licenses="AGPL-3.0-only"

# CUDA 13.1 runtime libraries and cuDNN 9 from the NVIDIA CUDA repository
# for Debian 13 (no toolkit, no compiler). Only the libraries that the ONNX
# Runtime CUDA provider loads. libcuda.so.1 comes from the host driver
# through the NVIDIA Container Toolkit.
ARG CUDA_KEYRING_SHA256=d0d4ef986a44400f9db33c600ef33a985175e7cc63d805a10e1839c7a1e78f5f
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl tzdata; \
    curl -fsSLo /tmp/cuda-keyring.deb https://developer.download.nvidia.com/compute/cuda/repos/debian13/x86_64/cuda-keyring_1.1-1_all.deb; \
    echo "$CUDA_KEYRING_SHA256  /tmp/cuda-keyring.deb" | sha256sum -c -; \
    dpkg -i /tmp/cuda-keyring.deb; \
    rm /tmp/cuda-keyring.deb; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
      cuda-cudart-13-1=13.1.80-1 \
      libcublas-13-1=13.2.2.2-1 \
      libcurand-13-1=10.4.1.81-1 \
      libcudnn9-cuda-13=9.25.1.1-1; \
    apt-get purge -y curl; \
    apt-get autoremove -y; \
    rm -rf /var/lib/apt/lists/*; \
    groupadd --system --gid 10001 talkops; \
    useradd --system --uid 10001 --gid talkops --no-create-home --shell /usr/sbin/nologin talkops; \
    mkdir -p /var/lib/talkops/models/cuda; \
    chown -R talkops:talkops /var/lib/talkops

COPY --from=sherpa /opt/sherpa-onnx/lib/ /usr/local/lib/
COPY --from=sherpa /opt/sherpa-onnx/LICENSE.sherpa-onnx /usr/share/doc/talkops-cuda-stt/LICENSE.sherpa-onnx
COPY --from=build /usr/local/bin/talkops-cuda-stt /usr/local/bin/talkops-cuda-stt
# All libraries resolve, except the driver library from the host.
RUN set -eux; \
    echo /usr/local/cuda-13.1/targets/x86_64-linux/lib > /etc/ld.so.conf.d/cuda-13.conf; \
    ldconfig; \
    for f in /usr/local/bin/talkops-cuda-stt /usr/local/lib/libonnxruntime_providers_cuda.so; do \
      if ldd "$f" | grep "not found" | grep -v libcuda.so.1; then echo "missing dependency of $f" >&2; exit 1; fi; \
    done; \
    ldconfig -p | grep -q libcudnn.so.9

ENV TALKOPS_LOG_FORMAT=json \
    TALKOPS_CUDA_MODELS_DIR=/var/lib/talkops/models/cuda \
    NVIDIA_VISIBLE_DEVICES=all \
    NVIDIA_DRIVER_CAPABILITIES=compute,utility

USER talkops
ENTRYPOINT ["/usr/local/bin/talkops-cuda-stt"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=3 CMD ["/usr/local/bin/talkops-cuda-stt", "healthcheck"]
