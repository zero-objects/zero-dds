# CMakeLists.txt — ament_cmake package "rmw_zerodds".
#
# Packages librmw_zerodds.so + rmw_zerodds.h (from cargo build) as a
# ROS-2 RMW plugin. Loaded by rclcpp via the plugin search path
# when `RMW_IMPLEMENTATION=rmw_zerodds_cpp` is set.
#
# Build expectation:
#   cargo build -p rmw-zerodds-shim --release   # produces librmw_zerodds.so
#   colcon build --packages-select rmw_zerodds  # pulls it in

cmake_minimum_required(VERSION 3.16)
project(rmw_zerodds VERSION 0.0.0 LANGUAGES CXX)

find_package(ament_cmake REQUIRED)

set(ZERODDS_REPO_ROOT "$ENV{ZERODDS_HOME}" CACHE PATH "Repo root of ZeroDDS")
if(NOT ZERODDS_REPO_ROOT)
  message(FATAL_ERROR
    "ZERODDS_HOME not set. Point it at the repo root where "
    "'cargo build -p rmw-zerodds-shim --release' was run.")
endif()

set(ZERODDS_INCLUDE_DIR
    "${ZERODDS_REPO_ROOT}/crates/rmw-zerodds-shim/include")
set(ZERODDS_LIB_DIR
    "${ZERODDS_REPO_ROOT}/target/release")

if(NOT EXISTS "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h")
  message(FATAL_ERROR
    "rmw_zerodds.h not found — run 'cargo build -p rmw-zerodds-shim "
    "--release' in ${ZERODDS_REPO_ROOT} first.")
endif()

# librmw_zerodds platform-specific suffix
if(APPLE)
  set(_lib_suffix "dylib")
elseif(WIN32)
  set(_lib_suffix "dll")
else()
  set(_lib_suffix "so")
endif()

set(_zd_lib "${ZERODDS_LIB_DIR}/librmw_zerodds.${_lib_suffix}")
set(_zd_underly "${ZERODDS_LIB_DIR}/libzerodds.${_lib_suffix}")

if(NOT EXISTS "${_zd_lib}")
  message(FATAL_ERROR "librmw_zerodds.${_lib_suffix} not in ${ZERODDS_LIB_DIR}.")
endif()

# Install header + lib so rclcpp finds it
install(FILES "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h"
        DESTINATION include/${PROJECT_NAME})

install(FILES "${_zd_lib}" "${_zd_underly}"
        DESTINATION lib)

ament_export_include_directories(include)
ament_export_libraries(rmw_zerodds zerodds)

# Mark as an rmw implementation (rclcpp discovery looks for the
# CMake variable PLUGIN_TYPE).
set(RMW_IMPLEMENTATION_LIBRARY rmw_zerodds)

ament_package()
