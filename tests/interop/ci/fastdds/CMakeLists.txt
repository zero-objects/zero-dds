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
execute_process(
    COMMAND "${FASTDDSGEN}" -replace -d "${GEN_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/robot.idl"
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

# Surface the Fast DDS version to the binary's `version` command if the SDK
# exports it (best-effort; harmless when absent).
if(DEFINED fastdds_VERSION)
    target_compile_definitions(fastdds_robot PRIVATE FASTDDS_VERSION_STR="${fastdds_VERSION}")
endif()
