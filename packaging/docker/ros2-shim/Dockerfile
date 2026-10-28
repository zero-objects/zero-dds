# packaging/docker/ros2-shim/Dockerfile — Spec zerodds-deployment-1.0.md §5.1
# (zerodds/ros2-jazzy:1.0), zerodds-ros2-bridge-1.0.md §11 ("mit ros2-Image als
# Base + ZeroDDS-RMW vorinstalliert").
# Variants per ROS distro: humble | iron | jazzy. Default = jazzy.
#
# The image carries the RMW rmw_zerodds_cpp, selected with
# RMW_IMPLEMENTATION=rmw_zerodds_cpp, and the diagnostics CLI
# zerodds-ros2-shim; the default command runs `doctor` once. A ros2 drop-in
# goes to /etc/zerodds/zerodds.d/ (template: /usr/share/zerodds/configs/).
#
# The RMW arrives the way a user installs it: the builder stage builds the
# Debian package ros-<distro>-rmw-zerodds-cpp from the same ament project the
# package is built from (crates/rmw-zerodds-shim/ament, through
# packaging/linux/deb/build-rmw-deb.sh) and this stage dpkg-installs it into
# the distro's ROS prefix. A second, image-only file layout would be one more
# place where the package and the image can disagree about what an installed
# rmw_zerodds_cpp consists of.
#
# Everything is built on the ROS image itself: the RMW links the distro's
# rmw and rcutils, and the Rust libraries must match its glibc.
ARG ROS_DISTRO=jazzy

FROM ros:${ROS_DISTRO}-ros-base AS builder
ARG ROS_DISTRO
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake curl ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.88
ENV PATH=/root/.cargo/bin:${PATH}
WORKDIR /build
COPY . .
RUN cargo build --release -p rmw-zerodds-shim -p zerodds-c-api \
    && cargo build --release --bin zerodds-admin
RUN . /opt/ros/${ROS_DISTRO}/setup.sh \
    && bash packaging/linux/deb/build-rmw-deb.sh \
         --distro "${ROS_DISTRO}" --out /build/dist-deb

FROM ros:${ROS_DISTRO}-ros-base
ARG ROS_DISTRO
COPY --from=builder /build/dist-deb /tmp/dist-deb
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 tini ca-certificates \
    && dpkg -i /tmp/dist-deb/ros-${ROS_DISTRO}-rmw-zerodds-cpp_*.deb \
    && rm -rf /tmp/dist-deb \
    && useradd --system --uid 999 --home /var/lib/zerodds --shell /usr/sbin/nologin zerodds \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/zerodds-ros2-shim /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-admin     /usr/local/bin/
COPY packaging/linux/configs/ros2.yaml.example /usr/share/zerodds/configs/ros2.yaml.example
RUN mkdir -p /var/log/zerodds /var/lib/zerodds /etc/zerodds/certs /etc/zerodds/zerodds.d \
    && chown -R zerodds:zerodds /var/log/zerodds /var/lib/zerodds /etc/zerodds
# The RMW is in the distro's own prefix now, so the two variables a
# colcon/ament setup.sh would export are the distro's alone. ros2 works in
# this container without sourcing it first.
ENV AMENT_PREFIX_PATH=/opt/ros/${ROS_DISTRO}
ENV LD_LIBRARY_PATH=/opt/ros/${ROS_DISTRO}/lib
ENV RMW_IMPLEMENTATION=rmw_zerodds_cpp
ENV ROS_DOMAIN_ID=0
USER zerodds
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/zerodds-ros2-shim"]
CMD ["doctor"]
