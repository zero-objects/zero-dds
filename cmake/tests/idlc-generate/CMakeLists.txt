# Generator test for zerodds_idlc_generate() against the IDLC build contract.
#
# Exercises the stamp/depfile/manifest wiring end to end with a real cmake +
# ninja build: robot.idl #includes common.idl, so a change to common.idl must
# retrigger generation via the DEPFILE (the core regression the old
# extension-guessing generator missed).
#
# The host compiler `zerodds::zerodds-idlc` is pre-defined as an IMPORTED target
# pointing at a prebuilt binary (-DZERODDS_IDLC_EXE=...), so the generator does
# NOT invoke cargo and this mini-project stays independent of the full workspace.
#
# Driven by cmake/tests/run-idlc-generate.sh.
cmake_minimum_required(VERSION 3.21 FATAL_ERROR)
project(zerodds_idlc_generate_test LANGUAGES CXX)

if(NOT ZERODDS_IDLC_EXE)
  message(FATAL_ERROR "pass -DZERODDS_IDLC_EXE=<path to a prebuilt zerodds-idlc>")
endif()

# Pre-define the host tool as an imported executable so zerodds_idlc_generate()
# skips its cargo-build branch entirely.
add_executable(zerodds::zerodds-idlc IMPORTED GLOBAL)
set_target_properties(zerodds::zerodds-idlc PROPERTIES IMPORTED_LOCATION "${ZERODDS_IDLC_EXE}")

# Stub runtime target the cpp INTERFACE library links against. Header-only stub;
# the test never compiles generated headers, so it needs no real runtime.
add_library(zerodds::zerodds-cpp INTERFACE IMPORTED)

# Location of the generator module and of the IDL sources are overridable so the
# driver can point IDL_DIR at a scratch copy (safe to `touch`) while keeping the
# module resolved from the real repo checkout.
if(NOT ZERODDS_CMAKE_DIR)
  set(ZERODDS_CMAKE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../..")
endif()
if(NOT IDL_DIR)
  set(IDL_DIR "${CMAKE_CURRENT_SOURCE_DIR}/idl")
endif()

include("${ZERODDS_CMAKE_DIR}/zerodds_idlc_generate.cmake")

zerodds_idlc_generate(
  robot_idl
  IDL_FILES "${IDL_DIR}/robot.idl"
  GENERATOR_BACKEND cpp
  IDL_INCLUDE_DIRS "${IDL_DIR}"
  OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated"
)

# Concrete target that links the generated library, forcing generation to run
# before it builds.
add_executable(app main.cpp)
target_link_libraries(app PRIVATE robot_idl)
