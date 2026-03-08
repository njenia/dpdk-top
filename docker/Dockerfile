# DPDK + testpmd + dpdk-top for local testing (e.g. on Mac via Docker).
# Uses virtual devices (net_null) so no real NIC or hugepages are required.
# Reliable for testing dpdk-top; not for benchmarking real hardware.

FROM ubuntu:22.04 AS dpdk-builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    meson \
    ninja-build \
    python3-pyelftools \
    libnuma-dev \
    pkg-config \
    wget \
    ca-certificates \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# DPDK 24.11 LTS
ARG DPDK_VERSION=24.11.1
RUN wget -q https://fast.dpdk.org/rel/dpdk-${DPDK_VERSION}.tar.xz -O /tmp/dpdk.tar.xz \
    && mkdir -p /tmp/dpdk && tar -C /tmp/dpdk --strip-components=1 -xf /tmp/dpdk.tar.xz \
    && rm /tmp/dpdk.tar.xz

WORKDIR /tmp/dpdk
# -Dplatform=generic for ARM64 (e.g. Colima/Apple Silicon) where implementer may be unrecognized
RUN meson setup build -Dplatform=generic \
    && ninja -C build \
    && DESTDIR=/dpdk-install meson install -C build

# Build dpdk-top for Linux
FROM rust:1.88-bookworm AS dpdk-top-builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

# Final image
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    libnuma1 \
    libatomic1 \
    iproute2 \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/*

COPY --from=dpdk-builder /dpdk-install/usr/local/ /usr/local/
COPY --from=dpdk-top-builder /build/target/release/dpdk-top /usr/local/bin/

# So that testpmd can find DPDK libs (layout varies: lib64 or lib/x86_64-linux-gnu)
ENV LD_LIBRARY_PATH=/usr/local/lib64:/usr/local/lib/x86_64-linux-gnu
# Telemetry socket default path
ENV DPDK_TELEMETRY_SOCKET=/var/run/dpdk/rte/dpdk_telemetry.v2

RUN mkdir -p /var/run/dpdk && ldconfig || true

COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
