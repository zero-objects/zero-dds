# SPDX-License-Identifier: Apache-2.0
#
# Cross-Target Roundtrip-Bench Build.
#
# Baut ein Per-Vendor-Binary aus der gemeinsamen roundtrip.idl + den
# Per-Vendor-Apps (cyclone_app.cpp / fastdds_app.cpp / rti_app.cpp /
# zerodds_app.cpp). Apples-to-apples: gleiche IDL → gleiches Wire-
# Format. Per Vendor wird optional gebaut (skipping wenn SDK nicht
# auffindbar).
#
# Vendor-Pfade per -D überschreibbar (Default = /opt-Layout; per Host anpassen):
#   -DZERODDS_REPO=/path/to/zerodds      (default: ../../..)
#   -DCYCLONE_ROOT=/opt/cyclone
#   -DFASTDDS_ROOT=/opt/fastdds
#   -DRTI_HOME=/opt/rti.com/rti_connext_dds-7.7.0
#   -DOPENDDS_ROOT=/opt/opendds          (oder /opt/opendds-secure für DDS-Security)
#
# Lib-Suffix auto-detect (.so vs .dylib). RTI-arch-dir auto-detect via
# glob (lib/{x64Linux4gcc8.5.0, arm64Darwin23clang16.0, ...}).
#
# Usage:
#   mkdir build && cd build
#   cmake [-Dvars] ..
#   cmake --build .
#   # ergibt {cyclone,fastdds,rti,zerodds,opendds}-roundtrip (wo SDKs
#   # gefunden wurden).

cmake_minimum_required(VERSION 3.16)
project(dds-roundtrip-bench CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
add_compile_options(-O2 -Wall -Wextra)

set(BENCH_DIR ${CMAKE_CURRENT_SOURCE_DIR})

# Default ZEROCDDS_REPO = .../zerodds (relative an unsere Position).
if(NOT DEFINED ZERODDS_REPO)
    get_filename_component(ZERODDS_REPO "${BENCH_DIR}/../../.." ABSOLUTE)
endif()
# Default vendor-roots — überschreibbar via -D.
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
# Lib-Suffix: .dylib auf macOS, .so sonst.
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
        pthread dl m
    )
else()
    message(WARNING "ZeroDDS not built: cargo build --release -p zerodds-c-api")
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
# Fast-DDS v3.x lib heißt "fastdds" (legacy v2.x: "fastrtps"). Beide
# probieren — finden = name reicht aus auch wenn cmake doc-string sagt
# der erste.
find_library(FASTDDS_LIB
    NAMES fastdds fastrtps
    PATHS ${FASTDDS_ROOT}/lib NO_DEFAULT_PATH)
# fastcdr kommt aus dem source-build (parallel zu fast-dds gebaut).
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
    # fastddsgen v4.x erzeugt:
    #   roundtrip.hpp                          (struct definition, header-only)
    #   roundtripCdrAux.{hpp,ipp}              (CDR Aux header-only)
    #   roundtripPubSubTypes.{hpp,cxx}         (TypeSupport)
    # `-no-typeobjectsupport`: fastddsgen v4 generiert per Default
    # einen TypeObject (roundtripTypeObjectSupport.*) der für
    # `sequence<octet, 8192>` einen all-zero PLAIN_SEQUENCE_LARGE
    # encoded — Cyclone DDS rejected den als "invalid type object"
    # ("ddsi_xt_type_add_typeobj with invalid type object" in pong-log).
    # Ohne TypeObject fällt SEDP-Match auf Type-name + ALLOW_TYPE_COERCION
    # zurück → cyclone↔fastdds matched.
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
# RTI 7.7.0 — die Express/Free-Lizenz (CONNEXTPRO + TrialEndLimits)
# ist im 7.5.0+-Format; RTI 7.3.0 konnte es nicht lesen ("feature
# license not found"). Arch-dir variiert pro Plattform:
#   Linux x64:    lib/x64Linux4gcc8.5.0
#   macOS arm64:  lib/arm64Darwin23clang16.0
# Glob beide.
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
# rti_app.cpp nutzt ISO C++ PSM (<dds/dds.hpp>) → libnddscpp2 (nicht
# das legacy libnddscpp). libnddscppz war zero-copy-Variante (gibt's
# in 7.3 nicht mehr).
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
        # rtiddsgen 4.3 (RTI 7.3) generiert mit -language C++11 nur:
        # roundtrip.{hpp,cxx} + roundtripPlugin.{hpp,cxx} — kein
        # Support.cxx mehr (in <=6.0.1 gab es das noch).
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
        # rtiddsgen 4.3 (RTI 7.3) generierte Header refen
        # `dds/core/SafeEnumeration.hpp` etc. → die liegen unter
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
        # librt fehlt auf macOS (POSIX-timer-Funktionen in libc).
        list(APPEND RTI_LINK_LIBS rt)
    endif()
    target_link_libraries(rti-roundtrip PRIVATE ${RTI_LINK_LIBS})
else()
    message(WARNING "RTI Connext not found (${RTI_HOME})")
endif()

# ============================================================
# opendds-roundtrip — OpenDDS via opendds_target_sources-Codegen
# ============================================================
# OpenDDS-CMake-Package. OPENDDS_ROOT zeigt auf den Install-Prefix
# (`make install` ergibt share/dds/cmake/OpenDDSConfig.cmake) ODER auf
# einen In-Tree-Source-Build (cmake/-Subdir).
# NO_DEFAULT_PATH absichtlich NICHT — OpenDDS's eigenes init.cmake macht
# weitere find_path-Calls intern (ACE/TAO/etc), und mit NO_DEFAULT_PATH
# auf find_package werden die internen find-Calls auch eingeschraenkt,
# was OpenDDS-Setups mit ACE_wrappers in einem Subverzeichnis sprengt
# (Source-Builds haben das, RPM-Installs in /usr/share nicht).
find_package(OpenDDS QUIET
    PATHS ${OPENDDS_ROOT}/share/cmake/OpenDDS
          ${OPENDDS_ROOT}/share/dds/cmake
          ${OPENDDS_ROOT}/cmake
          ${OPENDDS_ROOT}/lib/cmake/OpenDDS)
if(OpenDDS_FOUND)
    message(STATUS "Found OpenDDS: ${OPENDDS_ROOT}")
    add_executable(opendds-roundtrip opendds_app.cpp)
    # opendds_target_sources generiert TypeSupport via opendds_idl +
    # tao_idl aus roundtrip.idl. Klassisches C++-Mapping (default) —
    # die App nutzt RoundtripDataWriter/.payload.length().
    #
    # -Gxtypes-complete: OpenDDS emittiert per Default nur die "minimal"
    # TypeObject-Info. Cyclone (und andere strikte XTypes-Consumer)
    # lehnen minimal-only ab — die XTypes-Spec verlangt minimal UND
    # complete (oder keine). Mit dem Flag generiert opendds_idl auch das
    # complete TypeObject -> Cross-Vendor-Discovery klappt.
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
# Codec-schwerer Typ (string + double[16] + sequence<Waypoint{2×Vec3+string}>
# + payload) durch denselben typed CDR-Pfad. Misst den XCDR2-Member-Codec-
# Anteil gegen den reinen Transport-Anteil. Jeweils ein paralleles
# `<vendor>-roundtrip-rich`-Binary; reuse der oben gefundenen SDK-Vars.

# zerodds-rich — checked-in gen/zerodds/RoundtripRich.hpp (analog Roundtrip.hpp,
# erzeugt via `cargo run -p zerodds-idl-cpp --example emit_cpp -- roundtrip_rich.idl`).
if(EXISTS ${ZERODDS_LIB} AND EXISTS ${ZERODDS_HDR}
        AND EXISTS ${BENCH_DIR}/gen/zerodds/RoundtripRich.hpp)
    add_executable(zerodds-roundtrip-rich zerodds_app_rich.cpp)
    target_include_directories(zerodds-roundtrip-rich PRIVATE
        ${ZERODDS_REPO}/crates/zerodds-c-api/include
        ${BENCH_DIR}/gen/zerodds
        ${ZERODDS_REPO}/crates/cpp/include
    )
    target_link_libraries(zerodds-roundtrip-rich PRIVATE ${ZERODDS_LIB} pthread dl m)
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
    add_executable(cyclone-roundtrip-rich cyclone_app_rich.cpp ${CY_GEN_RICH}/roundtrip_rich.cpp)
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
        fastdds_app_rich.cpp ${FD_GEN_RICH}/roundtrip_richPubSubTypes.cxx)
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
        rti_app_rich.cpp ${RTI_GEN_RICH}/roundtrip_rich.cxx ${RTI_GEN_RICH}/roundtrip_richPlugin.cxx)
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

# opendds-rich — opendds_target_sources generiert TypeSupport aus roundtrip_rich.idl.
if(OpenDDS_FOUND)
    add_executable(opendds-roundtrip-rich opendds_app_rich.cpp)
    opendds_target_sources(opendds-roundtrip-rich
        PUBLIC ${BENCH_DIR}/roundtrip_rich.idl
        OPENDDS_IDL_OPTIONS -Gxtypes-complete)
    target_link_libraries(opendds-roundtrip-rich
        OpenDDS::Dcps OpenDDS::Rtps OpenDDS::Rtps_Udp pthread)
endif()
