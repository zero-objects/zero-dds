cmake_minimum_required(VERSION 3.21 FATAL_ERROR)

# Single source of truth for the version = the Cargo workspace manifest
# ([workspace.package] version). CMake's project(VERSION) accepts only a numeric
# X.Y.Z[.tweak] core, so the full pre-release identifier (e.g. 1.0.0-rc.7) is
# read into ZERODDS_VERSION_FULL and threaded into the pkg-config file / packages
# below, while project() itself receives the numeric core (1.0.0). This keeps the
# C/C++ SDK version in lockstep with the Rust workspace with no hand-maintained
# duplicate.
file(READ "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" _zerodds_cargo_manifest)
string(REGEX MATCH "\n\\[workspace\\.package\\][^\n]*\n([^\\[]*)" _zerodds_wp_match "${_zerodds_cargo_manifest}")
string(REGEX MATCH "version[ \t]*=[ \t]*\"([^\"]+)\"" _zerodds_ver_match "${CMAKE_MATCH_1}")
set(ZERODDS_VERSION_FULL "${CMAKE_MATCH_1}")
if(NOT ZERODDS_VERSION_FULL)
  message(FATAL_ERROR "Failed to read [workspace.package] version from Cargo.toml")
endif()
string(REGEX REPLACE "^([0-9]+\\.[0-9]+\\.[0-9]+).*$" "\\1" ZERODDS_VERSION_CORE "${ZERODDS_VERSION_FULL}")

project(zerodds
    VERSION ${ZERODDS_VERSION_CORE}
    DESCRIPTION "ZERO-DDS C C++ bindings"
    LANGUAGES C CXX
)

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
# CTest (defines BUILD_TESTING (default ON) + enable_testing()) is only pulled
# in when zerodds is the top-level project. Consuming zerodds via
# add_subdirectory()/FetchContent must NOT inject the CTest dashboard machinery
# or a BUILD_TESTING cache entry into the parent build.
if(PROJECT_IS_TOP_LEVEL)
  include(CTest)
endif()

# BUILD_SHARED_LIBS is the canonical CMake variable governing static-vs-shared;
# it is intentionally kept (not renamed) so a parent project's choice flows
# through unchanged. It is the only non-ZERODDS_*-prefixed option here.
option(BUILD_SHARED_LIBS "Build zerodds as a shared library instead of static" OFF)

# --- Backward-compat aliases for the pre-ZERODDS_* option names (deprecated) ---
# Only the zerodds-namespaced BUILD_ZERODDS_* names get aliased; the generic
# BUILD_EXAMPLES/BUILD_TESTS (crates/cpp) are deliberately dropped, not aliased,
# because their whole problem was colliding with identically named parent options.
if(DEFINED BUILD_ZERODDS_CPP)
  message(DEPRECATION "BUILD_ZERODDS_CPP is deprecated; use ZERODDS_BUILD_CPP instead.")
  set(ZERODDS_BUILD_CPP "${BUILD_ZERODDS_CPP}")
endif()
if(DEFINED BUILD_ZERODDS_IDLC)
  message(DEPRECATION "BUILD_ZERODDS_IDLC is deprecated; use ZERODDS_BUILD_IDLC instead.")
  set(ZERODDS_BUILD_IDLC "${BUILD_ZERODDS_IDLC}")
endif()

option(ZERODDS_BUILD_CPP "Enable building zerodds::zerodds-cpp library" ON)
# The `zerodds-idlc` CLI backs `zerodds_idlc_generate()` (cmake/zerodds_idlc_generate.cmake).
# It is normally built lazily on the first call to that function; turning this ON
# builds + installs it unconditionally, so an installed `zerodds` package is
# self-sufficient for a downstream `find_package(zerodds)` consumer that wants to
# run IDL codegen without ever having a Rust toolchain itself.
option(ZERODDS_BUILD_IDLC "Build + install the zerodds-idlc CLI unconditionally" ON)

# Examples/tests are developer artifacts: default ON only when zerodds is the
# top-level project, OFF when consumed via add_subdirectory()/FetchContent.
# Defined here in root scope, where PROJECT_IS_TOP_LEVEL reflects zerodds
# itself (in crates/cpp it never would, since that is always a subdirectory of
# this project); crates/cpp/CMakeLists.txt reads these cache entries.
option(ZERODDS_BUILD_EXAMPLES "Build zerodds C++ examples" ${PROJECT_IS_TOP_LEVEL})
option(ZERODDS_BUILD_TESTS "Build zerodds C++ tests" ${PROJECT_IS_TOP_LEVEL})
# Sanitizer for the header-only C++ binding + its own examples/tests (crates/cpp).
# Does NOT sanitize the Rust cdylib/staticlib across the FFI boundary — that is a
# separate, Rust-toolchain-level concern (`cargo +nightly -Zsanitizer=...`).
set(ZERODDS_SANITIZER "none" CACHE STRING "C++ sanitizer: none|address|undefined|thread|address,undefined")
set_property(CACHE ZERODDS_SANITIZER PROPERTY STRINGS none address undefined thread "address,undefined")
# Cargo `--features` forwarded to the zerodds-c-api build (e.g. `security`, `fips`,
# `wolfcrypt`, `delivery-iceoryx` — see crates/zerodds-c-api/Cargo.toml `[features]`).
set(ZERODDS_CARGO_FEATURES "" CACHE STRING "Semicolon-separated Cargo feature list for the zerodds-c-api build")

# Provide a sensible default build type, but ONLY when zerodds is the top-level
# project, the generator is single-config, and no build type was chosen.
# The guard on PROJECT_IS_TOP_LEVEL is what makes this safe: a consumer that
# adds zerodds via add_subdirectory()/FetchContent never enters this branch, so
# its CMAKE_BUILD_TYPE is never touched (see cmake/tests/consumer-regression).
# FORCE is required because project()/enable_language seeds an *empty*
# CMAKE_BUILD_TYPE cache entry on single-config generators, which a plain
# `set(... CACHE ...)` would refuse to overwrite.
get_property(ZERODDS_GENERATOR_IS_MULTI_CONFIG GLOBAL PROPERTY GENERATOR_IS_MULTI_CONFIG)
if(PROJECT_IS_TOP_LEVEL AND NOT CMAKE_BUILD_TYPE AND NOT ZERODDS_GENERATOR_IS_MULTI_CONFIG)
  set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type for the library" FORCE)
  set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS Debug Release RelWithDebInfo MinSizeRel)
endif()
if(ZERODDS_GENERATOR_IS_MULTI_CONFIG)
  message(WARNING
    "zerodds: multi-config generator detected (${CMAKE_GENERATOR}). `cargo rustc` builds "
    "exactly one profile per invocation, so the zerodds-c-api / zerodds-idlc Rust "
    "artifacts are always built Release regardless of the IDE-selected configuration "
    "(Debug/RelWithDebInfo/...); only the C/C++ side (crates/cpp, your own targets) "
    "honors the per-config selection."
  )
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

# Derive the cargo profile from the CMake config: Debug -> cargo dev profile
# (target/debug); everything else (Release/RelWithDebInfo/MinSizeRel, or an
# unset CMAKE_BUILD_TYPE as seen for consumers/multi-config) -> cargo release
# profile (target/release). `cargo rustc` builds exactly one profile per
# invocation, so this is a single per-configure mapping rather than a
# per-config matrix (true multi-config-per-build remains a separate concern).
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(ZERODDS_BUILD_TYPE "debug")
else()
    set(ZERODDS_BUILD_TYPE "release")
endif()

# ---------------------------------------------------------------------------
# Host tool vs. target library — the two-path split (spec P0-2 / decision #6).
# ---------------------------------------------------------------------------
# CROSS-COMPILE FACT: `libzerodds` (the zerodds-c-api cdylib/staticlib) is code
# that runs ON the target (ARM/Yocto/QNX/SDK), so it must be built FOR the target
# triple. `zerodds-idlc` is a *build-time code generator* invoked by CMake during
# THIS build (cmake/zerodds_idlc_generate.cmake) — a target-arch binary cannot be
# executed on the build host, so it must always be built FOR the host triple.
# `--target <target-triple>` for BOTH would break the generator; no `--target`
# for both breaks cross for the library. Hence the split below.
#
# CORROSION EVALUATION (decision #6(a)): corrosion-rs is the established
# Cargo↔CMake bridge and does handle target triples + per-config profiles for
# `corrosion_import_crate`d target libraries. It is NOT adopted here, for four
# concrete reasons, each a regression risk against contracts that JUST landed:
#   1. Install contract (P0-4, 8e8a1601): the SINGLE `cmake --install` staging
#      tree installs `zerodds-c` as PLAIN FILES precisely because it is an
#      IMPORTED target (cargo rustc output, not install(TARGETS EXPORT)).
#      Corrosion produces its own managed targets; its install story is the
#      experimental `corrosion_install()`, which lays artifacts out differently
#      and would force a rewrite of the install() block + zeroddsConfig.cmake.in.
#   2. Host tool: Corrosion builds every imported crate for the ONE CMake target
#      triple (Rust_CARGO_TARGET). It has no first-class "build THIS crate for
#      the host while cross-compiling the rest" mode — a host tool still needs a
#      separate host cargo invocation, i.e. exactly the code below, Corrosion or
#      not.
#   3. Network: Corrosion is normally pulled via FetchContent from GitHub; the
#      offline/local SDK gate cannot rely on that, and vendoring it is a larger
#      footprint change than this split.
#   4. Its headline advantage (profile-per-config on multi-config generators) is
#      already out of scope here: `cargo rustc` builds one profile per
#      invocation (see the multi-config WARNING above), so Corrosion buys nothing
#      the project needs today.
# => hand-rolled host/target split, preserving the P0-3/P0-4/P0-5 contracts.
#
# The host triple is read from `rustc -vV`; it is what `zerodds-idlc` is pinned
# to so an ambient CARGO_BUILD_TARGET (Yocto/QNX SDK environments export it)
# cannot silently cross-build the generator into an unrunnable target-arch
# binary. Overridable via the ZERODDS_CARGO_HOST_TARGET cache entry.
find_program(RUSTC_EXECUTABLE NAMES rustc)
set(ZERODDS_DETECTED_HOST_TARGET "")
if(RUSTC_EXECUTABLE)
  execute_process(
    COMMAND "${RUSTC_EXECUTABLE}" -vV
    OUTPUT_VARIABLE ZERODDS_RUSTC_VERSION_VERBOSE
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET
  )
  string(REGEX MATCH "host: ([^\n\r]+)" ZERODDS_HOST_MATCH "${ZERODDS_RUSTC_VERSION_VERBOSE}")
  if(ZERODDS_HOST_MATCH)
    set(ZERODDS_DETECTED_HOST_TARGET "${CMAKE_MATCH_1}")
  endif()
endif()
set(ZERODDS_CARGO_HOST_TARGET "${ZERODDS_DETECTED_HOST_TARGET}" CACHE STRING
    "Rust host target triple the host-side zerodds-idlc generator is built for")

# TARGET triple for the cross-compiled libzerodds. Empty = native host build
# (the default, so a plain native configure is byte-for-byte what it was before
# this split). Seeded from cargo's own CARGO_BUILD_TARGET convention, which
# cross toolchains (Yocto SDK, cross, cargo config) already export; an explicit
# -DZERODDS_CARGO_TARGET=... always wins. A CMAKE_TOOLCHAIN_FILE selecting a
# cross C/C++ compiler should set this to the matching Rust triple.
set(ZERODDS_CARGO_TARGET "$ENV{CARGO_BUILD_TARGET}" CACHE STRING
    "Rust target triple for the cross-compiled libzerodds (empty = native host build)")

# Cross iff a target triple is chosen that differs from the host triple.
set(ZERODDS_IS_CROSS FALSE)
if(ZERODDS_CARGO_TARGET AND NOT ZERODDS_CARGO_TARGET STREQUAL ZERODDS_CARGO_HOST_TARGET)
  set(ZERODDS_IS_CROSS TRUE)
endif()

# --- Target-library path: pass --target only when a triple is set. cargo nests
# artifacts under <target-dir>/<triple>/<profile> as soon as --target is given,
# so the output subdir must gain the triple component to match.
set(ZERODDS_CARGO_TARGET_FLAG "")
set(ZERODDS_LIB_TRIPLE_SUBDIR "")
if(ZERODDS_CARGO_TARGET)
  set(ZERODDS_CARGO_TARGET_FLAG "--target" "${ZERODDS_CARGO_TARGET}")
  set(ZERODDS_LIB_TRIPLE_SUBDIR "${ZERODDS_CARGO_TARGET}/")
endif()

# --- Host-tool path: the generator is pinned to the host triple ONLY when a
# cross is in play (an explicit cross target, or an ambient CARGO_BUILD_TARGET
# that would otherwise leak into the generator's build). In the pure-native
# default neither flag nor subdir is added, so the existing SDK/consumer gates
# see the identical layout they did before P0-2.
set(ZERODDS_IDLC_HOST_FLAG "")
set(ZERODDS_IDLC_HOST_TRIPLE_SUBDIR "")
if(ZERODDS_CARGO_HOST_TARGET AND (ZERODDS_IS_CROSS OR DEFINED ENV{CARGO_BUILD_TARGET}))
  set(ZERODDS_IDLC_HOST_FLAG "--target" "${ZERODDS_CARGO_HOST_TARGET}")
  set(ZERODDS_IDLC_HOST_TRIPLE_SUBDIR "${ZERODDS_CARGO_HOST_TARGET}/")
endif()

if(ZERODDS_IS_CROSS)
  message(STATUS
    "zerodds: cross build — libzerodds -> target '${ZERODDS_CARGO_TARGET}', "
    "zerodds-idlc -> host '${ZERODDS_CARGO_HOST_TARGET}' (generator stays host-runnable)."
  )
endif()

set(ZERODDS_CMAKE_LIBRARY_TYPE "STATIC")
if(BUILD_SHARED_LIBS)
  set(ZERODDS_CMAKE_LIBRARY_TYPE "SHARED")
endif()

# Set the path to the library binary file depending on the build type. Under a
# cross build the triple subdir (ZERODDS_LIB_TRIPLE_SUBDIR) is where cargo puts
# the target artifacts; native builds leave it empty (unchanged path).
set(ZERODDS_OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/${ZERODDS_LIB_TRIPLE_SUBDIR}${ZERODDS_BUILD_TYPE}")
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

# Set build flag for cargo only if building in Release. Kept as an empty list
# (not an empty string) so the unquoted expansion in the custom targets below
# adds zero arguments for the debug/dev profile instead of a bogus "" arg.
set(ZERODDS_CARGO_BUILD_FLAG "")
if(ZERODDS_BUILD_TYPE STREQUAL "release")
    set(ZERODDS_CARGO_BUILD_FLAG "--release")
endif()

# `--features a,b,c` — comma-joined, cargo's own separator.
set(ZERODDS_CARGO_FEATURES_FLAG)
if(ZERODDS_CARGO_FEATURES)
  list(JOIN ZERODDS_CARGO_FEATURES "," ZERODDS_CARGO_FEATURES_JOINED)
  set(ZERODDS_CARGO_FEATURES_FLAG "--features" "${ZERODDS_CARGO_FEATURES_JOINED}")
endif()

# Add a custom target to call cargo for generating the library. This is the
# TARGET path: ZERODDS_CARGO_TARGET_FLAG carries `--target <triple>` on a cross
# build (empty on native), so libzerodds is built for the target architecture.
add_custom_target(${PROJECT_NAME}_rust ALL
  ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
                      "--package" "zerodds-c-api" ${ZERODDS_CARGO_BUILD_FLAG}
                      ${ZERODDS_CARGO_TARGET_FLAG}
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
if(ZERODDS_BUILD_CPP)
  add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/crates/cpp")
endif()

# Export zerodds_idlc_generate function
include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds_idlc_generate.cmake")

# ---------------------------------------------------------------------------
# Installation — the SINGLE SDK install staging tree.
# ---------------------------------------------------------------------------
# find_package(zerodds) config mode. This install() block is the one and only
# place that lays down everything a C/C++ consumer needs; ALL downstream
# packages (DEB/RPM/Homebrew/MSI/SDK-tarball) MUST consume the output of
# `cmake --install` (a DESTDIR/prefix staging tree) rather than hand-copying
# individual files — that is what keeps a header from ever being "forgotten"
# in one packaging path but not another (spec P0-4). What lands here:
#   - C headers        (zerodds.h + zerodds_xcdr2.h)         -> include/
#   - C++ headers      (dds/**, zerodds/**)                  -> include/  (crates/cpp)
#   - the library      (shared + import lib, or static)      -> lib/ (+ bin/ on Win)
#   - zeroddsConfig.cmake + zeroddsConfigVersion.cmake       -> lib/cmake/zerodds/
#   - the IDLC generator CMake module                        -> lib/cmake/zerodds/
#   - zerodds-idlc host tool (ZERODDS_BUILD_IDLC)            -> bin/
#   - zerodds.pc pkg-config descriptor                       -> lib/pkgconfig/
#
# The C/C++ artifacts are built by `cargo rustc`, not CMake's own compiler
# driver, so `zerodds-c` stays an IMPORTED target end to end — it is installed
# as plain files (library + headers) and re-imported by zeroddsConfig.cmake on
# the consumer side, rather than through install(TARGETS ... EXPORT ...)
# (which requires a non-IMPORTED target).
#
# Windows shared build (P0-5): a cdylib produces TWO link-relevant files — the
# runtime DLL and the import library (`zerodds.dll.lib`). They go to different
# GNU-install dirs: DLL -> bin/, import lib -> lib/ (renamed to `zerodds.lib`,
# the stable name zeroddsConfig.cmake looks for), plus the PDB -> lib/ when
# present. Everywhere else (Unix shared, or any static build) the one file
# ZERODDS_LIBRARY_OUTPUT resolves to goes to lib/ unchanged.
if(WIN32 AND BUILD_SHARED_LIBS)
  install(FILES ${ZERODDS_LIBRARY_OUTPUT}
    DESTINATION ${CMAKE_INSTALL_BINDIR}
  )
  install(FILES "${ZERODDS_LIBRARY_OUTPUT}.${ZERODDS_STATIC_LIBRARY_EXTENSION}"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RENAME "${ZERODDS_C_LIBRARY_NAME}.${ZERODDS_STATIC_LIBRARY_EXTENSION}"
  )
  # rustc emits the debug symbols as <crate>.pdb next to the DLL; ship it if
  # the profile produced one (OPTIONAL: release-without-debuginfo has none).
  install(FILES "${ZERODDS_OUTPUT_DIR}/${ZERODDS_C_LIBRARY_NAME}.pdb"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}
    OPTIONAL
  )
else()
  install(FILES ${ZERODDS_LIBRARY_OUTPUT}
    DESTINATION ${CMAKE_INSTALL_LIBDIR}
  )
endif()
install(FILES ${ZERODDS_C_HEADERS}
  DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
)
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds_idlc_generate.cmake"
  DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
)

# pkg-config descriptor — generated from cmake/zerodds.pc.in into this same
# staging tree so the non-CMake link path (`pkg-config --cflags --libs
# zerodds`) sees the identical prefix/libdir/includedir. Relocatable via
# ${pcfiledir}; @CMAKE_INSTALL_LIBDIR@/@CMAKE_INSTALL_INCLUDEDIR@ resolve at
# configure time.
configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds.pc.in"
  "${CMAKE_CURRENT_BINARY_DIR}/zerodds.pc"
  @ONLY
)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/zerodds.pc"
  DESTINATION ${CMAKE_INSTALL_LIBDIR}/pkgconfig
)

if(ZERODDS_BUILD_IDLC)
  if(WIN32)
    set(ZERODDS_IDLC_EXE_EXTENSION ".exe")
  endif()
  # HOST path: zerodds-idlc is a build-time generator, so it is built for the
  # host triple (ZERODDS_IDLC_HOST_FLAG pins it under cross builds). Its output
  # lives under the host-triple subdir, distinct from the target library's.
  set(ZERODDS_IDLC_OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/${ZERODDS_IDLC_HOST_TRIPLE_SUBDIR}${ZERODDS_BUILD_TYPE}")
  set(ZERODDS_IDLC_OUTPUT "${ZERODDS_IDLC_OUTPUT_DIR}/zerodds-idlc${ZERODDS_IDLC_EXE_EXTENSION}")
  add_custom_target(${PROJECT_NAME}_idlc ALL
    ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
                        "--package" "zerodds-idlc" ${ZERODDS_CARGO_BUILD_FLAG}
                        ${ZERODDS_IDLC_HOST_FLAG}
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
