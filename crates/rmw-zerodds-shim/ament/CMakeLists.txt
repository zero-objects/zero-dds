# CMakeLists.txt — ament_cmake package "rmw_zerodds_cpp".
#
# The ROS 2 RMW implementation `rmw_zerodds_cpp`: builds the C layer
# (rmw_c/rmw_zerodds.c with its C++ helpers) against the ROS packages it
# includes, links the Rust DDS bridge librmw_zerodds and the ZeroDDS C API
# libzerodds from a cargo build, installs all three into lib/ and registers
# the implementation in the ament index, as rmw_cyclonedds_cpp does.
# `RMW_IMPLEMENTATION=rmw_zerodds_cpp` then loads lib/librmw_zerodds_cpp.
#
# Build expectation:
#   cargo build -p rmw-zerodds-shim -p zerodds-c-api --release
#   ZERODDS_HOME=<repo> colcon build --packages-select rmw_zerodds_cpp

cmake_minimum_required(VERSION 3.16)
project(rmw_zerodds_cpp VERSION 0.0.0 LANGUAGES C CXX)

find_package(ament_cmake REQUIRED)
find_package(rcutils REQUIRED)
find_package(rmw REQUIRED)
find_package(rmw_dds_common REQUIRED)
find_package(rosidl_runtime_c REQUIRED)
find_package(rosidl_typesupport_introspection_c REQUIRED)
find_package(rosidl_typesupport_introspection_cpp REQUIRED)
# ROS 2 Jazzy's dynamic-type-support package. Not on Humble, hence QUIET:
# rmw_zerodds.c and rmw_zerodds_dynamic.cpp both gate on
# `__has_include(<rosidl_dynamic_typesupport/types.h>)`, and on Humble
# that header simply is not on the include path this find_package would
# otherwise add — the same gate, expressed at the build-system level.
find_package(rosidl_dynamic_typesupport QUIET)

set(ZERODDS_REPO_ROOT "$ENV{ZERODDS_HOME}" CACHE PATH "Repo root of ZeroDDS")
if(NOT ZERODDS_REPO_ROOT)
  message(FATAL_ERROR
    "ZERODDS_HOME not set. Point it at the repo root where "
    "'cargo build -p rmw-zerodds-shim -p zerodds-c-api --release' was run.")
endif()

set(ZERODDS_INCLUDE_DIR "${ZERODDS_REPO_ROOT}/crates/rmw-zerodds-shim/include")
set(ZERODDS_RMW_C_DIR "${ZERODDS_REPO_ROOT}/crates/rmw-zerodds-shim/rmw_c")
set(ZERODDS_LIB_DIR "${ZERODDS_REPO_ROOT}/target/release")

if(APPLE)
  set(_lib_suffix "dylib")
elseif(WIN32)
  set(_lib_suffix "dll")
else()
  set(_lib_suffix "so")
endif()
set(_zd_lib "${ZERODDS_LIB_DIR}/librmw_zerodds.${_lib_suffix}")
set(_zd_underly "${ZERODDS_LIB_DIR}/libzerodds.${_lib_suffix}")
foreach(_f "${_zd_lib}" "${_zd_underly}" "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h")
  if(NOT EXISTS "${_f}")
    message(FATAL_ERROR
      "${_f} not found — run 'cargo build -p rmw-zerodds-shim -p zerodds-c-api "
      "--release' in ${ZERODDS_REPO_ROOT} first.")
  endif()
endforeach()

add_library(rmw_zerodds_cpp SHARED
  "${ZERODDS_RMW_C_DIR}/rmw_zerodds.c"
  "${ZERODDS_RMW_C_DIR}/rmw_zerodds_cxx.cpp"
  "${ZERODDS_RMW_C_DIR}/rmw_zerodds_qos.cpp"
  "${ZERODDS_RMW_C_DIR}/rmw_zerodds_dynamic.cpp")
set_target_properties(rmw_zerodds_cpp PROPERTIES CXX_STANDARD 17 CXX_STANDARD_REQUIRED ON)
target_include_directories(rmw_zerodds_cpp PRIVATE "${ZERODDS_RMW_C_DIR}")
# The ROS packages by the CMake targets each exports (`<package>_TARGETS`),
# not by `ament_target_dependencies`: Kilted deprecates that command and
# Rolling no longer has it. Every package here exports targets on every
# supported distro; one that does not stops the build by name.
set(_zd_ros_deps
  rcutils rmw rmw_dds_common rosidl_runtime_c
  rosidl_typesupport_introspection_c rosidl_typesupport_introspection_cpp)
if(rosidl_dynamic_typesupport_FOUND)
  message(STATUS
    "rmw_zerodds: rosidl_dynamic_typesupport found — dynamic type support "
    "(rmw_zerodds_dynamic.cpp) builds for real.")
  list(APPEND _zd_ros_deps rosidl_dynamic_typesupport)
else()
  message(STATUS
    "rmw_zerodds: rosidl_dynamic_typesupport not found (Humble) — "
    "rmw_zerodds_dynamic.cpp compiles to an empty translation unit.")
endif()
foreach(_zd_dep IN LISTS _zd_ros_deps)
  if(NOT ${_zd_dep}_TARGETS)
    message(FATAL_ERROR "${_zd_dep} exports no CMake targets (${_zd_dep}_TARGETS is empty)")
  endif()
  target_link_libraries(rmw_zerodds_cpp ${${_zd_dep}_TARGETS})
endforeach()
# By name, not by path: the dependency entries then name the libraries, and
# the loader finds the installed copies next to this one.
target_link_directories(rmw_zerodds_cpp PRIVATE "${ZERODDS_LIB_DIR}")
target_link_libraries(rmw_zerodds_cpp rmw_zerodds zerodds)
if(NOT APPLE AND NOT WIN32)
  set_target_properties(rmw_zerodds_cpp PROPERTIES INSTALL_RPATH "$ORIGIN")
endif()

# REP-2008 Pillar I — extensions to ament.
#
# "Package maintainers are encouraged to integrate hardware acceleration in
#  their ROS 2 packages by using the CMake hardware acceleration variables [...]
#  These CMake macros are meant to be used by maintainers to conditionally
#  compile their ROS 2 packages and acceleration kernels for those accelerators
#  that implement the architecture."   — REP-2008 §Specification
#
# ROS_ACCELERATION is set by ament_hardware_acceleration when the workspace
# targets any REP-2008 accelerator; ROS_VITIS by its ament_vitis
# specialization. Both packages are optional: an unaccelerated workspace
# configures exactly as before.
find_package(ament_hardware_acceleration QUIET)
find_package(ament_vitis QUIET)

# Kernel levels this rmw implementation serves (REP-2008 §"Kernel levels in
# ROS 2"), as the bitmask rmw_zerodds_acceleration_kernel_levels() returns:
# level 3 (the middleware is the ROS 2 underlayer) + level 2 (the rmw loan
# surface hands an accelerator a registerable buffer). Level 1 belongs to the
# application. Consumers read it via ament_export_dependencies below.
set(RMW_ZERODDS_ACCELERATION_KERNEL_LEVELS "2;3")

if(ROS_ACCELERATION)
  message(STATUS
    "rmw_zerodds: REP-2008 accelerator target detected "
    "(ROS_VITIS=${ROS_VITIS}); kernel levels "
    "${RMW_ZERODDS_ACCELERATION_KERNEL_LEVELS}, loans page-aligned by default.")
  # A REP-2008 workspace wants every loaned message buffer to start on its own
  # page so the accelerator runtime pins exactly that buffer. The runtime knob
  # is ZERODDS_LOAN_ALIGNMENT (see rmw_zerodds_ros2_ext.h); the build records
  # the intent so a dsv hook or launch file can export it.
  set(RMW_ZERODDS_LOAN_ALIGNMENT "page")
else()
  set(RMW_ZERODDS_LOAN_ALIGNMENT "")
endif()

# The implementation and the two libraries it loads; the headers carry the
# REP-2007 type-adaptation and REP-2008 loan-memory surface.
install(TARGETS rmw_zerodds_cpp
        LIBRARY DESTINATION lib
        ARCHIVE DESTINATION lib
        RUNTIME DESTINATION bin)
install(FILES "${_zd_lib}" "${_zd_underly}" DESTINATION lib)
install(FILES "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h"
              "${ZERODDS_RMW_C_DIR}/rmw_zerodds_ros2_ext.h"
        DESTINATION include/${PROJECT_NAME})

ament_export_include_directories(include)
ament_export_libraries(rmw_zerodds_cpp)
ament_export_dependencies(rcutils rmw rmw_dds_common rosidl_runtime_c
  rosidl_typesupport_introspection_c rosidl_typesupport_introspection_cpp)
if(rosidl_dynamic_typesupport_FOUND)
  ament_export_dependencies(rosidl_dynamic_typesupport)
endif()

# rmw_implementation finds implementations and their type supports through
# the ament index ("rmw_typesupport", "rmw_typesupport_c/_cpp").
register_rmw_implementation(
  "c:rosidl_typesupport_c:rosidl_typesupport_introspection_c"
  "cpp:rosidl_typesupport_cpp:rosidl_typesupport_introspection_cpp")

ament_package()
