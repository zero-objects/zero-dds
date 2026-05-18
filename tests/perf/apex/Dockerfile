# Apex.AI performance_test — CycloneDDS-Plugin Build (CI-3c v2 Welle).
#
# Basis: osrf/ros:jazzy-desktop bringt rclcpp + ament_cmake + colcon vor-
# installiert. Cyclone-Plugin braucht zusaetzlich `ros-jazzy-cyclonedds`
# + `ros-jazzy-rmw-cyclonedds-cpp`.
#
# Build-Erwartung:
#   cd /path/to/performance_test (mit ./performance_test/CMakeLists.txt)
#   docker build -f tests/perf/apex/Dockerfile -t apex-perf:cyclonedds .
#
# Voraussetzung: das Apex.AI-Repo ist als Submodule oder Sibling-Dir
# verfuegbar als `./performance_test`. Auf llvm-Bench-Host wurde es per
# `git clone https://gitlab.com/ApexAI/performance_test.git` gecloned.

FROM osrf/ros:jazzy-desktop

SHELL ["/bin/bash", "-c"]

RUN apt-get update && apt-get install -y --no-install-recommends \
    ros-jazzy-cyclonedds ros-jazzy-rmw-cyclonedds-cpp \
    && rm -rf /var/lib/apt/lists/*

COPY performance_test /perf_test/performance_test
WORKDIR /perf_test
RUN source /opt/ros/jazzy/setup.bash && \
    colcon build --packages-select performance_test \
      --cmake-args -DPERFORMANCE_TEST_PLUGIN=CYCLONEDDS -DBUILD_TESTING=OFF

COPY tests/perf/apex/perf_test_wrapper.sh /usr/local/bin/perf_test
RUN chmod +x /usr/local/bin/perf_test

ENTRYPOINT ["/usr/local/bin/perf_test"]
