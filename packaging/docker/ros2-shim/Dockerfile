# packaging/docker/ros2-shim/Dockerfile — Spec §5.1 (zerodds/ros2-jazzy:1.0).
# Variants per ROS distro: humble | iron | jazzy. Default = jazzy.
ARG ROS_DISTRO=jazzy
FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin zerodds-ros2-shim --bin zerodds-admin

FROM ros:${ROS_DISTRO}-ros-base
ARG ROS_DISTRO
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 tini ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 999 --home /var/lib/zerodds --shell /usr/sbin/nologin zerodds
COPY --from=builder /build/target/release/zerodds-ros2-shim /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-admin     /usr/local/bin/
COPY packaging/linux/configs/ros2-shim.yaml.example /etc/zerodds/ros2-shim.yaml
RUN mkdir -p /var/log/zerodds /var/lib/zerodds /etc/zerodds/certs \
    && chown -R zerodds:zerodds /var/log/zerodds /var/lib/zerodds /etc/zerodds
ENV RMW_IMPLEMENTATION=rmw_zerodds_cpp
ENV ROS_DOMAIN_ID=0
USER zerodds
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/zerodds-ros2-shim"]
CMD ["--config", "/etc/zerodds/ros2-shim.yaml", "--watch"]
