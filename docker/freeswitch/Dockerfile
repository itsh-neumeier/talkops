# syntax=docker/dockerfile:1
#
# FreeSWITCH for TalkOps, built from source with a pinned version and a
# curated module set (see build-modules.conf). FreeSWITCH only carries a
# minimal bootstrap configuration; everything else is served by TalkOps via
# mod_xml_curl (see docs/adr/0006-control-plane-xml-curl-esl.md).

ARG DEBIAN_RELEASE=trixie

FROM debian:${DEBIAN_RELEASE}-slim AS build

ARG FREESWITCH_VERSION=v1.10.12
ARG SOFIA_SIP_VERSION=v1.13.17
# spandsp has no releases that match FreeSWITCH 1.10.x; this is the commit the
# FreeSWITCH build documentation uses (master has since bumped its ABI to 4.0).
ARG SPANDSP_COMMIT=0d2e6ac65e0e8f53d652665a743015a88bf048d4

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        autoconf automake build-essential ca-certificates cmake git libtool libtool-bin \
        nasm pkg-config uuid-dev yasm \
        libcurl4-openssl-dev libedit-dev libjpeg-dev libldns-dev libncurses-dev libogg-dev \
        libopus-dev libpcre2-dev libsndfile1-dev libspeex-dev libspeexdsp-dev libsqlite3-dev \
        libssl-dev libtiff-dev zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# The pinned releases predate GCC 14 and curl 8.x type checks, and
# FreeSWITCH builds with -Werror. Keep warnings visible but non-fatal, and
# downgrade the diagnostics GCC 14 made errors by default.
ENV CFLAGS="-O2 -g -Wno-error -Wno-error=incompatible-pointer-types -Wno-error=implicit-function-declaration -Wno-error=int-conversion -Wno-error=implicit-int -Wno-error=return-mismatch"

WORKDIR /usr/src

RUN git clone --depth 1 --branch "${SOFIA_SIP_VERSION}" https://github.com/freeswitch/sofia-sip.git \
    && cd sofia-sip \
    && ./bootstrap.sh \
    && ./configure --prefix=/usr/local --disable-static \
    && make -j"$(nproc)" \
    && make install

RUN git clone https://github.com/freeswitch/spandsp.git \
    && cd spandsp \
    && git checkout "${SPANDSP_COMMIT}" \
    && ./bootstrap.sh \
    && ./configure --prefix=/usr/local --disable-static \
    && make -j"$(nproc)" \
    && make install \
    && ldconfig

RUN git clone --depth 1 --branch "${FREESWITCH_VERSION}" https://github.com/signalwire/freeswitch.git
COPY patches/ /usr/src/patches/
COPY build-modules.conf /usr/src/freeswitch/modules.conf
# Patches backport fixes from FreeSWITCH master (see each file's header).
RUN cd freeswitch && for p in /usr/src/patches/*.patch; do git apply --verbose "$p"; done
RUN cd freeswitch \
    && ./bootstrap.sh -j \
    && ./configure --prefix=/usr/local/freeswitch --disable-static \
    && make -j"$(nproc)" \
    && make install \
    # Drop headers, static archives and the stock sample configuration.
    && rm -rf /usr/local/freeswitch/include /usr/local/freeswitch/etc/freeswitch \
    && find /usr/local/freeswitch -name '*.la' -delete \
    && (strip --strip-unneeded /usr/local/freeswitch/bin/* /usr/local/freeswitch/lib/*.so* /usr/local/freeswitch/mod/*.so 2>/dev/null || true)

FROM debian:${DEBIAN_RELEASE}-slim

ARG FREESWITCH_VERSION=v1.10.12
LABEL org.opencontainers.image.title="talkops-freeswitch" \
      org.opencontainers.image.description="FreeSWITCH ${FREESWITCH_VERSION} for TalkOps" \
      org.opencontainers.image.source="https://github.com/itsh-neumeier/talkops" \
      org.opencontainers.image.licenses="MPL-1.1"

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libcurl4t64 libedit2 libjpeg62-turbo libldns3t64 libncursesw6 libogg0 \
        libopus0 libpcre2-8-0 libsndfile1 libspeex1 libspeexdsp1 libsqlite3-0 libssl3t64 \
        libtiff6 libuuid1 zlib1g \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/lib/ /usr/local/lib/
COPY --from=build /usr/local/freeswitch/ /usr/local/freeswitch/

# Fail the build if any shared library dependency is missing at runtime.
RUN ldconfig \
    && ! ldd /usr/local/freeswitch/bin/freeswitch /usr/local/freeswitch/mod/*.so | grep "not found"

# Fixed IDs shared by all TalkOps images so mounted volumes (recordings,
# voicemail, sounds) are writable by every service.
RUN groupadd --system --gid 10001 talkops \
    && useradd --system --uid 10001 --gid talkops --home-dir /usr/local/freeswitch --shell /usr/sbin/nologin talkops

# FreeSWITCH 1.10 uses an FHS layout below its prefix: configuration in
# etc/freeswitch, state in var/lib/freeswitch, modules in lib/freeswitch/mod.
COPY conf/ /usr/local/freeswitch/etc/freeswitch/
COPY docker-entrypoint.sh fs-healthcheck.sh /usr/local/bin/
RUN mkdir -p /var/lib/talkops/recordings /var/lib/talkops/voicemail /var/lib/talkops/sounds \
    && for d in var/lib/freeswitch/db var/lib/freeswitch/storage var/lib/freeswitch/recordings \
                share/freeswitch/sounds/music/default \
                var/log/freeswitch/xml_cdr var/run/freeswitch var/cache/freeswitch; do \
           mkdir -p /usr/local/freeswitch/$d; \
       done \
    && chown -R talkops:talkops /usr/local/freeswitch /var/lib/talkops \
    && ln -s /usr/local/freeswitch/bin/fs_cli /usr/local/bin/fs_cli \
    && ln -s /usr/local/freeswitch/bin/freeswitch /usr/local/bin/freeswitch

USER talkops
WORKDIR /usr/local/freeswitch
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
HEALTHCHECK --interval=15s --timeout=5s --start-period=60s --retries=3 CMD ["/usr/local/bin/fs-healthcheck.sh"]
