# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Builds `fastdds_robot`, the Fast DDS interop client for the #28 CI gate.
# The Robot type support is generated from robot.idl by Fast-DDS-Gen at
# configure time, then compiled against the installed Fast DDS SDK.
#
# Requires: an installed Fast DDS (find_package(fastdds)) + fastcdr, and the
# `fastddsgen` code generator on PATH (or passed via -DFASTDDSGEN=/path).
cmake_minimum_required(VERSION 3.16)
project(fastdds_robot CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(fastcdr REQUIRED)
find_package(fastdds REQUIRED)

# ---- code generation (configure time, so the sources exist for the build) --
find_program(FASTDDSGEN NAMES fastddsgen fast-dds-gen)
if(NOT FASTDDSGEN)
    message(FATAL_ERROR "fastddsgen not found; install Fast-DDS-Gen or pass -DFASTDDSGEN=/path/to/fastddsgen")
endif()

set(GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/gen")
file(MAKE_DIRECTORY "${GEN_DIR}")
# Invoke the generator from the IDL's own directory with a bare basename.
# Some Fast-DDS-Gen versions mirror a leading path component of the IDL into a
# subdirectory of -d (observed: `gen/fastdds/robot*.cxx`), which the flat GLOB
# below would miss. A bare basename keeps the generated sources flat in GEN_DIR,
# matching the `#include "robotPubSubTypes.hpp"` in fastdds_robot.cpp.
execute_process(
    COMMAND "${FASTDDSGEN}" -replace -d "${GEN_DIR}" robot.idl
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    RESULT_VARIABLE GEN_RC
    OUTPUT_VARIABLE GEN_OUT
    ERROR_VARIABLE GEN_ERR)
if(NOT GEN_RC EQUAL 0)
    message(FATAL_ERROR "fastddsgen failed (${GEN_RC}):\n${GEN_OUT}\n${GEN_ERR}")
endif()

file(GLOB GEN_SRCS "${GEN_DIR}/*.cxx")
if(GEN_SRCS STREQUAL "")
    message(FATAL_ERROR "fastddsgen produced no .cxx sources in ${GEN_DIR}")
endif()

# ---- executable ------------------------------------------------------------
add_executable(fastdds_robot fastdds_robot.cpp ${GEN_SRCS})
target_include_directories(fastdds_robot PRIVATE "${GEN_DIR}")
target_link_libraries(fastdds_robot PRIVATE fastdds fastcdr)

option(BUILD_LIVE_SHAPES "Build the local QoS live-suite endpoint" OFF)
if(BUILD_LIVE_SHAPES)
    set(SHAPE_GEN "${CMAKE_CURRENT_BINARY_DIR}/shape-gen")
    file(MAKE_DIRECTORY "${SHAPE_GEN}")
    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../../../interop/shapes/omg-dds-rtps.idl"
        "${SHAPE_GEN}/shape.idl" COPYONLY)
    execute_process(COMMAND "${FASTDDSGEN}" -replace -d "${SHAPE_GEN}" shape.idl
        WORKING_DIRECTORY "${SHAPE_GEN}" RESULT_VARIABLE SHAPE_RC)
    if(NOT SHAPE_RC EQUAL 0)
        message(FATAL_ERROR "Fast DDS Shapes generation failed: ${SHAPE_RC}")
    endif()
    file(GLOB SHAPE_SOURCES "${SHAPE_GEN}/*.cxx")
    add_executable(fastdds_shapes fastdds_shapes.cpp ${SHAPE_SOURCES})
    target_include_directories(fastdds_shapes PRIVATE "${SHAPE_GEN}")
    target_link_libraries(fastdds_shapes PRIVATE fastdds fastcdr)
endif()

# Surface the Fast DDS version to the binary's `version` command if the SDK
# exports it (best-effort; harmless when absent).
if(DEFINED fastdds_VERSION)
    target_compile_definitions(fastdds_robot PRIVATE FASTDDS_VERSION_STR="${fastdds_VERSION}")
endif()
