# Installed-SDK consumer gate for the zerodds CMake package (spec P0-4/P0-5).
#
# Unlike consumer-regression (which pulls zerodds in via add_subdirectory from
# the source tree), THIS project only ever sees the `cmake --install` staging
# tree: it find_package(zerodds CONFIG REQUIRED) against CMAKE_PREFIX_PATH and
# links a trivial C and C++ program — proving that every artifact a native
# consumer needs (headers, library, config, generator module, pc) is actually
# present in the staging tree, with no reference back into the repo.
#
# Driven by cmake/tests/run-sdk-consumer.sh.
cmake_minimum_required(VERSION 3.21 FATAL_ERROR)
project(zerodds_sdk_consumer LANGUAGES C CXX)

find_package(zerodds CONFIG REQUIRED)

add_executable(use_c use.c)
target_link_libraries(use_c PRIVATE zerodds::zerodds-c)

add_executable(use_cpp use.cpp)
target_link_libraries(use_cpp PRIVATE zerodds::zerodds-cpp)
