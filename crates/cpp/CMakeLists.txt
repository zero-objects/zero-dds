cmake_minimum_required(VERSION 3.21)
project(zerodds-cpp
    VERSION 1.0.0
    DESCRIPTION "ZERODDS CXX bindings"
    LANGUAGES CXX
)

include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)

# BUILD_SHARED_LIBS is intentionally NOT (re)declared here: this is a
# header-only INTERFACE target, so the flag is meaningless, and an option()
# for it would clobber a parent project's value under add_subdirectory().
#
# ZERODDS_BUILD_EXAMPLES / ZERODDS_BUILD_TESTS are normally already defined by
# the root CMakeLists.txt (with defaults tied to whether *zerodds* is
# top-level). When crates/cpp is configured standalone the option() calls below
# take effect, defaulting to whether THIS project is top-level. The previous
# generic BUILD_EXAMPLES / BUILD_TESTS names collided with parent-project
# options and defaulted ON for consumers; they are dropped, not aliased.
option(ZERODDS_BUILD_EXAMPLES "Build zerodds C++ examples" ${PROJECT_IS_TOP_LEVEL})
option(ZERODDS_BUILD_TESTS "Build zerodds C++ tests" ${PROJECT_IS_TOP_LEVEL})

# Interface library being header-only
set(ZERODDS_CXX_HEADERS
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/cond/Condition.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/policy/CorePolicy.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Exception.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/psm/Fixed.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/psm/check_status.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/psm/handle_seq.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/psm/expression_parameters.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/psm/type_support.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/InstanceHandle.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/qos_bridge.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/status/Status.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/status/State.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/policy/QosPolicyCount.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Time.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/domain/DomainParticipant.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/pub/DataWriterListener.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/pub/Publisher.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/pub/discovery.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/cond/ReadCondition.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/DataReaderListener.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/Subscriber.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/discovery.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/BuiltinTopic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/ContentFilteredTopic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/Topic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/TopicTraits.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/dds.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/dds.hpp"
)
add_library(zerodds-cpp INTERFACE ${ZERODDS_CXX_HEADERS})
add_library(zerodds::zerodds-cpp ALIAS zerodds-cpp)

target_include_directories(zerodds-cpp
    INTERFACE
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
)

target_link_libraries(zerodds-cpp INTERFACE zerodds::zerodds-c
                                            zerodds::zerodds-wire-cpp
                                            $<$<BOOL:${WIN32}>:userenv>
)

# ---------------------------------------------------------------------------
# Sanitizer (ZERODDS_SANITIZER, set at the root project: none|address|
# undefined|thread|address,undefined). Applies to the header-only interface
# target, so every consumer (examples/tests below, and downstream
# target_link_libraries(... zerodds::zerodds-cpp) users building from
# source) gets the same instrumented build — NOT to the Rust cdylib/staticlib
# across the FFI boundary, which is a separate Rust-toolchain concern.
# ---------------------------------------------------------------------------
if(DEFINED ZERODDS_SANITIZER AND NOT ZERODDS_SANITIZER STREQUAL "none" AND NOT MSVC)
    # `-fsanitize=` itself takes a comma-separated list (e.g.
    # "address,undefined"), so ZERODDS_SANITIZER is forwarded verbatim.
    target_compile_options(zerodds-cpp INTERFACE
        -fsanitize=${ZERODDS_SANITIZER} -fno-omit-frame-pointer -g
    )
    target_link_options(zerodds-cpp INTERFACE -fsanitize=${ZERODDS_SANITIZER})
endif()

# Examples
if(ZERODDS_BUILD_EXAMPLES)
    add_executable(cpp_smoke ${CMAKE_CURRENT_SOURCE_DIR}/examples/cpp_smoke.cpp)
    target_link_libraries(cpp_smoke PRIVATE zerodds::zerodds-cpp)
endif()

# Tests — registered with CTest (root CMakeLists.txt `include(CTest)`), so
# `ctest` / `cmake --build . --target test` run them, not just `BUILD_TESTS`
# producing an executable nobody invokes. The integration pillar's job
# `integration-sdk` (tests/integration/sdk_cmake.sh) runs CTest with leased
# DDS domains in `ZERODDS_TEST_DOMAINS` and 25 s per test.
if(ZERODDS_BUILD_TESTS)
    # The DDS-PSM-Cxx smoke program: every test `main()` calls, in one run.
    add_executable(smoke_dds_psm ${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke_dds_psm.cpp)
    target_link_libraries(smoke_dds_psm PRIVATE zerodds::zerodds-cpp)

    # The same program as ISO C++ 2003 (DDS-PSM-Cxx 1.0 §3 [C++], §7.1): the headers need no
    # C++11; the tests that do (threads, move semantics) are not compiled there.
    add_executable(smoke_dds_psm_cxx03 ${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke_dds_psm.cpp)
    target_link_libraries(smoke_dds_psm_cxx03 PRIVATE zerodds::zerodds-cpp)
    set_target_properties(smoke_dds_psm_cxx03 PROPERTIES
        CXX_STANDARD 98 CXX_STANDARD_REQUIRED ON CXX_EXTENSIONS OFF)
    if(NOT MSVC)
        # `long long` is a C99 and C++11 type the C API uses.
        target_compile_options(smoke_dds_psm_cxx03 PRIVATE -pedantic -Wno-long-long)
    endif()

    # The DataWriter and DataReader classes and the TypeSupports of the four built-in types
    # (DDS-XTypes 1.3 §7.6.5, Annex E).
    add_executable(builtin_data_endpoints ${CMAKE_CURRENT_SOURCE_DIR}/tests/builtin_data_endpoints.cpp)
    target_link_libraries(builtin_data_endpoints PRIVATE zerodds::zerodds-cpp)
    add_executable(builtin_type_support ${CMAKE_CURRENT_SOURCE_DIR}/tests/builtin_type_support.cpp)
    target_link_libraries(builtin_type_support PRIVATE zerodds::zerodds-cpp)
    if(NOT MSVC)
        target_compile_options(builtin_data_endpoints PRIVATE -Wall -Werror)
        target_compile_options(builtin_type_support PRIVATE -Wall -Werror)
    endif()

    if(BUILD_TESTING)
        add_test(NAME dds_psm_cxx_smoke COMMAND smoke_dds_psm)
        add_test(NAME psmcxx_7_1_the_smoke_program_runs_as_cxx03 COMMAND smoke_dds_psm_cxx03)
        add_test(NAME xtypes_7_6_5_the_data_writers_and_readers_of_the_built_in_types_write_and_take
                 COMMAND builtin_data_endpoints)
        add_test(NAME xtypes_7_6_5_the_type_supports_of_the_built_in_types_name_and_register_their_types
                 COMMAND builtin_type_support)
    endif()

    # The smoke program under the sanitizers, where the compiler has them and no sanitizer is
    # set for the whole build (ZERODDS_SANITIZER instruments every consumer already). The Rust
    # library is not instrumented: the C++ headers and the tests are.
    if(NOT MSVC AND CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang"
       AND (NOT DEFINED ZERODDS_SANITIZER OR ZERODDS_SANITIZER STREQUAL "none"))
        # A use of a freed object or a write past a buffer in the C++ headers.
        add_executable(smoke_dds_psm_asan ${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke_dds_psm.cpp)
        target_link_libraries(smoke_dds_psm_asan PRIVATE zerodds::zerodds-cpp)
        target_compile_options(smoke_dds_psm_asan PRIVATE -fsanitize=address -g -O1 -fno-omit-frame-pointer)
        target_link_options(smoke_dds_psm_asan PRIVATE -fsanitize=address)
        # The tests in which threads call the operations of the entities at once (DDS-PSM-Cxx
        # 1.0 §7.3): a data race in the C++ headers fails; tests/tsan_verdict.py tells the
        # reports in the uninstrumented library from the others.
        add_executable(smoke_dds_psm_tsan ${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke_dds_psm.cpp)
        target_link_libraries(smoke_dds_psm_tsan PRIVATE zerodds::zerodds-cpp)
        target_compile_options(smoke_dds_psm_tsan PRIVATE -fsanitize=thread -g -O1 -fno-omit-frame-pointer)
        target_link_options(smoke_dds_psm_tsan PRIVATE -fsanitize=thread)
        if(BUILD_TESTING)
            add_test(NAME dds_psm_cxx_smoke_under_address_sanitizer COMMAND smoke_dds_psm_asan)
            add_test(NAME psmcxx_7_3_no_data_race_in_the_cpp_code_under_thread_sanitizer
                     COMMAND python3 ${CMAKE_CURRENT_SOURCE_DIR}/tests/tsan_verdict.py
                             $<TARGET_FILE:smoke_dds_psm_tsan> threads)
        endif()
    endif()
endif()

# Every public header must compile on its own (a header that relies on an
# earlier include of the application breaks `#include <dds/pub/find.hpp>`).
# One translation unit per header; nothing links or runs.
if(ZERODDS_BUILD_TESTS)
    file(GLOB_RECURSE ZERODDS_PSM_PUBLIC_HEADERS RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}/include"
         "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/*.hpp")
    set(ZERODDS_PSM_STANDALONE_SOURCES "")
    foreach(header IN LISTS ZERODDS_PSM_PUBLIC_HEADERS)
        if(NOT header MATCHES "/detail/")
            string(MAKE_C_IDENTIFIER "${header}" header_id)
            set(tu "${CMAKE_CURRENT_BINARY_DIR}/standalone/${header_id}.cpp")
            file(WRITE "${tu}" "#include <${header}>\n")
            list(APPEND ZERODDS_PSM_STANDALONE_SOURCES "${tu}")
        endif()
    endforeach()
    add_library(zerodds_psm_standalone_headers OBJECT ${ZERODDS_PSM_STANDALONE_SOURCES})
    target_link_libraries(zerodds_psm_standalone_headers PRIVATE zerodds::zerodds-cpp)

    # The same translation units as ISO/IEC C++ 2003 (DDS-PSM-Cxx 1.0 section 3 [C++], 7.1): the
    # headers of dds/ need no C++11.
    add_library(zerodds_psm_standalone_headers_cxx03 OBJECT ${ZERODDS_PSM_STANDALONE_SOURCES})
    target_link_libraries(zerodds_psm_standalone_headers_cxx03 PRIVATE zerodds::zerodds-cpp)
    set_target_properties(zerodds_psm_standalone_headers_cxx03 PROPERTIES
        CXX_STANDARD 98 CXX_STANDARD_REQUIRED ON CXX_EXTENSIONS OFF)
endif()

# ---------------------------------------------------------------------------
# Installation — header-only, so only the headers move; the INTERFACE
# target itself is re-created by zeroddsConfig.cmake.in on the consumer
# side (see root CMakeLists.txt's install() block).
# ---------------------------------------------------------------------------
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include/"
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
    FILES_MATCHING PATTERN "*.hpp"
)
