# CMakeLists.txt — ament_cmake-Package "rmw_zerodds".
#
# Verpackt librmw_zerodds.so + rmw_zerodds.h (aus cargo build) als
# ROS-2-RMW-Plugin. Wird vom rclcpp via Plugin-Search-Path geladen,
# wenn `RMW_IMPLEMENTATION=rmw_zerodds_cpp` gesetzt ist.
#
# Build-Erwartung:
#   cargo build -p rmw-zerodds-shim --release   # erzeugt librmw_zerodds.so
#   colcon build --packages-select rmw_zerodds  # bindet rein

cmake_minimum_required(VERSION 3.16)
project(rmw_zerodds VERSION 0.0.0 LANGUAGES CXX)

find_package(ament_cmake REQUIRED)

set(ZERODDS_REPO_ROOT "$ENV{ZERODDS_HOME}" CACHE PATH "Repo-Root von ZeroDDS")
if(NOT ZERODDS_REPO_ROOT)
  message(FATAL_ERROR
    "ZERODDS_HOME nicht gesetzt. Auf den Repo-Root zeigen lassen, in "
    "dem 'cargo build -p rmw-zerodds-shim --release' gelaufen ist.")
endif()

set(ZERODDS_INCLUDE_DIR
    "${ZERODDS_REPO_ROOT}/crates/rmw-zerodds-shim/include")
set(ZERODDS_LIB_DIR
    "${ZERODDS_REPO_ROOT}/target/release")

if(NOT EXISTS "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h")
  message(FATAL_ERROR
    "rmw_zerodds.h nicht gefunden — vorher 'cargo build -p rmw-zerodds-shim "
    "--release' im ${ZERODDS_REPO_ROOT}.")
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
  message(FATAL_ERROR "librmw_zerodds.${_lib_suffix} nicht in ${ZERODDS_LIB_DIR}.")
endif()

# Install header + lib so dass rclcpp das findet
install(FILES "${ZERODDS_INCLUDE_DIR}/rmw_zerodds.h"
        DESTINATION include/${PROJECT_NAME})

install(FILES "${_zd_lib}" "${_zd_underly}"
        DESTINATION lib)

ament_export_include_directories(include)
ament_export_libraries(rmw_zerodds zerodds)

# Markiere als rmw-Implementation (rclcpp-Discovery sucht nach
# CMake-Variable PLUGIN_TYPE).
set(RMW_IMPLEMENTATION_LIBRARY rmw_zerodds)

ament_package()
