# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The DDS-PSM-Cxx DataWriter<T> and DataReader<T> over types zerodds-idlc
# generates, the way a user's CMake project generates them
# (`zerodds_idlc_generate`, cmake/zerodds_idlc_generate.cmake): the IDL
# beside this file is generated on every build, so nothing generated is
# committed to go stale. Added from the root CMakeLists.txt with the tests on.
# The programs take a leased domain from `ZERODDS_TEST_DOMAINS`
# (../test_support.hpp).

# The native mapping of IDL4 to C++ (`--cpp-mapping native`) through the PSM classes.
zerodds_idlc_generate(cpp_typed_probe
    IDL_FILES "${CMAKE_CURRENT_SOURCE_DIR}/typed_probe.idl"
    GENERATOR_BACKEND cpp
    GENERATOR_FLAGS --cpp-mapping native
    OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/typed")
add_executable(generated_type_roundtrip "${CMAKE_CURRENT_SOURCE_DIR}/../generated_type_roundtrip.cpp")
target_link_libraries(generated_type_roundtrip PRIVATE cpp_typed_probe)

# The DDS-PSM-Cxx 1.0 §8.1 mapping (`--cpp-mapping psm`): classes with accessors,
# dds::core::optional, std::map, std::wstring, long double, through zerodds::psm::Conv.
zerodds_idlc_generate(cpp_psm_probe
    IDL_FILES "${CMAKE_CURRENT_SOURCE_DIR}/psm_probe.idl"
    GENERATOR_BACKEND cpp
    GENERATOR_FLAGS --cpp-mapping psm
    OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/psm")
add_executable(generated_psm_roundtrip "${CMAKE_CURRENT_SOURCE_DIR}/../generated_psm_roundtrip.cpp")
target_link_libraries(generated_psm_roundtrip PRIVATE cpp_psm_probe)

# A MultiTopic over two generated topics (DDS 1.4 §2.2.2.3.4, Annex B.2).
zerodds_idlc_generate(cpp_multitopic_probe
    IDL_FILES "${CMAKE_CURRENT_SOURCE_DIR}/multitopic_probe.idl"
    GENERATOR_BACKEND cpp
    GENERATOR_FLAGS --cpp-mapping native
    OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/multitopic")
add_executable(generated_multitopic "${CMAKE_CURRENT_SOURCE_DIR}/../generated_multitopic.cpp")
target_link_libraries(generated_multitopic PRIVATE cpp_multitopic_probe)

foreach(program generated_type_roundtrip generated_psm_roundtrip generated_multitopic)
    target_include_directories(${program} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/..")
    if(NOT MSVC)
        target_compile_options(${program} PRIVATE -Wall -Werror)
    endif()
endforeach()

if(BUILD_TESTING)
    add_test(NAME psmcxx_7_12_a_generated_type_round_trips_through_the_dds_psm_writer_and_reader
             COMMAND generated_type_roundtrip)
    add_test(NAME psmcxx_8_1_a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader
             COMMAND generated_psm_roundtrip)
    add_test(NAME dcps_b_2_a_multitopic_joins_two_generated_topics_for_a_data_reader
             COMMAND generated_multitopic)
endif()
