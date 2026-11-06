# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The C wire runtime (DDS-XTypes 1.3 §7.4) and its self-test as CMake targets,
# so that `ctest` from a zerodds CMake build with the tests on runs the
# self-test (the root CMakeLists.txt adds this directory then). `Makefile` in
# this directory remains the way to build both without CMake (`make check`);
# the two compile the same translation units with the same warnings and are
# kept in step by hand, as runtime/cpp's two are.

cmake_minimum_required(VERSION 3.21)
project(zerodds-wire-c
    DESCRIPTION "ZeroDDS C wire runtime (DDS-XTypes 1.3 §7.4)"
    LANGUAGES C
)

add_library(zerodds-wire-c STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/src/writer.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/reader.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/md5.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/float128.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/float128_decimal.c"
)
target_include_directories(zerodds-wire-c PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
set_target_properties(zerodds-wire-c PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
if(NOT MSVC)
    # The Makefile's warning set, as errors.
    target_compile_options(zerodds-wire-c PRIVATE
        -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Werror)
    target_link_libraries(zerodds-wire-c PUBLIC m)
endif()

add_executable(zerodds_wire_c_selftest "${CMAKE_CURRENT_SOURCE_DIR}/tests/selftest.c")
target_link_libraries(zerodds_wire_c_selftest PRIVATE zerodds-wire-c)
set_target_properties(zerodds_wire_c_selftest PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
if(NOT MSVC)
    target_compile_options(zerodds_wire_c_selftest PRIVATE
        -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Werror)
endif()
if(BUILD_TESTING)
    add_test(NAME zerodds_wire_c_selftest COMMAND zerodds_wire_c_selftest)
endif()
