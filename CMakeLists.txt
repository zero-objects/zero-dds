cmake_minimum_required(VERSION 3.21 FATAL_ERROR)
project(zerodds
    VERSION 1.0.0
    DESCRIPTION "ZERO-DDS C C++ bindings"
    LANGUAGES C CXX
)

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
include(CTest) # defines BUILD_TESTING (default ON) + enable_testing()

option(BUILD_SHARED_LIBS "Build static or shared library" OFF)
option(BUILD_ZERODDS_CPP "Enable building zerodds::zerodds-cpp library" ON)
# The `zerodds-idlc` CLI backs `zerodds_idlc_generate()` (cmake/zerodds_idlc_generate.cmake).
# It is normally built lazily on the first call to that function; turning this ON
# builds + installs it unconditionally, so an installed `zerodds` package is
# self-sufficient for a downstream `find_package(zerodds)` consumer that wants to
# run IDL codegen without ever having a Rust toolchain itself.
option(BUILD_ZERODDS_IDLC "Build + install the zerodds-idlc CLI unconditionally" ON)
# Sanitizer for the header-only C++ binding + its own examples/tests (crates/cpp).
# Does NOT sanitize the Rust cdylib/staticlib across the FFI boundary — that is a
# separate, Rust-toolchain-level concern (`cargo +nightly -Zsanitizer=...`).
set(ZERODDS_SANITIZER "none" CACHE STRING "C++ sanitizer: none|address|undefined|thread|address,undefined")
set_property(CACHE ZERODDS_SANITIZER PROPERTY STRINGS none address undefined thread "address,undefined")
# Cargo `--features` forwarded to the zerodds-c-api build (e.g. `security`, `fips`,
# `wolfcrypt`, `delivery-iceoryx` — see crates/zerodds-c-api/Cargo.toml `[features]`).
set(ZERODDS_CARGO_FEATURES "" CACHE STRING "Semicolon-separated Cargo feature list for the zerodds-c-api build")

# Set CMAKE_BUILD_TYPE if neither parent nor project set it or if we are using a multi-config generator
get_property(ZERODDS_GENERATOR_IS_MULTI_CONFIG GLOBAL PROPERTY GENERATOR_IS_MULTI_CONFIG)
if(NOT CMAKE_BUILD_TYPE OR ZERODDS_GENERATOR_IS_MULTI_CONFIG)
  set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type for the library" FORCE)
  if(ZERODDS_GENERATOR_IS_MULTI_CONFIG)
    message(WARNING
      "zerodds: multi-config generator detected (${CMAKE_GENERATOR}). `cargo rustc` builds "
      "exactly one profile per invocation, so the zerodds-c-api / zerodds-idlc Rust "
      "artifacts are always built Release regardless of the IDE-selected configuration "
      "(Debug/RelWithDebInfo/...); only the C/C++ side (crates/cpp, your own targets) "
      "honors the per-config selection."
    )
  endif()
endif()

# Ensure cargo is installed
set(CARGO_EXECUTABLE)
find_program(CARGO_EXECUTABLE NAMES cargo)
if("${CARGO_EXECUTABLE}" STREQUAL "CARGO_EXECUTABLE-NOTFOUND")
  message(FATAL_ERROR "Cargo not found. Look at: https://rust-lang.org/learn/get-started")
endif()

# Determine library extensions and suffixes for the current OS
if(WIN32)
  set(ZERODDS_SHARED_LIBRARY_EXTENSION "dll")
  set(ZERODDS_STATIC_LIBRARY_EXTENSION "lib")
  set(ZERODDS_LIBRARY_PREFIX "")
elseif(APPLE)
  set(ZERODDS_SHARED_LIBRARY_EXTENSION "dylib")
  set(ZERODDS_STATIC_LIBRARY_EXTENSION "a")
  set(ZERODDS_LIBRARY_PREFIX "lib")
else()
  set(ZERODDS_SHARED_LIBRARY_EXTENSION "so")
  set(ZERODDS_STATIC_LIBRARY_EXTENSION "a")
  set(ZERODDS_LIBRARY_PREFIX "lib")
endif()
set(ZERODDS_C_LIBRARY_NAME ${ZERODDS_LIBRARY_PREFIX}${PROJECT_NAME})

# Use the build type to know the folder in which the library will be built
if(${CMAKE_BUILD_TYPE} STREQUAL "Release" OR ${CMAKE_BUILD_TYPE} STREQUAL "Debug")
    string(TOLOWER ${CMAKE_BUILD_TYPE} ZERODDS_BUILD_TYPE)
else()
    set(ZERODDS_BUILD_TYPE "release")
endif()

set(ZERODDS_CMAKE_LIBRARY_TYPE "STATIC")
if(BUILD_SHARED_LIBS)
  set(ZERODDS_CMAKE_LIBRARY_TYPE "SHARED")
endif()

# Set the path to the library binary file depending on the build type
set(ZERODDS_OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/${ZERODDS_BUILD_TYPE}")
if(BUILD_SHARED_LIBS)
  set(ZERODDS_LIBRARY_OUTPUT ${ZERODDS_OUTPUT_DIR}/${ZERODDS_C_LIBRARY_NAME}.${ZERODDS_SHARED_LIBRARY_EXTENSION})
else()
  set(ZERODDS_LIBRARY_OUTPUT ${ZERODDS_OUTPUT_DIR}/${ZERODDS_C_LIBRARY_NAME}.${ZERODDS_STATIC_LIBRARY_EXTENSION})
endif()

# Set location of library headers
set(ZERODDS_C_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/crates/zerodds-c-api/include")
set(ZERODDS_C_HEADERS "${ZERODDS_C_INCLUDE_DIR}/zerodds.h"
                      "${ZERODDS_C_INCLUDE_DIR}/zerodds_xcdr2.h"
)

# Set build flag for cargo only if building in Release
if(${ZERODDS_BUILD_TYPE} STREQUAL "release")
    set(ZERODDS_CARGO_BUILD_FLAG "--${ZERODDS_BUILD_TYPE}")
endif()

# `--features a,b,c` — comma-joined, cargo's own separator.
set(ZERODDS_CARGO_FEATURES_FLAG)
if(ZERODDS_CARGO_FEATURES)
  list(JOIN ZERODDS_CARGO_FEATURES "," ZERODDS_CARGO_FEATURES_JOINED)
  set(ZERODDS_CARGO_FEATURES_FLAG "--features" "${ZERODDS_CARGO_FEATURES_JOINED}")
endif()

# Add a custom target to call cargo for generating the library
add_custom_target(${PROJECT_NAME}_rust ALL
  ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
                      "--package" "zerodds-c-api" "${ZERODDS_CARGO_BUILD_FLAG}"
                      "--crate-type" $<IF:$<BOOL:${BUILD_SHARED_LIBS}>,"cdylib","staticlib">
                      ${ZERODDS_CARGO_FEATURES_FLAG}
                      "--target-dir" "${CMAKE_CURRENT_BINARY_DIR}"
  BYPRODUCTS ${ZERODDS_LIBRARY_OUTPUT}
  DEPENDS ${ZERODDS_C_HEADERS} # CMake will re-run the command whenever the files change
)

# Generate the imported library for ZERODDS and link it to the custom target to ensure the custom command to be called
add_library(${PROJECT_NAME}-c ${ZERODDS_CMAKE_LIBRARY_TYPE} IMPORTED GLOBAL ${ZERODDS_C_HEADERS})
add_library(${PROJECT_NAME}::${PROJECT_NAME}-c ALIAS ${PROJECT_NAME}-c)

if (WIN32)
  # Link Windows libraries
  target_link_libraries(${PROJECT_NAME}-c INTERFACE ntdll ws2_32 bcrypt)
endif()

# Manually specify the dependency between ZERODDS target and the fake one to ensure compiler will not invert the order
add_dependencies(${PROJECT_NAME}-c ${PROJECT_NAME}_rust)

# Set imported target library location and implib.
# On Windows if the library is static then both properties
# must be the same .lib file. On Linux the 'IMPORTED_IMPLIB' will be ignored
set_target_properties(
  ${PROJECT_NAME}-c
  PROPERTIES IMPORTED_LOCATION ${ZERODDS_LIBRARY_OUTPUT}
             IMPORTED_IMPLIB ${ZERODDS_LIBRARY_OUTPUT}.${ZERODDS_STATIC_LIBRARY_EXTENSION}
)

target_include_directories(${PROJECT_NAME}-c INTERFACE
  "${ZERODDS_C_INCLUDE_DIR}"
)

# Expose also zerodds_cxx target
if(BUILD_ZERODDS_CPP)
  add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/crates/cpp")
endif()

# Export zerodds_idlc_generate function
include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds_idlc_generate.cmake")

# ---------------------------------------------------------------------------
# Installation (find_package(zerodds) config mode)
# ---------------------------------------------------------------------------
# The C/C++ artifacts are built by `cargo rustc`, not CMake's own compiler
# driver, so `zerodds-c` stays an IMPORTED target end to end — it is installed
# as plain files (library + headers) and re-imported by zeroddsConfig.cmake on
# the consumer side, rather than through install(TARGETS ... EXPORT ...)
# (which requires a non-IMPORTED target).
# ZERODDS_LIBRARY_OUTPUT already resolves to the right filename for the
# selected crate-type (cargo builds exactly one of staticlib/cdylib per
# BUILD_SHARED_LIBS, never both) — a single, un-renamed install() rule.
install(FILES ${ZERODDS_LIBRARY_OUTPUT}
  DESTINATION ${CMAKE_INSTALL_LIBDIR}
)
install(FILES ${ZERODDS_C_HEADERS}
  DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
)
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds_idlc_generate.cmake"
  DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
)

if(BUILD_ZERODDS_IDLC)
  if(WIN32)
    set(ZERODDS_IDLC_EXE_EXTENSION ".exe")
  endif()
  set(ZERODDS_IDLC_OUTPUT "${ZERODDS_OUTPUT_DIR}/zerodds-idlc${ZERODDS_IDLC_EXE_EXTENSION}")
  add_custom_target(${PROJECT_NAME}_idlc ALL
    ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
                        "--package" "zerodds-idlc" "${ZERODDS_CARGO_BUILD_FLAG}"
                        "--target-dir" "${CMAKE_CURRENT_BINARY_DIR}"
    BYPRODUCTS ${ZERODDS_IDLC_OUTPUT}
  )
  add_executable(${PROJECT_NAME}::${PROJECT_NAME}-idlc IMPORTED GLOBAL)
  add_dependencies(${PROJECT_NAME}::${PROJECT_NAME}-idlc ${PROJECT_NAME}_idlc)
  set_target_properties(${PROJECT_NAME}::${PROJECT_NAME}-idlc PROPERTIES
    IMPORTED_LOCATION ${ZERODDS_IDLC_OUTPUT}
  )
  install(PROGRAMS ${ZERODDS_IDLC_OUTPUT}
    DESTINATION ${CMAKE_INSTALL_BINDIR}
    RENAME "zerodds-idlc${ZERODDS_IDLC_EXE_EXTENSION}"
  )
endif()

write_basic_package_version_file(
  "${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake"
  VERSION ${PROJECT_VERSION}
  COMPATIBILITY SameMajorVersion
)
configure_package_config_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/cmake/zeroddsConfig.cmake.in"
  "${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake"
  INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
  PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR CMAKE_INSTALL_BINDIR
)
install(FILES
  "${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake"
  "${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake"
  DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
)
