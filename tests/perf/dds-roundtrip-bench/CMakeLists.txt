# SPDX-License-Identifier: Apache-2.0
#
# Cross-target roundtrip bench build.
#
# Builds one binary per vendor from the shared roundtrip.idl + the
# per-vendor apps (cyclone_app.cpp / fastdds_app.cpp / rti_app.cpp /
# zerodds_app.cpp). Apples-to-apples: same IDL → same wire
# format. Each vendor is built optionally (skipped if the SDK cannot
# be found).
#
# Vendor paths can be overridden with -D (default = /opt layout; adjust per host):
#   -DZERODDS_REPO=/path/to/zerodds      (default: ../../..)
#   -DCYCLONE_ROOT=/opt/cyclone
#   -DFASTDDS_ROOT=/opt/fastdds
#   -DRTI_HOME=/opt/rti.com/rti_connext_dds-7.7.0
#   -DOPENDDS_ROOT=/opt/opendds          (or /opt/opendds-secure for DDS-Security)
#
# Lib-Suffix auto-detect (.so vs .dylib). RTI-arch-dir auto-detect via
# glob (lib/{x64Linux4gcc8.5.0, arm64Darwin23clang16.0, ...}).
#
# Usage:
#   mkdir build && cd build
#   cmake [-Dvars] ..
#   cmake --build .
#   # yields {cyclone,fastdds,rti,zerodds,opendds}-roundtrip (where SDKs
#   # were found).

cmake_minimum_required(VERSION 3.16)
project(dds-roundtrip-bench CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
add_compile_options(-O2 -Wall -Wextra)

set(BENCH_DIR ${CMAKE_CURRENT_SOURCE_DIR})

# Default ZEROCDDS_REPO = .../zerodds (relative to our position).
if(NOT DEFINED ZERODDS_REPO)
    get_filename_component(ZERODDS_REPO "${BENCH_DIR}/../../.." ABSOLUTE)
endif()

# The C++ wire runtime (`runtime/cpp`) — the one codec `gen/zerodds/
# Roundtrip.hpp` / `RoundtripRich.hpp` (`zerodds-idlc generate --lang c++ --cpp-mapping native`
# output) call into. `zerodds::zerodds-wire-cpp` below is its CMake target.
add_subdirectory("${ZERODDS_REPO}/runtime/cpp" "${CMAKE_CURRENT_BINARY_DIR}/zerodds-wire-cpp")
# Default vendor roots — can be overridden via -D.
if(NOT DEFINED CYCLONE_ROOT)
    set(CYCLONE_ROOT /opt/cyclone)
endif()
if(NOT DEFINED FASTDDS_ROOT)
    set(FASTDDS_ROOT /opt/fastdds)
endif()
if(NOT DEFINED RTI_HOME)
    set(RTI_HOME /opt/rti.com/rti_connext_dds-7.7.0)
endif()
if(NOT DEFINED OPENDDS_ROOT)
    set(OPENDDS_ROOT /opt/opendds)
endif()

# ============================================================
# zerodds-roundtrip — C-FFI + gen/zerodds/Roundtrip.hpp
# ============================================================
# Lib suffix: .dylib on macOS, .so otherwise.
if(APPLE)
    set(ZERODDS_LIB ${ZERODDS_REPO}/target/release/libzerodds.dylib)
else()
    set(ZERODDS_LIB ${ZERODDS_REPO}/target/release/libzerodds.so)
endif()
set(ZERODDS_HDR ${ZERODDS_REPO}/crates/zerodds-c-api/include/zerodds.h)
if(EXISTS ${ZERODDS_LIB} AND EXISTS ${ZERODDS_HDR})
    message(STATUS "Found ZeroDDS: ${ZERODDS_LIB}")
    add_executable(zerodds-roundtrip zerodds_app.cpp)
    target_include_directories(zerodds-roundtrip PRIVATE
        ${ZERODDS_REPO}/crates/zerodds-c-api/include
        ${BENCH_DIR}/gen/zerodds
        ${ZERODDS_REPO}/crates/cpp/include
    )
    target_link_libraries(zerodds-roundtrip PRIVATE
        ${ZERODDS_LIB}
        zerodds::zerodds-wire-cpp
        pthread dl m
    )
    # The same app with the XCDR1 codec (gen/zerodds-xcdr1, generated with
    # `--encoding xcdr1`) and an XCDR1 offer, for the pairs with OpenSplice,
    # which writes and reads XCDR1 only. matrix_sweep.sh picks it for those.
    add_executable(zerodds-xcdr1-roundtrip zerodds_app.cpp)
    target_compile_definitions(zerodds-xcdr1-roundtrip PRIVATE ZERODDS_BENCH_XCDR1)
    target_include_directories(zerodds-xcdr1-roundtrip PRIVATE
        ${ZERODDS_REPO}/crates/zerodds-c-api/include
        ${BENCH_DIR}/gen/zerodds-xcdr1
        ${ZERODDS_REPO}/crates/cpp/include
    )
    target_link_libraries(zerodds-xcdr1-roundtrip PRIVATE
        ${ZERODDS_LIB}
        zerodds::zerodds-wire-cpp
        pthread dl m
    )
else()
    # zerodds_app.cpp links the zerodds_security_* functions (ZERODDS_BENCH_SECURITY
    # switches them on at run time), which libzerodds exports only with `security`.
    message(WARNING "ZeroDDS not built: cargo build --release -p zerodds-c-api --features security")
endif()

# ============================================================
# cyclone-roundtrip — needs cyclonedds-cxx (libddscxx + libddsc) + idlc
# ============================================================
find_path(CYCLONE_CXX_INCLUDE dds/dds.hpp
    PATHS ${CYCLONE_ROOT}/include ${CYCLONE_ROOT}/include/ddscxx
    NO_DEFAULT_PATH)
find_library(CYCLONE_CXX_LIB ddscxx PATHS ${CYCLONE_ROOT}/lib NO_DEFAULT_PATH)
find_library(CYCLONE_C_LIB   ddsc   PATHS ${CYCLONE_ROOT}/lib NO_DEFAULT_PATH)
find_program(CYCLONE_IDLC    idlc   PATHS ${CYCLONE_ROOT}/bin NO_DEFAULT_PATH)

if(CYCLONE_CXX_INCLUDE AND CYCLONE_CXX_LIB AND CYCLONE_IDLC)
    message(STATUS "Found Cyclone DDS C++: ${CYCLONE_CXX_LIB}")
    # Generate roundtrip.cpp + roundtrip.hpp via Cyclone's idlc -l cxx.
    set(CY_GEN_DIR ${CMAKE_CURRENT_BINARY_DIR}/cyclone-gen)
    file(MAKE_DIRECTORY ${CY_GEN_DIR})
    add_custom_command(
        OUTPUT ${CY_GEN_DIR}/roundtrip.hpp ${CY_GEN_DIR}/roundtrip.cpp
        COMMAND ${CYCLONE_IDLC} -l cxx -o ${CY_GEN_DIR} ${BENCH_DIR}/roundtrip.idl
        DEPENDS ${BENCH_DIR}/roundtrip.idl
        WORKING_DIRECTORY ${CY_GEN_DIR}
        COMMENT "cyclone: idlc -l cxx roundtrip.idl"
    )
    add_executable(cyclone-roundtrip
        cyclone_app.cpp
        ${CY_GEN_DIR}/roundtrip.cpp
    )
    target_include_directories(cyclone-roundtrip PRIVATE
        ${CYCLONE_CXX_INCLUDE}
        ${CYCLONE_ROOT}/include  # for dds/dds.h transitive
        ${CY_GEN_DIR}
    )
    target_link_libraries(cyclone-roundtrip PRIVATE
        ${CYCLONE_CXX_LIB} ${CYCLONE_C_LIB} pthread
    )
else()
    message(WARNING "Cyclone DDS C++ not found (libddscxx + idlc -l cxx in /opt/cyclone)")
endif()

# ============================================================
# fastdds-roundtrip — needs fastrtps + fastcdr + fastddsgen
# ============================================================
find_path(FASTDDS_INCLUDE fastdds/dds/domain/DomainParticipantFactory.hpp
    PATHS ${FASTDDS_ROOT}/include NO_DEFAULT_PATH)
# The Fast-DDS v3.x lib is called "fastdds" (legacy v2.x: "fastrtps"). Try
# both — finding either name is enough, even though the cmake doc string
# says the first.
find_library(FASTDDS_LIB
    NAMES fastdds fastrtps
    PATHS ${FASTDDS_ROOT}/lib NO_DEFAULT_PATH)
# fastcdr comes from the source build (built alongside fast-dds).
find_library(FASTCDR_LIB fastcdr
    PATHS ${FASTDDS_ROOT}/lib /usr/lib/x86_64-linux-gnu)
# fastddsgen v4.x: scripts/fastddsgen wrapper; v3.x bin/fastddsgen.
find_program(FASTDDS_GEN fastddsgen
    PATHS ${FASTDDS_ROOT}/bin ${FASTDDS_ROOT}/../fastddsgen/scripts
          $ENV{HOME}/vendors/fastddsgen/scripts)

if(FASTDDS_INCLUDE AND FASTDDS_LIB AND FASTDDS_GEN)
    message(STATUS "Found Fast-DDS: ${FASTDDS_LIB}")
    set(FD_GEN_DIR ${CMAKE_CURRENT_BINARY_DIR}/fastdds-gen)
    file(MAKE_DIRECTORY ${FD_GEN_DIR})
    # fastddsgen v4.x generates:
    #   roundtrip.hpp                          (struct definition, header-only)
    #   roundtripCdrAux.{hpp,ipp}              (CDR Aux header-only)
    #   roundtripPubSubTypes.{hpp,cxx}         (TypeSupport)
    # `-no-typeobjectsupport`: by default fastddsgen v4 generates
    # a TypeObject (roundtripTypeObjectSupport.*) that encodes an all-zero
    # PLAIN_SEQUENCE_LARGE for `sequence<octet, 8192>` —
    # Cyclone DDS rejects it as "invalid type object"
    # ("ddsi_xt_type_add_typeobj with invalid type object" in the pong log).
    # Without a TypeObject the SEDP match falls back to type name +
    # ALLOW_TYPE_COERCION → cyclone↔fastdds match.
    add_custom_command(
        OUTPUT ${FD_GEN_DIR}/roundtrip.hpp
            ${FD_GEN_DIR}/roundtripPubSubTypes.cxx
        COMMAND ${FASTDDS_GEN} -no-typeobjectsupport
            -d ${FD_GEN_DIR} -replace ${BENCH_DIR}/roundtrip.idl
        DEPENDS ${BENCH_DIR}/roundtrip.idl
        COMMENT "fastdds: fastddsgen -no-typeobjectsupport roundtrip.idl"
    )
    add_executable(fastdds-roundtrip
        fastdds_app.cpp
        ${FD_GEN_DIR}/roundtripPubSubTypes.cxx
    )
    target_include_directories(fastdds-roundtrip PRIVATE
        ${FASTDDS_INCLUDE}
        ${FD_GEN_DIR}
    )
    target_link_libraries(fastdds-roundtrip PRIVATE
        ${FASTDDS_LIB} ${FASTCDR_LIB} pthread
    )
else()
    message(WARNING "Fast-DDS not found (libfastrtps + fastddsgen)")
endif()

# ============================================================
# rti-roundtrip — needs NDDSHOME with rtiddsgen
# ============================================================
# RTI 7.7.0 — the Express/Free licence (CONNEXTPRO + TrialEndLimits)
# is in the 7.5.0+ format; RTI 7.3.0 could not read it ("feature
# license not found"). The arch dir varies per platform:
#   Linux x64:    lib/x64Linux4gcc8.5.0
#   macOS arm64:  lib/arm64Darwin23clang16.0
# Glob both.
find_program(RTI_DDSGEN rtiddsgen PATHS ${RTI_HOME}/bin NO_DEFAULT_PATH)
file(GLOB RTI_ARCH_DIRS LIST_DIRECTORIES true ${RTI_HOME}/lib/*)
set(RTI_LIB_DIR "")
foreach(d ${RTI_ARCH_DIRS})
    if(IS_DIRECTORY ${d} AND NOT ${d} MATCHES "java$")
        # Pick first arch-dir that has libnddscpp2.{so,dylib}
        if(EXISTS ${d}/libnddscpp2.so OR EXISTS ${d}/libnddscpp2.dylib)
            set(RTI_LIB_DIR ${d})
            break()
        endif()
    endif()
endforeach()
# rti_app.cpp uses the ISO C++ PSM (<dds/dds.hpp>) → libnddscpp2 (not
# the legacy libnddscpp). libnddscppz was the zero-copy variant (no
# longer present in 7.3).
if(APPLE)
    set(RTI_NDDS_SUFFIX ".dylib")
else()
    set(RTI_NDDS_SUFFIX ".so")
endif()
if(RTI_DDSGEN AND RTI_LIB_DIR AND EXISTS ${RTI_LIB_DIR}/libnddscpp2${RTI_NDDS_SUFFIX})
    message(STATUS "Found RTI: ${RTI_HOME}")
    set(RTI_GEN_DIR ${CMAKE_CURRENT_BINARY_DIR}/rti-gen)
    file(MAKE_DIRECTORY ${RTI_GEN_DIR})
    add_custom_command(
        OUTPUT ${RTI_GEN_DIR}/roundtrip.hpp ${RTI_GEN_DIR}/roundtrip.cxx
            ${RTI_GEN_DIR}/roundtripPlugin.cxx ${RTI_GEN_DIR}/roundtripPlugin.hpp
        # rtiddsgen 4.3 (RTI 7.3) with -language C++11 generates only:
        # roundtrip.{hpp,cxx} + roundtripPlugin.{hpp,cxx} — no
        # Support.cxx any more (<=6.0.1 still had it).
        COMMAND env NDDSHOME=${RTI_HOME}
            RTI_LICENSE_FILE=${RTI_HOME}/rti_license.dat
            LD_LIBRARY_PATH=${RTI_LIB_DIR}
            ${RTI_DDSGEN} -language C++11 -d ${RTI_GEN_DIR} -replace ${BENCH_DIR}/roundtrip.idl
        DEPENDS ${BENCH_DIR}/roundtrip.idl
        COMMENT "rti: rtiddsgen roundtrip.idl"
    )
    add_executable(rti-roundtrip
        rti_app.cpp
        ${RTI_GEN_DIR}/roundtrip.cxx
        ${RTI_GEN_DIR}/roundtripPlugin.cxx
    )
    target_include_directories(rti-roundtrip PRIVATE
        ${RTI_HOME}/include
        ${RTI_HOME}/include/ndds
        ${RTI_HOME}/include/ndds/hpp
        # Headers generated by rtiddsgen 4.3 (RTI 7.3) reference
        # `dds/core/SafeEnumeration.hpp` etc. → those live under
        # resource/app/app_support/rtiddsgen/standalone/include/cpp11
        ${RTI_HOME}/resource/app/app_support/rtiddsgen/standalone/include/cpp11
        ${RTI_GEN_DIR}
    )
    if(APPLE)
        target_compile_definitions(rti-roundtrip PRIVATE RTI_UNIX RTI_DARWIN RTI_64BIT)
    else()
        target_compile_definitions(rti-roundtrip PRIVATE RTI_UNIX RTI_LINUX RTI_64BIT)
    endif()
    set(RTI_LINK_LIBS
        ${RTI_LIB_DIR}/libnddscpp2${RTI_NDDS_SUFFIX}
        ${RTI_LIB_DIR}/libnddsc${RTI_NDDS_SUFFIX}
        ${RTI_LIB_DIR}/libnddscore${RTI_NDDS_SUFFIX}
        pthread dl m
    )
    if(NOT APPLE)
        # librt is missing on macOS (the POSIX timer functions are in libc).
        list(APPEND RTI_LINK_LIBS rt)
    endif()
    target_link_libraries(rti-roundtrip PRIVATE ${RTI_LINK_LIBS})
else()
    message(WARNING "RTI Connext not found (${RTI_HOME})")
endif()

# ============================================================
# opendds-roundtrip — OpenDDS via opendds_target_sources-Codegen
# ============================================================
# OpenDDS CMake package. OPENDDS_ROOT points at the install prefix
# (`make install` yields share/dds/cmake/OpenDDSConfig.cmake) OR at
# an in-tree source build (cmake/ subdirectory).
# Deliberately NO NO_DEFAULT_PATH — OpenDDS's own init.cmake makes
# further find_path calls internally (ACE/TAO/etc), and NO_DEFAULT_PATH
# on find_package restricts those internal find calls as well,
# which breaks OpenDDS setups with ACE_wrappers in a subdirectory
# (source builds have that, RPM installs in /usr/share do not).
find_package(OpenDDS QUIET
    PATHS ${OPENDDS_ROOT}/share/cmake/OpenDDS
          ${OPENDDS_ROOT}/share/dds/cmake
          ${OPENDDS_ROOT}/cmake
          ${OPENDDS_ROOT}/lib/cmake/OpenDDS)
if(OpenDDS_FOUND)
    message(STATUS "Found OpenDDS: ${OPENDDS_ROOT}")
    add_executable(opendds-roundtrip opendds_app.cpp)
    # opendds_target_sources generates TypeSupport via opendds_idl +
    # tao_idl from roundtrip.idl. Classic C++ mapping (default) —
    # the app uses RoundtripDataWriter/.payload.length().
    #
    # -Gxtypes-complete: by default OpenDDS emits only the "minimal"
    # TypeObject info. Cyclone (and other strict XTypes consumers)
    # reject minimal-only — the XTypes spec requires minimal AND
    # complete (or none). With the flag opendds_idl also generates the
    # complete TypeObject -> cross-vendor discovery works.
    # Ref: OpenDDS#4244 / cyclonedds-cxx#448.
    opendds_target_sources(opendds-roundtrip
        PUBLIC ${BENCH_DIR}/roundtrip.idl
        OPENDDS_IDL_OPTIONS -Gxtypes-complete
    )
    target_link_libraries(opendds-roundtrip
        OpenDDS::Dcps OpenDDS::Rtps OpenDDS::Rtps_Udp pthread)
else()
    message(WARNING "OpenDDS not found (${OPENDDS_ROOT}/cmake)")
endif()

# ============================================================
# RICH-TYPED variant: roundtrip_rich.idl -> RoundtripBench::RoundtripRich
# ============================================================
# Codec-heavy type (string + double[16] + sequence<Waypoint{2×Vec3+string}>
# + payload) through the same typed CDR path. Measures the XCDR2 member codec
# share against the pure transport share. One parallel
# `<vendor>-roundtrip-rich` binary each; reuses the SDK variables found above.

# zerodds-rich — checked-in gen/zerodds/RoundtripRich.hpp (like
# Roundtrip.hpp, generated via `zerodds-idlc generate --lang c++ --cpp-mapping native --out
# gen/zerodds roundtrip_rich.idl`, then renamed to match this file's include).
if(EXISTS ${ZERODDS_LIB} AND EXISTS ${ZERODDS_HDR}
        AND EXISTS ${BENCH_DIR}/gen/zerodds/RoundtripRich.hpp)
    add_executable(zerodds-roundtrip-rich zerodds_app.cpp)
    target_compile_definitions(zerodds-roundtrip-rich PRIVATE BENCH_RICH)
    target_include_directories(zerodds-roundtrip-rich PRIVATE
        ${ZERODDS_REPO}/crates/zerodds-c-api/include
        ${BENCH_DIR}/gen/zerodds
        ${ZERODDS_REPO}/crates/cpp/include
    )
    target_link_libraries(zerodds-roundtrip-rich PRIVATE
        ${ZERODDS_LIB}
        zerodds::zerodds-wire-cpp
        pthread dl m
    )
endif()

# cyclone-rich
if(CYCLONE_CXX_INCLUDE AND CYCLONE_CXX_LIB AND CYCLONE_IDLC)
    set(CY_GEN_RICH ${CMAKE_CURRENT_BINARY_DIR}/cyclone-gen-rich)
    file(MAKE_DIRECTORY ${CY_GEN_RICH})
    add_custom_command(
        OUTPUT ${CY_GEN_RICH}/roundtrip_rich.hpp ${CY_GEN_RICH}/roundtrip_rich.cpp
        COMMAND ${CYCLONE_IDLC} -l cxx -o ${CY_GEN_RICH} ${BENCH_DIR}/roundtrip_rich.idl
        DEPENDS ${BENCH_DIR}/roundtrip_rich.idl
        WORKING_DIRECTORY ${CY_GEN_RICH}
        COMMENT "cyclone: idlc -l cxx roundtrip_rich.idl"
    )
    add_executable(cyclone-roundtrip-rich cyclone_app.cpp ${CY_GEN_RICH}/roundtrip_rich.cpp)
    target_compile_definitions(cyclone-roundtrip-rich PRIVATE BENCH_RICH)
    target_include_directories(cyclone-roundtrip-rich PRIVATE
        ${CYCLONE_CXX_INCLUDE} ${CYCLONE_ROOT}/include ${CY_GEN_RICH})
    target_link_libraries(cyclone-roundtrip-rich PRIVATE
        ${CYCLONE_CXX_LIB} ${CYCLONE_C_LIB} pthread)
endif()

# fastdds-rich
if(FASTDDS_INCLUDE AND FASTDDS_LIB AND FASTDDS_GEN)
    set(FD_GEN_RICH ${CMAKE_CURRENT_BINARY_DIR}/fastdds-gen-rich)
    file(MAKE_DIRECTORY ${FD_GEN_RICH})
    add_custom_command(
        OUTPUT ${FD_GEN_RICH}/roundtrip_rich.hpp ${FD_GEN_RICH}/roundtrip_richPubSubTypes.cxx
        COMMAND ${FASTDDS_GEN} -no-typeobjectsupport
            -d ${FD_GEN_RICH} -replace ${BENCH_DIR}/roundtrip_rich.idl
        DEPENDS ${BENCH_DIR}/roundtrip_rich.idl
        COMMENT "fastdds: fastddsgen -no-typeobjectsupport roundtrip_rich.idl"
    )
    add_executable(fastdds-roundtrip-rich
        fastdds_app.cpp ${FD_GEN_RICH}/roundtrip_richPubSubTypes.cxx)
    target_compile_definitions(fastdds-roundtrip-rich PRIVATE BENCH_RICH)
    target_include_directories(fastdds-roundtrip-rich PRIVATE ${FASTDDS_INCLUDE} ${FD_GEN_RICH})
    target_link_libraries(fastdds-roundtrip-rich PRIVATE ${FASTDDS_LIB} ${FASTCDR_LIB} pthread)
endif()

# rti-rich
if(RTI_DDSGEN AND RTI_LIB_DIR AND EXISTS ${RTI_LIB_DIR}/libnddscpp2${RTI_NDDS_SUFFIX})
    set(RTI_GEN_RICH ${CMAKE_CURRENT_BINARY_DIR}/rti-gen-rich)
    file(MAKE_DIRECTORY ${RTI_GEN_RICH})
    add_custom_command(
        OUTPUT ${RTI_GEN_RICH}/roundtrip_rich.cxx ${RTI_GEN_RICH}/roundtrip_richPlugin.cxx
        COMMAND env NDDSHOME=${RTI_HOME}
            RTI_LICENSE_FILE=${RTI_HOME}/rti_license.dat
            LD_LIBRARY_PATH=${RTI_LIB_DIR}
            ${RTI_DDSGEN} -language C++11 -d ${RTI_GEN_RICH} -replace ${BENCH_DIR}/roundtrip_rich.idl
        DEPENDS ${BENCH_DIR}/roundtrip_rich.idl
        COMMENT "rti: rtiddsgen roundtrip_rich.idl"
    )
    add_executable(rti-roundtrip-rich
        rti_app.cpp ${RTI_GEN_RICH}/roundtrip_rich.cxx ${RTI_GEN_RICH}/roundtrip_richPlugin.cxx)
    target_compile_definitions(rti-roundtrip-rich PRIVATE BENCH_RICH)
    target_include_directories(rti-roundtrip-rich PRIVATE
        ${RTI_HOME}/include ${RTI_HOME}/include/ndds ${RTI_HOME}/include/ndds/hpp
        ${RTI_HOME}/resource/app/app_support/rtiddsgen/standalone/include/cpp11
        ${RTI_GEN_RICH})
    if(APPLE)
        target_compile_definitions(rti-roundtrip-rich PRIVATE RTI_UNIX RTI_DARWIN RTI_64BIT)
    else()
        target_compile_definitions(rti-roundtrip-rich PRIVATE RTI_UNIX RTI_LINUX RTI_64BIT)
    endif()
    target_link_libraries(rti-roundtrip-rich PRIVATE ${RTI_LINK_LIBS})
endif()

# ============================================================
# opensplice-roundtrip — OpenSplice Community Edition 6.9, classic C++ API
# ============================================================
# idlpp reads no annotations, so the type comes from roundtrip_ospl.idl
# (roundtrip.idl in keylist form). The app is opensplice-roundtrip-bin; the
# name the sweep calls, opensplice-roundtrip, is the wrapper that writes the
# OpenSplice configuration for the bench domain (opensplice_roundtrip.sh.in).
if(NOT DEFINED OSPL_HOME)
    set(OSPL_HOME /opt/opensplice-6.9)
endif()
if(EXISTS ${OSPL_HOME}/bin/idlpp AND EXISTS ${OSPL_HOME}/lib/libdcpssacpp.so)
    message(STATUS "Found OpenSplice: ${OSPL_HOME}")
    set(OSPL_GEN_DIR ${CMAKE_CURRENT_BINARY_DIR}/opensplice-gen)
    set(OSPL_GEN_SRCS
        ${OSPL_GEN_DIR}/roundtrip_ospl.cpp
        ${OSPL_GEN_DIR}/roundtrip_osplDcps.cpp
        ${OSPL_GEN_DIR}/roundtrip_osplDcps_impl.cpp
        ${OSPL_GEN_DIR}/roundtrip_osplSplDcps.cpp)
    add_custom_command(
        OUTPUT ${OSPL_GEN_SRCS}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${OSPL_GEN_DIR}
        COMMAND env OSPL_HOME=${OSPL_HOME} LD_LIBRARY_PATH=${OSPL_HOME}/lib
            ${OSPL_HOME}/bin/idlpp -S -l cpp -d ${OSPL_GEN_DIR} ${BENCH_DIR}/roundtrip_ospl.idl
        DEPENDS ${BENCH_DIR}/roundtrip_ospl.idl
        COMMENT "opensplice: idlpp -S -l cpp roundtrip_ospl.idl"
    )
    add_executable(opensplice-roundtrip-bin opensplice_app.cpp ${OSPL_GEN_SRCS})
    target_include_directories(opensplice-roundtrip-bin PRIVATE
        ${OSPL_GEN_DIR}
        ${OSPL_HOME}/include
        ${OSPL_HOME}/include/sys
        ${OSPL_HOME}/include/dcps/C++/SACPP)
    target_link_directories(opensplice-roundtrip-bin PRIVATE ${OSPL_HOME}/lib)
    target_link_libraries(opensplice-roundtrip-bin PRIVATE dcpssacpp ddskernel pthread)
    set_target_properties(opensplice-roundtrip-bin PROPERTIES BUILD_RPATH ${OSPL_HOME}/lib)
    # configure_file keeps the template's permissions: the wrapper is executable.
    configure_file(${BENCH_DIR}/opensplice_roundtrip.sh.in
        ${CMAKE_CURRENT_BINARY_DIR}/opensplice-roundtrip @ONLY)
else()
    message(WARNING "OpenSplice not found (${OSPL_HOME}: bin/idlpp, lib/libdcpssacpp.so)")
endif()

# opendds-rich — opendds_target_sources generates TypeSupport from roundtrip_rich.idl.
if(OpenDDS_FOUND)
    add_executable(opendds-roundtrip-rich opendds_app.cpp)
    target_compile_definitions(opendds-roundtrip-rich PRIVATE BENCH_RICH)
    opendds_target_sources(opendds-roundtrip-rich
        PUBLIC ${BENCH_DIR}/roundtrip_rich.idl
        OPENDDS_IDL_OPTIONS -Gxtypes-complete)
    target_link_libraries(opendds-roundtrip-rich
        OpenDDS::Dcps OpenDDS::Rtps OpenDDS::Rtps_Udp pthread)
endif()
