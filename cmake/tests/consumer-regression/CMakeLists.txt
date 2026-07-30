# Consumer regression test for the zerodds CMake build (P0-3: no parent mutation).
#
# This mini "parent" project pulls zerodds in via add_subdirectory() the way a
# real FetchContent / vendored consumer would, then asserts — at configure time,
# no build required — that zerodds left the parent scope intact and did not
# configure its own developer-only example/test targets.
#
# Driven by cmake/tests/run-consumer-regression.sh.
cmake_minimum_required(VERSION 3.21 FATAL_ERROR)
project(zerodds_consumer_regression LANGUAGES C CXX)

get_filename_component(ZERODDS_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../.." ABSOLUTE)
message(STATUS "consumer-regression: adding zerodds from ${ZERODDS_ROOT}")
add_subdirectory("${ZERODDS_ROOT}" "${CMAKE_BINARY_DIR}/_zerodds")

# --- Assertions -----------------------------------------------------------
set(_fail "")

# 1. Parent CMAKE_BUILD_TYPE must be untouched (was Debug; must NOT be forced to
#    Release by zerodds).
if(NOT CMAKE_BUILD_TYPE STREQUAL "Debug")
  string(APPEND _fail "  - CMAKE_BUILD_TYPE mutated: expected 'Debug', got '${CMAKE_BUILD_TYPE}'\n")
endif()

# 2. Parent BUILD_TESTING must stay OFF (zerodds must not include(CTest) as a
#    subproject, which would inject/flip a BUILD_TESTING cache entry).
if(BUILD_TESTING)
  string(APPEND _fail "  - BUILD_TESTING flipped ON (expected OFF)\n")
endif()

# 3. The parent's own generic BUILD_* options must be untouched by zerodds.
if(NOT BUILD_EXAMPLES)
  string(APPEND _fail "  - parent BUILD_EXAMPLES mutated (expected ON)\n")
endif()
if(BUILD_TESTS)
  string(APPEND _fail "  - parent BUILD_TESTS mutated (expected OFF)\n")
endif()

# 4. zerodds example/test targets must NOT be configured when zerodds is not
#    top-level — even though the parent set BUILD_EXAMPLES=ON. This is the core
#    regression: zerodds keys off ZERODDS_BUILD_EXAMPLES/ZERODDS_BUILD_TESTS
#    (default = PROJECT_IS_TOP_LEVEL), never the generic parent BUILD_* names.
if(TARGET cpp_smoke)
  string(APPEND _fail "  - zerodds example target 'cpp_smoke' was configured (must be off for consumers)\n")
endif()
if(TARGET smoke_dds_psm)
  string(APPEND _fail "  - zerodds test target 'smoke_dds_psm' was configured (must be off for consumers)\n")
endif()

# 5. The renamed options exist; the old generic collision-prone ones are gone.
if(NOT DEFINED ZERODDS_BUILD_CPP)
  string(APPEND _fail "  - ZERODDS_BUILD_CPP not defined (option rename missing)\n")
endif()

if(_fail)
  message(FATAL_ERROR "consumer-regression FAILED:\n${_fail}")
else()
  message(STATUS "consumer-regression PASSED: parent scope intact, zerodds examples/tests not configured")
endif()
