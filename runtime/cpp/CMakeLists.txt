# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The C++ wire runtime (DDS-XTypes 1.3 §7.4) as a CMake target — the one
# codec `crates/cpp`'s DDS-PSM-Cxx binding and `zerodds-idlc generate --lang
# c++ --cpp-mapping native` build against. `Makefile` in this directory remains the fast,
# dependency-free way to build and run `tests/selftest.cpp` and
# `tests/corpus_bytes.cpp` (`make check`) without configuring the whole
# zerodds CMake tree; this file adds the same two binaries to CTest so
# `ctest` from a zerodds CMake build runs them too. The two build systems
# compile the same four translation units and are kept in step by hand,
# the way `crates/cpp/CMakeLists.txt`'s header list already is.

cmake_minimum_required(VERSION 3.21)
project(zerodds-wire-cpp
    DESCRIPTION "ZeroDDS C++ wire runtime (DDS-XTypes 1.3 §7.4)"
    LANGUAGES CXX
)

include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(ZERODDS_BUILD_TESTS "Build zerodds C++ tests" ${PROJECT_IS_TOP_LEVEL})

add_library(zerodds-wire-cpp STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/src/writer.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/reader.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/float128.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/md5.cpp"
)
add_library(zerodds::zerodds-wire-cpp ALIAS zerodds-wire-cpp)

target_include_directories(zerodds-wire-cpp
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
)

# The Makefile's warning set (`Makefile` in this directory), kept to the
# library's own translation units rather than propagated PUBLIC: a caller
# linking `zerodds::zerodds-wire-cpp` (crates/cpp, generated code) is not
# thereby opted into `-Werror` for its own, unrelated sources.
if(NOT MSVC)
    target_compile_options(zerodds-wire-cpp PRIVATE
        -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Werror
    )
endif()

# Sanitizer (ZERODDS_SANITIZER, set at the root project: none|address|
# undefined|thread|address,undefined) — same knob crates/cpp/CMakeLists.txt
# reads, so the wire runtime and the PSM binding it backs are instrumented
# together.
if(DEFINED ZERODDS_SANITIZER AND NOT ZERODDS_SANITIZER STREQUAL "none" AND NOT MSVC)
    target_compile_options(zerodds-wire-cpp PUBLIC
        -fsanitize=${ZERODDS_SANITIZER} -fno-omit-frame-pointer -g
    )
    target_link_options(zerodds-wire-cpp PUBLIC -fsanitize=${ZERODDS_SANITIZER})
endif()

# Tests — the byte oracle (`corpus_bytes`, against `tests/interop-corpus`)
# and the self-test (`selftest`). Registered with CTest (root CMakeLists.txt
# `include(CTest)`) the way crates/cpp/CMakeLists.txt registers
# `smoke_dds_psm`, so `ctest` runs them, not just `ZERODDS_BUILD_TESTS`
# producing binaries nobody invokes.
if(ZERODDS_BUILD_TESTS)
    add_executable(zerodds_wire_cpp_selftest "${CMAKE_CURRENT_SOURCE_DIR}/tests/selftest.cpp")
    target_link_libraries(zerodds_wire_cpp_selftest PRIVATE zerodds::zerodds-wire-cpp)

    add_executable(zerodds_wire_cpp_corpus_bytes "${CMAKE_CURRENT_SOURCE_DIR}/tests/corpus_bytes.cpp")
    target_link_libraries(zerodds_wire_cpp_corpus_bytes PRIVATE zerodds::zerodds-wire-cpp)

    if(BUILD_TESTING)
        add_test(NAME zerodds_wire_cpp_selftest COMMAND zerodds_wire_cpp_selftest)
        add_test(NAME zerodds_wire_cpp_corpus_bytes
            COMMAND zerodds_wire_cpp_corpus_bytes "${CMAKE_CURRENT_SOURCE_DIR}/../../tests"
        )
    endif()
endif()

# Installation — header-only interface plus the static archive, laid down
# beside crates/cpp's own install() block so a `find_package(zerodds)`
# consumer that links `zerodds::zerodds-cpp` also gets
# `zerodds::zerodds-wire-cpp` transitively.
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include/"
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
    FILES_MATCHING PATTERN "*.hpp"
)
install(TARGETS zerodds-wire-cpp
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
)
