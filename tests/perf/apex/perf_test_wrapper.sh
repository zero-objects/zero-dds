#!/bin/bash
# Apex.AI perf_test entry-point — sourct ROS 2 jazzy + Apex install env.
source /opt/ros/jazzy/setup.bash
source /perf_test/install/setup.bash
exec /perf_test/install/performance_test/lib/performance_test/perf_test "$@"
